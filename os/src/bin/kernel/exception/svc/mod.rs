//! System call handlers

use core::{ptr, slice};

use bitfield_struct::bitfield;
use macros::AsBits;

use crate::{execution, memory::PAGE_ALLOCATOR, println, UART};

use super::ExceptionSyndrome;

#[derive(AsBits, Debug)]
#[repr(u32)]
/// The SVC code for a specific system call
enum CallCode {
    Print = 0x1000,
    Exit = 0x2000,
    AllocPage = 0x3000,
    SetInfo = 0x4000,
}

#[bitfield(u32)]
pub struct SvcIS {
    #[bits(16)]
    code: CallCode,
    __: u16,
}

#[derive(Debug)]
#[repr(C)]
/// Return values for system calls, to be stored into registers
pub struct Return {
    /// Whether or not the system call was successful
    is_success: bool,
    /// Any relevant value returned from the syscall
    value: u64,
}

/// Creates the appropriate return value for a system call, given a success condition and any values
macro_rules! ret {
    ($is_success:expr) => {{
        Return {
            is_success: $is_success,
            value: Default::default(),
        }
    }};
    ($is_success:expr, $val:expr) => {{
        Return {
            is_success: $is_success,
            value: $val,
        }
    }};
}

/// Creates a successful system call return value
macro_rules! success {
    () => {
        ret!(true)
    };
    ($val:expr) => {
        ret!(true, $val)
    };
}

/// Creates a failure system call return value
macro_rules! fail {
    () => {
        ret!(false)
    };
    ($val:expr) => {
        ret!(false, $val)
    };
}

#[derive(Debug)]
enum SetContextFailure {
    InaccessibleTtbr0 = 0b00,
    MisalignedTtbr0 = 0b01,
    InvalidTcrBits = 0b10,
    InaccessibleUserContext = 0b100,
    MisalignedUserContext = 0b101,
}

/// The general system call handler; dispatches to more specific handlers in other files
pub extern "C" fn handle(arg0: u64, arg1: u64, arg2: u64) -> Return {
    let execution = execution::current()
        .expect("Current execution should be set when receiving a syscall from usermode");
    let esr: u64;
    // SAFETY: This does not touch anything but ESR_EL1 to safely read its value
    unsafe {
        core::arch::asm! {
            "mrs {}, ESR_EL1",
            out(reg) esr,
            options(nomem, nostack, preserves_flags)
        };
    };

    let esr = ExceptionSyndrome::from(esr);
    let iss = unsafe { esr.instruction_syndrome().svc };
    match iss.code() {
        CallCode::Exit => {
            todo!("Implement program exits")
        }
        CallCode::Print => {
            let data_ptr = ptr::from_exposed_addr(
                usize::try_from(arg0).expect("usizes and u64s should be interchangeable"),
            );
            let data_len =
                usize::try_from(arg1).expect("usizes and u64s should be interchangeable");
            // TODO: actually validate pointers
            // SAFETY: If the user is nice...
            let data_bytes = unsafe { slice::from_raw_parts(data_ptr, data_len) };
            let uart = UART.get().expect("UART should be initialized by now");
            uart.lock()
                .write_bytes(data_bytes)
                .expect("UART should not fail");
            success!()
        }
        CallCode::AllocPage => {
            if let Some(result) = PAGE_ALLOCATOR
                .get()
                .expect("Page allocator should be initialized")
                .alloc()
            {
                let addr = result.addr();
                // TODO: store this page for the process
                execution.add_writable_page(result);
                success!(addr)
            } else {
                fail!()
            }
        }
        CallCode::SetInfo => {
            let ret = match execution.set_context(
                ptr::from_exposed_addr(
                    usize::try_from(arg0).expect("`u64` should always fit into `usize`"),
                ),
                arg1,
                arg2,
            ) {
                Ok(()) => success!(),
                #[expect(clippy::as_conversions)]
                Err(err) => fail!(match err {
                    execution::ContextError::MisalignedTtbr0 => {
                        SetContextFailure::MisalignedTtbr0
                    }
                    execution::ContextError::InaccessibleTtbr0 => {
                        SetContextFailure::InaccessibleTtbr0
                    }
                    execution::ContextError::InvalidTcrBits => {
                        SetContextFailure::InvalidTcrBits
                    }
                    execution::ContextError::MisalignedUserContext => {
                        SetContextFailure::MisalignedUserContext
                    }
                    execution::ContextError::InaccessibleUserContext => {
                        SetContextFailure::InaccessibleUserContext
                    }
                } as u64),
            };
            println!("set {:?}", ret);
            ret
        }
    }
}

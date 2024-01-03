//! System call handlers

use core::{arch::asm, ptr};

use bitfield_struct::bitfield;
use macros::AsBits;

use crate::{
    execution::{self, ContextError, ExceptionCode},
    memory::PAGE_ALLOCATOR,
    println, UART,
};

use super::ExceptionSyndrome;

#[derive(AsBits, Debug)]
#[repr(u32)]
/// The SVC code for a specific system call
#[derive(PartialEq)]
pub(super) enum CallCode {
    Print = 0x1000,
    Exit = 0x2000,
    AllocPage = 0x3000,
    SetInfo = 0x4000,
    Eret = 0x0,
}

#[bitfield(u32)]
pub struct SvcIS {
    #[bits(16)]
    pub code: CallCode,
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
        ret!(false)
    };
    ($val:expr) => {
        ret!(false, $val)
    };
}

/// Creates a failure system call return value
macro_rules! fail {
    () => {
        ret!(true)
    };
    ($val:expr) => {
        ret!(true, $val)
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

pub fn eret_handle(x0: u64, x1: u64) {
    let execution = execution::current()
        .expect("Current execution should be set when receiving a syscall from usermode");
    let return_address = execution.user_context().pop();
    unsafe {
        asm! {
            "msr ELR_EL1, {}",
            in(reg) return_address,
            options(nomem, nostack, preserves_flags)
        }
    }
}

/// The general system call handler; dispatches to more specific handlers in other files
pub extern "C" fn handle(arg0: u64, arg1: u64, arg2: u64, arg3: u64) -> Return {
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
            let data_ptr: *const u8 = ptr::from_exposed_addr(
                usize::try_from(arg0).expect("usizes and u64s should be interchangeable"),
            );
            let data_len =
                usize::try_from(arg1).expect("usizes and u64s should be interchangeable");
            // TODO: actually validate pointers
            let uart = UART.get().expect("UART should be initialized by now");
            for offset in 0..data_len {
                let byte = unsafe { data_ptr.byte_add(offset).read() };
                uart.lock().write_byte(byte).expect("UART should not fail");
            }
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
                Ok(()) => {
                    execution.jump_into(ExceptionCode::Resumption, &[]);
                }
                #[expect(clippy::as_conversions)]
                Err(err) => fail!(match err {
                    ContextError::MisalignedTtbr0 => {
                        SetContextFailure::MisalignedTtbr0
                    }
                    ContextError::InaccessibleTtbr0 => {
                        SetContextFailure::InaccessibleTtbr0
                    }
                    ContextError::InvalidTcrBits => {
                        SetContextFailure::InvalidTcrBits
                    }
                    ContextError::MisalignedUserContext => {
                        SetContextFailure::MisalignedUserContext
                    }
                    ContextError::InaccessibleUserContext => {
                        SetContextFailure::InaccessibleUserContext
                    }
                } as u64),
            };
            println!("set {:?}", ret);
            ret
        }
        CallCode::Eret => unreachable!(),
    }
}

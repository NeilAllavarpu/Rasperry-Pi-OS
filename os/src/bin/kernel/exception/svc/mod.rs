//! System call handlers

use core::{arch::asm, ptr};

use bitfield_struct::bitfield;
use macros::AsBits;

use crate::{
    execution::{self, ContextError, ExceptionCode, Execution, EXECUTIONS},
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
    Unblock = 0x5000,
    Block = 0x6000,
    SendSignal = 0x7000,
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
    status: u64,
    /// Any relevant value returned from the syscall
    value: u64,
}

/// Creates the appropriate return value for a system call, given a success condition and any values
macro_rules! ret {
    ($status:expr) => {{
        Return {
            status: $status,
            value: Default::default(),
        }
    }};
    ($status:expr, $val:expr) => {{
        Return {
            status: $status,
            value: $val,
        }
    }};
}

/// Creates a successful system call return value
macro_rules! success {
    () => {
        ret!(0)
    };
    ($val:expr) => {
        ret!(0, $val)
    };
}

/// Creates a failure system call return value
macro_rules! fail {
    () => {
        ret!(1)
    };
    ($status:expr) => {
        ret!($status)
    };
    ($status:expr, $val:expr) => {{
        assert_ne!(status, 0);
        ret!($status, $val)
    }};
}

#[derive(Debug)]
enum SetContextFailure {
    InaccessibleTtbr0 = 0b010,
    MisalignedTtbr0 = 0b011,
    InvalidTcrBits = 0b100,
    InaccessibleUserContext = 0b110,
    MisalignedUserContext = 0b111,
}

/// Handles an `eret`
pub fn handle_eret() {
    let execution = execution::current()
        .expect("Current execution should be set when receiving a syscall from usermode");
    let return_address = execution.user_context().pop();
    println!("ERET: {return_address:X}");
    let sp: u64;
    unsafe {
        asm! {
            "mrs {}, SP_EL0",
            out(reg) sp,
            options(nomem, nostack, preserves_flags)
        }
    }
    println!("sp at {:X}", sp);
    unsafe {
        asm! {
            "msr ELR_EL1, {}",
            in(reg) return_address,
            options(nomem, nostack, preserves_flags)
        }
    }
}

#[allow(clippy::too_many_lines)]
/// The general system call handler; dispatches to more specific handlers in other files
pub extern "C" fn handle(arg0: u64, arg1: u64, arg2: u64) -> Return {
    let execution = execution::current()
        .expect("Current execution should be set when receiving a syscall from usermode");
    let esr_el1: u64;
    // SAFETY: This does not touch anything but ESR_EL1 to safely read its value
    unsafe {
        core::arch::asm! {
            "mrs {}, ESR_EL1",
            lateout(reg) esr_el1,
            options(nomem, nostack, preserves_flags)
        };
    };

    let esr = ExceptionSyndrome::from(esr_el1);
    let iss = unsafe { esr.instruction_syndrome().svc };
    match iss.code() {
        CallCode::Exit => execution.exit(),
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
                execution.add_writable_page(result);
                success!(addr)
            } else {
                fail!()
            }
        }
        CallCode::SetInfo => {
            let sp: u64;
            unsafe {
                asm! {
                    "mrs {}, SP_EL0",
                    out(reg) sp,
                    options(nomem, nostack, preserves_flags)
                }
            }
            println!("otha sp at {:X}", sp);
            match execution.set_context(
                ptr::from_exposed_addr(
                    usize::try_from(arg0).expect("`u64` should always fit into `usize`"),
                ),
                arg1,
                arg2,
            ) {
                Ok(()) => {
                    execution.jump_into_async(ExceptionCode::Resumption, 0);
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
            }
        }
        CallCode::Eret => unreachable!(),
        CallCode::Unblock => {
            let status = u16::try_from(arg0)
                .ok()
                .and_then(|pid| EXECUTIONS.lock().get(pid).map(Execution::unblock))
                .is_some();
            if status {
                success!()
            } else {
                fail!()
            }
        }
        CallCode::Block => {
            execution.block();
            success!()
        }
        CallCode::SendSignal => {
            if let Some(target) = EXECUTIONS.lock().get(arg0.try_into().unwrap()) {
                target.add_signal(execution.pid);
                success!()
            } else {
                fail!()
            }
        }
    }
}

//! System call handlers

use core::{ptr, slice};

use bitfield_struct::bitfield;
use macros::AsBits;

use crate::{memory::PAGE_ALLOCATOR, UART};

#[derive(AsBits, Debug)]
#[repr(u32)]
/// The SVC code for a specific system call
enum CallCode {
    Print = 0x1000,
    Exit = 0x2000,
    AllocPage = 0x3000,
}

#[bitfield(u32)]
pub struct SvcIS {
    #[bits(16)]
    code: CallCode,
    __: u16,
}

#[repr(C)]
/// Return values for system calls, to be stored into registers
pub struct Return {
    /// Whether or not the system call was successful
    is_success: bool,
    /// Any relevant value returned from the syscall
    value: usize,
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

/// The general system call handler; dispatches to more specific handlers in other files
pub extern "C" fn handle(iss: SvcIS, arg0: u64, arg1: u64) -> Return {
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
                // TODO: store this page for the process
                success!(usize::try_from(result.addr())
                    .expect("Page addresses should always fit into a `usize`"))
            } else {
                fail!()
            }
        }
    }
}

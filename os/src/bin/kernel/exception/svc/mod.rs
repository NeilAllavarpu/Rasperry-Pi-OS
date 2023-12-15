//! System call handler
use core::{arch::asm, fmt::Write};

use bitfield_struct::bitfield;
use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::{FromPrimitive, ToPrimitive};

use crate::{impl_u32, memory::PAGE_ALLOCATOR, println, UART};

#[derive(FromPrimitive, ToPrimitive, Debug)]
enum CallCode {
    Print = 0x1000,
    Exit = 0x2000,
    AllocPage = 0x3000,
    Other,
}

#[bitfield(u32)]
pub struct SvcIS {
    #[bits(16)]
    code: CallCode,
    __: u16,
}

#[repr(C)]
pub struct SvcReturn {
    is_success: bool,
    values: (usize),
}

impl From<SvcReturn> for (usize, usize) {
    fn from(value: SvcReturn) -> Self {
        (usize::from(value.is_success), value.values)
    }
}

macro_rules! ret {
    ($is_success:expr) => {{
        SvcReturn {
            is_success: $is_success,
            values: Default::default(),
        }
    }};
    ($is_success:expr, $val:expr) => {{
        SvcReturn {
            is_success: $is_success,
            values: $val,
        }
    }};
}

macro_rules! success {
    () => {
        ret!(true)
    };
    ($val:expr) => {
        ret!(true, $val)
    };
}

macro_rules! fail {
    () => {
        ret!(false)
    };
    ($val:expr) => {
        ret!(false, $val)
    };
}

/// The general system call handler; dispatches to more specific handlers in other files
pub extern "C" fn handle(iss: SvcIS, arg0: u64, arg1: u64) -> SvcReturn {
    match iss.code() {
        CallCode::Exit => {
            todo!("Implement program exits")
        }
        CallCode::Print => {
            let data_ptr = core::ptr::from_exposed_addr(usize::try_from(arg0).unwrap());
            let data_len = usize::try_from(arg1).unwrap();
            // TODO: actually validate pointers
            // SAFETY: If the user is nice...
            let data_bytes = unsafe { core::slice::from_raw_parts(data_ptr, data_len) };
            let uart = UART.get().expect("UART should be initialized by now");
            uart.lock()
                .write_bytes(data_bytes)
                .expect("UART should not fail");
            success!()
        }
        CallCode::AllocPage => {
            if let Some(result) = PAGE_ALLOCATOR.get().unwrap().alloc() {
                // TODO: store this page for the process
                success!(result.addr() as usize)
            } else {
                fail!()
            }
        }
        CallCode::Other => {
            println!("WARNING: Unhandled system call 0x{}", u32::from(iss));
            fail!()
        }
    }
}

impl_u32!(CallCode);

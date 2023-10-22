//! System call handler
use bitfield_struct::bitfield;
use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::{FromPrimitive, ToPrimitive};

use crate::{impl_u32, println, UART};

#[derive(FromPrimitive, ToPrimitive, Debug)]
enum CallCode {
    Print = 0x1000,
    Exit = 0x2000,
    Other,
}

#[bitfield(u32)]
pub struct SvcIS {
    #[bits(16)]
    code: CallCode,
    __: u16,
}

/// The general system call handler; dispatches to more specific handlers in other files
pub fn handle(iss: SvcIS, arg0: u64, arg1: u64) {
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
            UART.lock().write_bytes(data_bytes);
        }
        CallCode::Other => println!("WARNING: Unhandled system call 0x{}", u32::from(iss)),
    }
}

impl_u32!(CallCode);

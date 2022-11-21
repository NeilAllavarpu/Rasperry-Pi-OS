//! Hardware-independent code

#![no_main]
#![no_std]

#[path = "../architecture/architecture.rs"]
mod architecture;

use core::panic::PanicInfo;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    unimplemented!()
}

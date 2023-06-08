#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(asm_const)]
use core::arch::asm;

#[no_mangle]
#[naked]
extern "C" fn _start() {
    const MASK: u64 = !(0xF);
    unsafe {
        asm!(
            "mov x0, sp",
            "and sp, x0, {}",
     "b {}",
     const MASK,
     sym main, 
     options(noreturn))
    }
}

fn main() {
    unsafe {
        (0xFFFF_FFFF_FFFF_1000 as *mut u32).write_volatile('x' as u32);
    }
    loop {}
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    loop {}
}

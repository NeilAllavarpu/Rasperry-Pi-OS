#![no_main]
#![no_std]
#![feature(naked_functions)]

use stdos::os::syscalls;

#[no_mangle]
#[link_section = ".init"]
#[naked]
extern "C" fn _start() -> ! {
    unsafe {
        core::arch::asm! {
            "0: mov sp, 0x10000",
            "b {start}",
            start = sym start,
            options(noreturn)
        }
    }
}

extern "C" fn start() -> ! {
    syscalls::write("Hello from usermode!\n".as_bytes());
    syscalls::exit()
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

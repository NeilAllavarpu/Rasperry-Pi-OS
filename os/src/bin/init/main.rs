#![no_main]
#![no_std]
#![feature(naked_functions)]

use core::{arch::asm, fmt::Write};

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

struct Stdout;
impl core::fmt::Write for Stdout {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        syscalls::write(s.as_bytes());
        Ok(())
    }
}

extern "C" fn start() -> ! {
    let mut stdout = Stdout {};
    stdout.write_str("Hello from usermode!\n");
    loop {
        core::hint::spin_loop();
    }
    syscalls::write("Unreachable!\n".as_bytes());
    syscalls::exit()
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

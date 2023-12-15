#![no_main]
#![no_std]
#![feature(naked_functions)]
#![feature(asm_const)]
// use core::{arch::asm, fmt::Wr`ite};

static MESSAGE: &'static str = "Hello from UART";

#[no_mangle]
#[link_section = ".init"]
#[naked]
extern "C" fn _start() -> ! {
    unsafe {
        core::arch::asm! {
            "adr x0, {MESSAGE_ADDR}",
            "mov x1, {MESSAGE_LEN}",
            "svc #0x1000",
            "svc #0x2000",
            MESSAGE_ADDR = sym MESSAGE,
            MESSAGE_LEN = const 14,
            // in ("x0") bytes.as_ptr(),
            // in ("x1") bytes.len(),
            options(noreturn)
        }
    }
}

// struct Stdout;
// impl core::fmt::Write for Stdout {
//     fn write_str(&mut self, s: &str) -> core::fmt::Result {
//         syscalls::write(s.as_bytes());
//         Ok(())
//     }
// }

// extern "C" fn start() -> ! {
//     let mut stdout = Stdout {};
//     stdout.write_str("Hello from usermode!\n");
//     loop {
//         core::hint::spin_loop();
//     }
//     syscalls::write("Unreachable!\n".as_bytes());
//     syscalls::exit()
// }

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

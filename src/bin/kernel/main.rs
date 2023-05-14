//! A Raspberry Pi Operating System
#![no_main]
#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::complexity)]
#![warn(clippy::correctness)]
#![warn(clippy::pedantic)]
#![warn(clippy::suspicious)]
#![warn(clippy::perf)]
#![warn(clippy::style)]
#![allow(clippy::blanket_clippy_restriction_lints)]
#![warn(clippy::restriction)]
#![feature(naked_functions)]
#![feature(panic_info_message)]
#![feature(format_args_nl)]
#![feature(asm_const)]
#![feature(generic_arg_infer)]
#![feature(strict_provenance)]
#![feature(pointer_byte_offsets)]
#![allow(clippy::inline_asm_x86_intel_syntax)]
#![allow(clippy::mod_module_files)]

mod boot;
use core::fmt::{Arguments, Result, Write};

fn write_out(c: u8) {
    unsafe { (0xFFFF_FFFF_FFFF_1000 as *mut u32).write_volatile(c as u32) };
}

struct Uart {}
impl Write for Uart {
    fn write_str(&mut self, s: &str) -> Result {
        for c in s.bytes() {
            write_out(c)
        }
        Ok(())
    }
}

extern "C" fn init() {
    println!("Hello world");
    loop {}
}

fn _print(args: Arguments) {
    let _ = (Uart {}).write_fmt(args);
}
/// Print to serial output
// <https://doc.rust-lang.org/src/std/macros.rs.html>
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::_print(format_args!($($arg)*)));
}

/// Print, with a newline, to serial output
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ({
        $crate::_print(format_args_nl!($($arg)*));
    })
}

#[cfg(not(test))]
/// Upon panics, print the location of the panic and any associated message,
/// then shutdown
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let (file, line, column) = match info.location() {
        Some(loc) => (loc.file(), loc.line(), loc.column()),
        _ => ("Unknown file", 0, 0),
    };

    println!(
        "PANIC at {}:{}:{}\n{}",
        file,
        line,
        column,
        info.message().unwrap_or(&format_args!("")),
    );

    loop {}
}

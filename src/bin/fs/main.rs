#![no_std]
#![no_main]
#![feature(asm_const)]
#![feature(naked_functions)]
#![feature(const_nonnull_new)]
#![feature(const_option)]
#![feature(format_args_nl)]
#![feature(stdsimd)]
#![feature(panic_info_message)]

use stdos::os::vm::ADDRESS_SPACE;

#[no_mangle]
extern "C" fn main() {
    unsafe {
        ADDRESS_SPACE
            .lock()
            .map_range(0x1_0000, 0x3F20_0000, 0x1_0000, true, false, true);
        core::arch::asm!("dsb OSHST", "isb");
    };
    println!("Hello world");
}

struct Uart;

impl core::fmt::Write for Uart {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for c in s.bytes() {
            unsafe {
                (0x1_1000 as *mut u32).write_volatile(c as u32);
            }
        }
        Ok(())
    }
}

/// Writes the given information out to the serial output
pub fn _print(args: core::fmt::Arguments) {
    use core::fmt::Write;
    (Uart {}).write_fmt(args);
}
/// Discards the input arguments
pub fn _unused(_args: core::fmt::Arguments) {}

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

    loop {
        unsafe { core::arch::aarch64::__wfi() }
    }
}

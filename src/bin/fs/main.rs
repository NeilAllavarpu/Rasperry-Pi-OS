#![no_std]
#![no_main]
#![feature(asm_const)]
#![feature(naked_functions)]
#![feature(const_nonnull_new)]
#![feature(const_option)]
#![feature(format_args_nl)]
#![feature(stdsimd)]
#![feature(panic_info_message)]
#![feature(lint_reasons)]
#![feature(int_roundings)]

use stdos::os::vm::ADDRESS_SPACE;

mod emmc;
//use emmc::Emmc;

const EMMC_VA: usize = 0x2_0000;
const EMMC_PA: u64 = 0x3F30_0000;

#[no_mangle]
extern "C" fn main() {
    unsafe {
        let mut x = ADDRESS_SPACE.lock();
        x.map_range(0x1_0000, 0x3F20_0000, 0x1_0000, true, false, true);
        x.map_range(
            EMMC_VA.try_into().unwrap(),
            EMMC_PA,
            0x1_0000,
            true,
            false,
            true,
        );
        core::arch::asm!("dsb OSHST", "isb");
    }; /*
       let mut emmc = Emmc::new(EMMC_VA);
       emmc.init();
       let mut buf = [0xFF_u8; 512];
       emmc.read_blk(0, &mut buf);*/
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
    use qemu_exit::QEMUExit;
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

    qemu_exit::AArch64::new().exit(0);
}

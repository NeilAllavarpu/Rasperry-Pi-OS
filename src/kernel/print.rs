/// Writes the given information out to the serial output
pub fn _print(args: core::fmt::Arguments) {
    use crate::{board, kernel::Serial};
    board::serial().write_fmt(args);
}
/// Discards the input arguments
pub fn _unused(_args: core::fmt::Arguments) {}

/// Print to serial output
// <https://doc.rust-lang.org/src/std/macros.rs.html>
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::kernel::print::_print(format_args!($($arg)*)));
}

/// Print, with a newline, to serial output
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ({
        $crate::kernel::print::_print(format_args_nl!($($arg)*));
    })
}

/// Prints info prefixed with thread ID and timestamp
#[macro_export]
#[cfg(feature = "verbose")]
macro_rules! log {
    ($string:expr) => ({
        use core::time::Duration;
        let timestamp: Duration = $crate::architecture::time::now();

        $crate::kernel::print::_print(format_args_nl!(
            concat!("[T {}, {}.{:03}s] ", $string),
            $crate::architecture::thread::me(|me| me.id),
            timestamp.as_secs(),
            timestamp.subsec_millis(),
        ));
    });
    ($format_string:expr, $($arg:tt)*) => ({
        use core::time::Duration;
        let timestamp: Duration = $crate::architecture::time::now();

        $crate::kernel::print::_print(format_args_nl!(
            concat!("[T {}, {}.{:03}s] ", $format_string),
            $crate::architecture::thread::me(|me| me.id),
            timestamp.as_secs(),
            timestamp.subsec_millis(),
            $($arg)*
        ));
    })
}

/// A no-op logging mechanism
#[cfg(not(feature = "verbose"))]
#[macro_export]
macro_rules! log {
    ($string:expr) => ({
        $crate::kernel::print::_unused(format_args_nl!(
             $string,
        ));
    });
    ($format_string:expr, $($arg:tt)*) => ({
        $crate::kernel::print::_unused(format_args_nl!(
            $format_string,
            $($arg)*
        ));
    })
}

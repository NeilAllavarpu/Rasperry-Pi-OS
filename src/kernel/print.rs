pub fn _print(args: core::fmt::Arguments) {
    crate::serial::get().write_format_string(args);
}

/// Print to serial output
// <https://doc.rust-lang.org/src/std/macros.rs.html>
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::print::_print(format_args!($($arg)*)));
}

/// Print, with a newline, to serial output
#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ({
        $crate::print::_print(format_args_nl!($($arg)*));
    })
}

/// Prints info prefixed with thread ID and timestamp
#[macro_export]
macro_rules! log {
    ($string:expr) => ({
        use core::time::Duration;
        let timestamp: Duration = crate::now().into();


        $crate::print::_print(format_args_nl!(
            concat!("[TH {:2}, {}.{:03}s] ", $string),
            crate::architecture::thread_id(),
            timestamp.as_secs(),
            timestamp.subsec_millis(),
        ));
    });
    ($format_string:expr, $($arg:tt)*) => ({
        use core::time::Duration;
        let timestamp: Duration = crate::now().into();

        $crate::print::_print(format_args_nl!(
            concat!("[TH {:2}, {}.{:03}s] ", $format_string),
            crate::architecture::thread_id(),
            timestamp.as_secs(),
            timestamp.subsec_millis(),
            $($arg)*
        ));
    })
}

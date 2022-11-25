#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use crate::{architecture, log};
    let (file, line, column) = match info.location() {
        Some(loc) => (loc.file(), loc.line(), loc.column()),
        _ => ("Unknown file", 0, 0),
    };

    log!(
        "PANIC at {}:{}:{}\n{}",
        file,
        line,
        column,
        info.message().unwrap_or(&format_args!("")),
    );

    // Shutdown badly
    architecture::shutdown(1);
}

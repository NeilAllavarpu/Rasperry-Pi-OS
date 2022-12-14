/// Upon panics, print the location of the panic and any associated message,
/// then shutdown
#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    use crate::{architecture, println};
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

    // Shutdown badly
    architecture::shutdown(1);
}

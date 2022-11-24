//! The initialization sequences

#![no_main]
#![no_std]
#![feature(format_args_nl)]
#![feature(panic_info_message)]
#![feature(const_option)]
#![feature(once_cell)]
#![feature(result_option_inspect)]

#[path = "../architecture/architecture.rs"]
mod architecture;
#[path = "../board/board.rs"]
mod board;

mod exception;
mod mutex;
mod once;
mod per_core;
mod print;
mod serial;
mod timer;

use core::time::Duration;

pub use mutex::Mutex;
pub use once::*;
pub use per_core::PerCore;
pub use serial::Serial;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let (file, line, column) = match info.location() {
        Some(loc) => (loc.file(), loc.line(), loc.column()),
        _ => ("Unknown file", 0, 0),
    };

    log!(
        "PANIC on core {} (at {}:{}:{})\n{}",
        architecture::machine::core_id(),
        file,
        line,
        column,
        info.message().unwrap_or(&format_args!("")),
    );

    // Shutdown badly
    architecture::shutdown(1);
}

/// Global initialization of the system
#[no_mangle]
fn init() -> ! {
    if architecture::machine::core_id() == 0 {
        // This is the global initialization sequence; it should only run once
        call_once!();

        // Initialize architecture-specific items
        architecture::init();

        // Initialize board-specific items
        board::init();

        log!("What just happened? Why am I here?");
        architecture::CONFIG.get().log();

        board::wake_all_cores();
    }

    per_core_init()
}

/// Per-core initialization
#[no_mangle]
fn per_core_init() -> ! {
    // Must only be called once per core
    call_once_per_core!();

    // Temporarily set thread ID to match core ID, for logs
    architecture::machine::set_thread_id(architecture::machine::core_id() as u64);

    // Make sure this is running in EL1
    assert_eq!(
        architecture::exception::exception_level(),
        exception::PrivilegeLevel::Kernel,
        "The kernel must be running with kernel privileges"
    );

    architecture::per_core_init();

    log!("Enabling interrupts, I'm scared...");
    architecture::exception::enable();

    timer::wait_at_least(Duration::from_secs(1));
    architecture::shutdown(0);
}

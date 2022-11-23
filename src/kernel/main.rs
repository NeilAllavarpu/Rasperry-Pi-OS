//! The initialization sequences

#![no_main]
#![no_std]
#![feature(format_args_nl)]
#![feature(panic_info_message)]
#![feature(const_option)]

#[path = "../architecture/architecture.rs"]
mod architecture;
#[path = "../board/board.rs"]
mod board;

mod exception;
mod macros;
mod mutex;
mod per_core;
mod print;
mod serial;
mod timer;

pub use exception::*;
pub use mutex::*;
pub use per_core::*;
pub use timer::*;

#[panic_handler]
fn panic(info: &core::panic::PanicInfo) -> ! {
    let (file, line, column) = match info.location() {
        Some(loc) => (loc.file(), loc.line(), loc.column()),
        _ => ("Unknown file", 0, 0),
    };

    println!(
        "*** PANIC on core {} (at {}:{}:{}):\n  {}",
        architecture::core_id(),
        file,
        line,
        column,
        info.message().unwrap_or(&format_args!("")),
    );

    loop {
        aarch64_cpu::asm::wfe();
    }
}

/// Global initialization of the system
#[no_mangle]
fn init() -> ! {
    // Must only be run once
    call_once!();

    // Make sure this is running in EL1
    assert_eq!(
        architecture::exception_level(),
        exception::PrivilegeLevel::Kernel
    );

    // Initialize board-specific items
    board::init();

    println!("What just happened? Why am I here?");

    board::wake_all_cores();
    wait_at_least(core::time::Duration::new(2, 0));
    per_core_init()
}

/// Per-core initialization
#[no_mangle]
fn per_core_init() -> ! {
    // Must only be called once per core
    static IS_FIRST_INIT: PerCore<bool> = PerCore::new(true);
    assert!(IS_FIRST_INIT.with_current(|is_first| core::mem::replace(is_first, false)));
    println!(
        "*** Per-core sequence loaded on core {} ***",
        architecture::core_id()
    );

    todo!()
}

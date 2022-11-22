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

mod mutex;
pub use mutex::Mutex;
mod per_core;
pub use per_core::PerCore;
mod print;
mod serial;
mod timer;

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

#[no_mangle]
fn init() -> ! {
    // The main init sequence must only run once, globally
    #[cfg(debug_assertions)]
    {
        use core::sync::atomic::{AtomicBool, Ordering::AcqRel};
        static IS_FIRST_INIT: AtomicBool = AtomicBool::new(true);
        assert!(IS_FIRST_INIT.swap(false, AcqRel));
        println!("*** Init sequence loaded ***");
    }

    board::wake_all_cores();
    timer::wait_at_least(core::time::Duration::new(2, 0));
    per_core_init()
}

#[no_mangle]
fn per_core_init() -> ! {
    // The per-core init sequence must only run once per core
    #[cfg(debug_assertions)]
    {
        static IS_FIRST_INIT: PerCore<bool> = PerCore::new(true);
        assert!(IS_FIRST_INIT.with_current(|is_first| core::mem::replace(is_first, false)));
        println!("*** Per-core sequence loaded on core {} ***", architecture::core_id());
    }
    todo!()
}

use crate::{architecture, board, call_once, call_once_per_core, kernel, log};

extern "Rust" {
    /// The `kernel_init()` for unit tests.
    fn kernel_main();
}

/// Global initialization of the system
#[no_mangle]
pub extern "C" fn init() -> ! {
    if architecture::machine::core_id() == 0 {
        // This is the global initialization sequence; it should only run once
        call_once!();

        // Create the heap
        kernel::heap::init();

        // Initialize architecture-specific items
        architecture::init();

        // Initialize board-specific items
        board::init();

        log!("What just happened? Why am I here?");
        architecture::CONFIG.get().log();
        kernel::thread::init();

        kernel::thread::schedule(kernel::thread::TCB::new(|| unsafe { kernel_main() }));

        board::wake_all_cores();
    }

    per_core_init()
}

/// Per-core initialization
fn per_core_init() -> ! {
    // Must only be called once per core
    call_once_per_core!();

    // Make sure this is running in EL1
    assert_eq!(
        architecture::exception::exception_level(),
        kernel::exception::PrivilegeLevel::Kernel,
        "The kernel must be running with kernel privileges"
    );

    architecture::per_core_init();
    kernel::thread::per_core_init();

    log!("Enabling interrupts, I'm scared...");
    architecture::exception::enable();

    kernel::thread::idle_loop();
    unreachable!();
}

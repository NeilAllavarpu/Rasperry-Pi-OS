use crate::{
    architecture::{self, thread::me},
    board, call_once, call_once_per_core, kernel, log, thread,
};

extern "Rust" {
    /// The `kernel_init()` for unit tests.
    fn kernel_main();
}

/// Global initialization of the system
#[no_mangle]
pub extern "C" fn init() -> ! {
    // SAFETY: This should only run once
    unsafe {
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
            kernel::thread::init();

            kernel::thread::schedule(thread!(||
                // SAFETY: `kernel_main` is appropriately defined by the build system
                   kernel_main()));

            board::wake_all_cores();
        }

        per_core_init()
    }
}

/// Per-core initialization
/// # Safety
/// Must only be called once per core
unsafe fn per_core_init() -> ! {
    call_once_per_core!();
    // Make sure this is running in EL1
    assert_eq!(
        architecture::exception::el(),
        kernel::exception::PrivilegeLevel::Kernel,
        "The kernel must be running with kernel privileges"
    );

    // SAFETY: Only runs once per core
    unsafe {
        kernel::thread::per_core_init();
        architecture::per_core_init();
    }

    log!("Enabling interrupts, I'm scared...");
    // SAFETY: This is the first time we are enabling exceptions
    unsafe {
        architecture::exception::enable();
    }

    // SAFETY: It is safe to run the idle threads because the idle threads
    // have not been run yet, and will not be run any other way
    unsafe { me(|me| me.run()) }
}

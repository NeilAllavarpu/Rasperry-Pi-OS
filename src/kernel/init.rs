use core::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use aarch64_cpu::asm::{sev, wfe};

use crate::{architecture, board, call_once, kernel, log, thread};

extern "Rust" {
    /// The `kernel_init()` for unit tests.
    fn kernel_main();
}

/// Global initialization of the system
#[no_mangle]
pub extern "C" fn init() -> ! {
    /// Whether or not initialization is complete
    static MAIN_INIT_DONE: AtomicBool = AtomicBool::new(false);
    // SAFETY: This should only run once
    unsafe {
        if architecture::machine::core_id() == 0 {
            // This is the global initialization sequence; it should only run once
            call_once!();

            // Create the heap
            kernel::heap::init();

            thread::init();

            // Initialize architecture-specific items
            architecture::init();

            // Initialize board-specific items
            board::init();

            log!("What just happened? Why am I here?");

            MAIN_INIT_DONE.store(true, Ordering::Release);
            sev();
        } else {
            while !MAIN_INIT_DONE.load(Ordering::Acquire) {
                wfe();
            }
        }

        per_core_init()
    }
}

/// Per-core initialization
/// # Safety
/// Must only be called once per core
unsafe fn per_core_init() -> ! {
    /// Number of cores that finished initialization
    static FINISHED_CORES: AtomicUsize = AtomicUsize::new(0);
    /// Number of cores
    const NUM_CORES: usize = 4;

    // Make sure this is running in EL1
    assert_eq!(
        architecture::exception::el(),
        kernel::exception::PrivilegeLevel::Kernel,
        "The kernel must be running with kernel privileges"
    );

    // SAFETY: Only runs once per core
    unsafe {
        thread::per_core_init();
        architecture::per_core_init();
    }

    log!("Enabling interrupts, I'm scared...");
    // SAFETY: This is the first time we are enabling exceptions
    unsafe {
        architecture::exception::enable();
    }

    if FINISHED_CORES.fetch_add(1, Ordering::Relaxed) + 1 == NUM_CORES {
        thread::schedule(thread::spawn(||
            // SAFETY: `kernel_main` is appropriately defined by the build system
            unsafe { kernel_main(); }));
    }

    thread::idle_loop();
}

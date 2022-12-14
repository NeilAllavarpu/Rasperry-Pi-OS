/// Invokes a system shutdown, as appropriate
///
/// In QEMU, this exits QEMU
pub fn shutdown(exit_code: u32) -> ! {
    use crate::{architecture, kernel, log};
    use aarch64_cpu::asm::wfi;
    use core::sync::atomic::{AtomicBool, Ordering};
    use qemu_exit::QEMUExit;

    /// Stores whether or not a shutdown has already been called
    static SHUTDOWN_CALLED: AtomicBool = AtomicBool::new(false);
    if SHUTDOWN_CALLED.swap(true, Ordering::Relaxed) {
        loop {
            // Don't double-shut down, just enter low power state
            wfi();
        }
    }

    log!(
        "Core {}: shutdown ({})",
        architecture::machine::core_id(),
        exit_code
    );
    // SAFETY: This is at shutdown and used only for a logging estimate
    unsafe {
        kernel::heap::log_allocator();
    }
    qemu_exit::AArch64::new().exit(exit_code);
}

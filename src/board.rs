/// UART (PL011) support
mod uart;
pub use uart::serial;

use crate::call_once;

extern "C" {
    // Must not be run on concurrent execution paths with the same core ID
    fn _per_core_init() -> !;
}

/// Wakes up all cores and runs their per-core initialization sequences
/// # Safety
/// Must only be called once
#[allow(dead_code)]
pub unsafe fn wake_all_cores() {
    call_once!();
    #[allow(clippy::as_conversions)]
    // SAFETY: These addresses are taken from the spec for Raspbeery Pi 4
    unsafe {
        // Tell the cores to start running the per core init sequence
        core::ptr::write_volatile(0xE0 as *mut unsafe extern "C" fn() -> !, _per_core_init);
        core::ptr::write_volatile(0xE8 as *mut unsafe extern "C" fn() -> !, _per_core_init);
        core::ptr::write_volatile(0xF0 as *mut unsafe extern "C" fn() -> !, _per_core_init);
    }
    // make sure the cores are notified to wake up
    aarch64_cpu::asm::sev();
}

/// Board-specific initialization sequences
/// # Safety
/// Must be initialized only once
pub unsafe fn init() {
    call_once!();
    serial().init();
}

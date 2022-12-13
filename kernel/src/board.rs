mod uart;
pub use uart::serial;

use crate::call_once;

extern "C" {
    // Must not be run on concurrent execution paths with the same core ID
    fn _per_core_init() -> !;
}

#[allow(dead_code)]
pub fn wake_all_cores() {
    // Not invalid, but we shouldn't be trying to call this multiple times
    call_once!();
    unsafe {
        // Tell the cores to start running the per core init sequence
        core::ptr::write_volatile(0xE0 as *mut unsafe extern "C" fn() -> !, _per_core_init);
        core::ptr::write_volatile(0xE8 as *mut unsafe extern "C" fn() -> !, _per_core_init);
        core::ptr::write_volatile(0xF0 as *mut unsafe extern "C" fn() -> !, _per_core_init)
    }
    // make sure the cores are notified to wake up
    aarch64_cpu::asm::sev();
}

pub fn init() {
    // Must only be initialized once
    call_once!();
    serial().init();
}

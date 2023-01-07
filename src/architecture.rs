/// Exception-related information: masks and triggering/registering
pub mod exception;
/// Basic exception handlers
mod exception_handlers;
/// Miscellaneous machine functions
pub mod machine;
/// System shutdown functionality
mod shutdown;
/// Timer support
pub mod time;

pub use shutdown::shutdown;

// The boot sequence
core::arch::global_asm!(include_str!("architecture/boot.s"));

/// Architecture-wide initialization
/// # Safety
/// Must be initialized only once
pub unsafe fn init() {
    crate::call_once!();
    time::init();
    exception::init();
}

/// Per-core architecture-wide initialization
/// # Safety
/// Must only be called once per core
pub unsafe fn per_core_init() {
    crate::call_once_per_core!();
    exception::per_core_init();
}

#[cfg(not(target_pointer_width = "64"))]
compile_error!("Only 64-bit platforms are currently supported");

/// Converts a `usize` into a `u64`
#[cfg(target_pointer_width = "64")]
#[allow(clippy::as_conversions)]
pub const fn usize_to_u64(n: usize) -> u64 {
    // SAFETY: Because `usize` is 64 bit, this is safe
    n as u64
}

/// Converts a `u64` into a `usize`
#[cfg(target_pointer_width = "64")]
#[allow(clippy::as_conversions)]
#[allow(clippy::cast_possible_truncation)]
pub const fn u64_to_usize(n: u64) -> usize {
    // SAFETY: Because `usize` is 64 bit, this is safe
    n as usize
}

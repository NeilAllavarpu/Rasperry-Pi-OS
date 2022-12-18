use core::sync::atomic::AtomicBool;

/// Ensures that the given function is only called once
/// Panics if run more than once
#[allow(clippy::module_name_repetitions)]
#[macro_export]
macro_rules! call_once {
    () => {{
        use core::sync::atomic::{AtomicBool, Ordering};
        static IS_FIRST_INVOCATION: AtomicBool = AtomicBool::new(false);
        assert!(!IS_FIRST_INVOCATION.swap(true, Ordering::Relaxed))
    }};
}

/// Helper function to create idle threads.
/// For some reason, an inline closure is not being considered `const`...
///
/// TODO: Replace this with `const` closures if those are made available
pub const fn _create_idle_percore() -> AtomicBool {
    AtomicBool::new(true)
}

/// Ensures that the given function is only called once per core
/// Panics if run more than once on any given core
#[macro_export]
macro_rules! call_once_per_core {
    () => {{
        use core::sync::atomic::{AtomicBool, Ordering};
        use $crate::kernel::PerCore;
        static IS_CORE_FIRST_INVOCATION: PerCore<AtomicBool> =
            PerCore::new($crate::macros::once::_create_idle_percore);
        assert!(IS_CORE_FIRST_INVOCATION
            .current()
            .swap(false, Ordering::Relaxed));
    }};
}

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

/// Ensures that the given function is only called once per core
/// Panics if run more than once on any given core
#[macro_export]
macro_rules! call_once_per_core {
    () => {{
        use $crate::kernel::PerCore;
        static IS_CORE_FIRST_INVOCATION: PerCore<bool> = PerCore::new(true);
        assert!(
            IS_CORE_FIRST_INVOCATION.with_current(|is_first| core::mem::replace(is_first, false))
        )
    }};
}

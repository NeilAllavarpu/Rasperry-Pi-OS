/// Ensures that the given function is only called once
/// Panics if run more than once
#[macro_export]
macro_rules! call_once {
    () => {{
        use core::sync::atomic::{AtomicBool, Ordering::AcqRel};
        static IS_FIRST_INVOCATION: AtomicBool = AtomicBool::new(true);
        assert!(IS_FIRST_INVOCATION.swap(false, AcqRel));
    }};
}

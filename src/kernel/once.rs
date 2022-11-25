use core::cell::OnceCell;

/// Can only be set once
#[derive(Debug)]
pub struct SetOnce<T> {
    inner: OnceCell<T>,
}

impl<T> SetOnce<T> {
    /// Creates an unset SetOnce
    pub const fn new() -> Self {
        Self {
            inner: OnceCell::new(),
        }
    }

    /// Sets the value
    ///
    /// Panics if the value is already set
    pub fn set(&self, value: T) -> () {
        assert!(self.inner.set(value).is_ok());
    }

    /// Gets the value
    ///
    /// Panics if the value is not yet set
    pub fn get(&self) -> &T {
        self.inner
            .get()
            .expect("Should not access before being set")
    }
}

unsafe impl<T> Sync for SetOnce<T> {}
unsafe impl<T> Send for SetOnce<T> {}

/// Ensures that the given function is only called once
/// Panics if run more than once
#[macro_export]
macro_rules! call_once {
    () => {{
            use core::sync::atomic::{AtomicBool, Ordering::AcqRel};
            static IS_FIRST_INVOCATION: AtomicBool = AtomicBool::new(false);
            assert!(!IS_FIRST_INVOCATION.swap(true, AcqRel))
        }};
}

/// Ensures that the given function is only called once per core
/// Panics if run more than once on any given core
#[macro_export]
macro_rules! call_once_per_core {
    () => {{
        use crate::kernel::PerCore;
        static IS_CORE_FIRST_INVOCATION: PerCore<bool> = PerCore::new(true);
        assert!(
            IS_CORE_FIRST_INVOCATION.with_current(|is_first| core::mem::replace(is_first, false))
        )
    }};
}

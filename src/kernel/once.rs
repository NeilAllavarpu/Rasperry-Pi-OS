use core::{cell::OnceCell, ops::Deref};

/// A wrapper for an object that can only be set once
#[derive(Debug)]
pub struct SetOnce<T> {
    /// The enclosed value
    inner: OnceCell<T>,
}

impl<T> SetOnce<T> {
    /// Creates an unset `SetOnce`
    pub const fn new() -> Self {
        Self {
            inner: OnceCell::new(),
        }
    }

    /// Sets the value
    /// # Safety
    /// Should only be called once to set the value
    /// Must not be accessed at all before or during this call
    pub unsafe fn set(&self, value: T) {
        assert!(self.inner.set(value).is_ok());
    }
}

impl<T> Deref for SetOnce<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.inner
            .get()
            .expect("Should not access before being set")
    }
}

// SAFETY: Because the value is only set once, safely, if the underlying type is Sync, then this is also Sync
unsafe impl<T: Sync> Sync for SetOnce<T> {}
// SAFETY: Because the value is only set once, safely, if the underlying type is Send, then this is also Send
unsafe impl<T: Send> Send for SetOnce<T> {}

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
        use $crate::kernel::PerCore;
        static IS_CORE_FIRST_INVOCATION: PerCore<bool> = PerCore::new(true);
        assert!(
            IS_CORE_FIRST_INVOCATION.with_current(|is_first| core::mem::replace(is_first, false))
        )
    }};
}

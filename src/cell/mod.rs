use core::cell::SyncUnsafeCell;
use core::ops::Deref;
use core::ptr::NonNull;

/// A cell that may only be initialized once and exactly once
pub struct InitCell<T>(SyncUnsafeCell<Option<T>>);

impl<T> InitCell<T> {
    /// Creates a new, empty `InitCell`
    pub const fn new() -> Self {
        Self(SyncUnsafeCell::new(None))
    }

    fn get_pointer(&self) -> NonNull<Option<T>> {
        // SAFETY: This pointer is never null
        unsafe { NonNull::new_unchecked(self.0.get()) }
    }

    /// Sets the `InitCell` to the given value
    ///
    /// # Safety
    ///
    /// The `InitCell` must be fully set before anyone attempts to read its value, and may only be
    /// set once.
    ///
    /// # Panics
    ///
    /// Panics if this method is called multiple times and the executions are non-concurrent. May
    /// not panic if this method is called concurrently
    pub unsafe fn set(&self, value: T) {
        // SAFETY: The caller guarantees that we have exclusive, mutable access to the cell
        let inner = unsafe { self.get_pointer().as_mut() };
        assert!(inner.is_none());
        *inner = Some(value);
    }
}

impl<T> Deref for InitCell<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: By assumption, the `InitCell` has already been constructed mutably, so no
        // mutable references are possible`
        unsafe { self.get_pointer().as_ref() }
            .as_ref()
            .expect("`InitCell` should be initialized before access")
    }
}

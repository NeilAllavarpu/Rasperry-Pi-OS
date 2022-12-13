use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, Ordering};

const MAX_CORES: usize = 4;

struct PerCoreEntry<T> {
    is_available: AtomicBool,
    value: T,
}

impl<T> PerCoreEntry<T> {
    const fn new(initial: T) -> Self {
        Self {
            is_available: AtomicBool::new(true),
            value: initial,
        }
    }
}

/// Provides access to a list of items per-core
pub struct PerCoreInner<T> {
    data: [PerCoreEntry<T>; MAX_CORES],
}

impl<T: Copy> PerCoreInner<T> {
    pub const fn new_copy(initial: T) -> Self {
        Self {
            // TODO: Is there a better way to initialize this without copy-paste?
            data: [
                PerCoreEntry::new(initial),
                PerCoreEntry::new(initial),
                PerCoreEntry::new(initial),
                PerCoreEntry::new(initial),
            ],
        }
    }
}

impl<T> PerCoreInner<T> {
    pub fn new_from_array(initial: [T; MAX_CORES]) -> Self {
        Self {
            // TODO: Is there a better way to initialize this without copy-paste?
            data: initial.map(PerCoreEntry::new),
        }
    }

    /// Runs the given function with a mutable reference
    /// to the current core's value
    ///
    /// Prevents the current execution from being switched to another core
    /// while using the core's value
    pub fn with_current<'a, R>(&'a mut self, f: impl FnOnce(&'a mut T) -> R) -> R {
        let core_id: usize = crate::architecture::machine::core_id().into();
        assert!(core_id < MAX_CORES);
        let entry: &mut PerCoreEntry<T> = &mut self.data[core_id];
        // make sure the entry is not already in use, and claim it
        assert!(entry.is_available.swap(false, Ordering::AcqRel));
        let result: R = f(&mut entry.value);
        // release the entry
        entry.is_available.store(true, Ordering::Release);
        result
    }
}

/// Provides access to a list of items per-core
pub struct PerCore<T> {
    inner: UnsafeCell<PerCoreInner<T>>,
}

impl<T> PerCore<T> {
    /// Creates a default-initialized `PerCore` struct
    /// that is initializable at compile time
    pub fn new_from_array(initial: [T; MAX_CORES]) -> Self {
        Self {
            inner: UnsafeCell::new(PerCoreInner::new_from_array(initial)),
        }
    }

    /// Runs the given function with a mutable reference
    /// to the current core's value
    ///
    /// Prevents the current execution from being switched to another core
    /// while using the core's value
    pub fn with_current<'a, R>(&'a self, f: impl FnOnce(&'a mut T) -> R) -> R {
        unsafe { &mut *self.inner.get() }.with_current(f)
    }
}

impl<T: Copy> PerCore<T> {
    /// Creates a default-initialized `PerCore` struct
    /// that is initializable at compile time
    pub const fn new(initial: T) -> Self {
        Self {
            inner: UnsafeCell::new(PerCoreInner::new_copy(initial)),
        }
    }
}

unsafe impl<T> Send for PerCore<T> {}
unsafe impl<T> Sync for PerCore<T> {}

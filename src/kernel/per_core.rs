use crate::architecture;
use core::cell::UnsafeCell;

/// The maximum possible number of cores supported
const MAX_CORES: usize = 4;

/// Provides access to a list of items per-core
struct PerCoreInner<T> {
    /// The underlying data
    data: [T; MAX_CORES],
}

impl<T: Copy> PerCoreInner<T> {
    /// Creates a new `PerCore` by copying the given value
    pub const fn new_copy(initial: T) -> Self {
        Self {
            // TODO: Is there a better way to initialize this without copy-paste?
            data: [initial; 4],
        }
    }
}

impl<T> PerCoreInner<T> {
    /// Creates a new `PerCore` from the initialization array
    pub fn new_from_array(initial: [T; MAX_CORES]) -> Self {
        Self {
            // TODO: Is there a better way to initialize this without copy-paste?
            data: initial,
        }
    }

    /// Runs the given function with a mutable reference
    /// to the current core's value
    ///
    /// Prevents the current execution from being switched to another core
    /// while using the core's value
    pub fn with_current<'a, R>(&'a mut self, f: impl FnOnce(&'a mut T) -> R) -> R {
        let result: R = f.call_once((self
            .data
            .get_mut(usize::from(architecture::machine::core_id()))
            .expect("Core ID was out of bounds"),));
        result
    }
}

/// Provides access to a list of items per-core
pub struct PerCore<T> {
    /// The interior PerCore object
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
        // SAFETY:
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

// SAFETY: Because objects are only accessed one core at a time, and are
// non-preemptible while doing so, only one thread can access a given element
// at any time, so mutual exclusion is enforced
unsafe impl<T> Send for PerCore<T> {}
// SAFETY: Because objects are only accessed one core at a time, and are
// non-preemptible while doing so, only one thread can access a given element
// at any time, so mutual exclusion is enforced
unsafe impl<T> Sync for PerCore<T> {}

use core::cell::UnsafeCell;

const NUM_CORES: usize = 4;

#[cfg(debug_assertions)]
mod per_core_debug {
    use super::NUM_CORES;
    use core::sync::atomic::{AtomicBool, Ordering};

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
        data: [PerCoreEntry<T>; NUM_CORES],
    }

    impl<T: Copy> PerCoreInner<T> {
        /// Creates a default-initialized PerCore struct
        /// that is initializable at compile time
        pub const fn new(initial: T) -> Self {
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

        /// Runs the given function with a mutable reference
        /// to the current core's value
        ///
        /// Prevents the current execution from being switched to another core
        /// while using the core's value
        pub fn with_current<'a, R>(&'a mut self, f: impl FnOnce(&'a mut T) -> R) -> R {
            let core_id: usize = crate::architecture::core_id() as usize;
            assert!(core_id < NUM_CORES);
            let entry: &mut PerCoreEntry<T> = &mut self.data[core_id];
            // make sure the entry is not already in use, and claim it
            assert!(entry.is_available.swap(false, Ordering::AcqRel));
            let result: R = f(&mut entry.value);
            // release the entry
            entry.is_available.store(true, Ordering::Release);
            result
        }
    }
}

#[cfg(not(debug_assertions))]
mod per_core_unchecked {
    use super::NUM_CORES;

    /// Provides access to a list of items per-core
    pub struct PerCoreInner<T> {
        data: [T; NUM_CORES],
    }

    impl<T: Copy> PerCoreInner<T> {
        /// Creates a default-initialized PerCore struct
        /// that is initializable at compile time
        pub const fn new(initial: T) -> Self {
            Self {
                data: [initial; NUM_CORES],
            }
        }

        /// Runs the given function with a mutable reference
        /// to the current core's value
        ///
        /// Prevents the current execution from being switched to another core
        /// while using the core's value
        pub fn with_current<'a, R>(&'a mut self, f: impl FnOnce(&'a mut T) -> R) -> R {
            f(&mut self.data[crate::architecture::core_id() as usize])
        }
    }
}

#[cfg(debug_assertions)]
use per_core_debug::PerCoreInner;
#[cfg(not(debug_assertions))]
use per_core_unchecked::PerCoreInner;

/// Provides access to a list of items per-core
pub struct PerCore<T> {
    inner: UnsafeCell<PerCoreInner<T>>,
}

impl<T: Copy> PerCore<T> {
    /// Creates a default-initialized PerCore struct
    /// that is initializable at compile time
    pub const fn new(initial: T) -> Self {
        Self {
            inner: UnsafeCell::new(PerCoreInner::new(initial)),
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

unsafe impl<T> Send for PerCore<T> {}
unsafe impl<T> Sync for PerCore<T> {}

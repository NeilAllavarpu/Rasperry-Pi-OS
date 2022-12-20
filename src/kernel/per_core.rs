use crate::architecture;
use core::{
    cell::{RefCell, RefMut},
    mem,
    ops::{Deref, DerefMut},
};

use super::thread::PreemptionGuard;

/// The maximum possible number of cores supported
const MAX_CORES: usize = 4;

/// Provides access to a list of items per-core
pub struct PerCore<T> {
    /// The underlying data
    data: [RefCell<T>; MAX_CORES],
}

impl<T> PerCore<T> {
    /// Runs the given function with a mutable reference
    /// to the current core's value
    ///
    /// Prevents the current execution from being switched to another core
    /// while using the core's value
    pub fn current(&self) -> Guard<T> {
        // SAFETY: There is no overwriting of entries that do not belong to the
        // current core: We only modify our core's entry, and no one can switch
        // onto this core while doing so because preemption is disabled
        Guard::new(
            self.data
                .get(usize::from(architecture::machine::core_id()))
                .expect("Core ID should be in-bounds")
                .borrow_mut(),
        )
    }
}

impl<T> PerCore<T> {
    /// Creates a default-initialized `PerCore` struct that is initializable at
    /// compile time, by using the result of the closure as the default value
    pub const fn new<G: ~const Fn() -> T>(initial: G) -> Self {
        // TODO: Is there a better way to initialize this without copy-paste?
        let per_core = Self {
            data: [
                RefCell::new(initial.call(())),
                RefCell::new(initial.call(())),
                RefCell::new(initial.call(())),
                RefCell::new(initial.call(())),
            ],
        };
        // `forget` is necessary because this is a const context
        mem::forget(initial);
        per_core
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

/// Provides protected access to the data of a `Mutex`. Dereferencing the `Guard` will provide access to the data, and the `Mutex` remains locked while the `Guard` persists. When the `Guard` is dropped, the `Mutex` is unlocked.
pub struct Guard<'a, T> {
    /// The mutex's state
    data: RefMut<'a, T>,
    /// Guard for preemption
    _preemption_guard: PreemptionGuard,
}

impl<'a, T> Guard<'a, T> {
    /// Creates a new `Guard` for the given mutex
    /// # Safety
    /// The mutex must be locked before creating this guard
    /// Only one guard should be active at any given time
    pub fn new(data: RefMut<'a, T>) -> Self {
        Self {
            data,
            _preemption_guard: PreemptionGuard::new(),
        }
    }
}

impl<'a, T> Deref for Guard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.data
    }
}

impl<'a, T> DerefMut for Guard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data
    }
}

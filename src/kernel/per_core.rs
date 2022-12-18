use crate::architecture::{self, thread::me};
use core::{
    cell::{RefCell, RefMut},
    mem,
    ops::{Deref, DerefMut},
    sync::atomic::{compiler_fence, Ordering},
};

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
        let i = Self {
            data: [
                RefCell::new(initial.call(())),
                RefCell::new(initial.call(())),
                RefCell::new(initial.call(())),
                RefCell::new(initial.call(())),
            ],
        };
        mem::forget(initial);
        i
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
    /// Whether or not preemption was enabled beforehand
    was_preemptible: bool,
}

impl<'a, T> Guard<'a, T> {
    /// Creates a new `Guard` for the given mutex
    /// # Safety
    /// The mutex must be locked before creating this guard
    /// Only one guard should be active at any given time
    pub fn new(data: RefMut<'a, T>) -> Self {
        let was_preemptible = me(|me| {
            let was_preemptible = me.preemptible;
            me.preemptible = false;
            was_preemptible
        });
        // Compiler fence: make sure that the disabling of preemption change is
        // committed before we execute important work
        compiler_fence(Ordering::Release);
        Self {
            data,
            was_preemptible,
        }
    }
}

impl<'a, T> Drop for Guard<'a, T> {
    fn drop(&mut self) {
        // Reenable preemption if it was disabled
        me(|me| me.preemptible = self.was_preemptible);
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

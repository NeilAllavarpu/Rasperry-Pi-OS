use core::{cell::OnceCell, ops::Deref};

/// A [`OnceCell`](https://doc.rust-lang.org/core/cell/struct.LazyCell.html),
/// that must be initialized (via an `init` sequence in the kernel boot) prior
/// to any accesses
///
/// If possible, use a `LazySyncCell`, as that requires less code bloat and is
/// generally safer
#[allow(clippy::module_name_repetitions)]
pub struct InitCell<T> {
    /// The underlying `OnceCell`
    cell: OnceCell<T>,
}

impl<T> InitCell<T> {
    /// Creates a new empty cell
    #[must_use]
    pub const fn new() -> Self {
        Self {
            cell: OnceCell::new(),
        }
    }

    /// Sets the contents of the cell to `value`.
    /// # Safety
    /// This method should only be used once. If called multiple times, the cell
    /// may or may not panic
    /// # Panics
    /// May or may not panic if called multiple times
    pub unsafe fn set(&self, value: T) {
        assert!(
            self.cell.set(value).is_ok(),
            "Should only write once to an `InitCell`"
        );
    }
}

impl<T> Deref for InitCell<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.cell
            .get()
            .expect("Should only access an `InitCell` once it has been initialized")
    }
}
// SAFETY: `InitCell` is always Send because it only provides shared access
// once set, and by assumption it can only be mutably set before there are readers
unsafe impl<T> Send for InitCell<T> {}
// SAFETY: `InitCell` is always Sync because it only provides shared access
// once set, and by assumption it can only be mutably set before there are readers
unsafe impl<T> Sync for InitCell<T> {}

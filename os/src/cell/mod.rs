use core::cell::SyncUnsafeCell;
use core::mem::MaybeUninit;
use core::ops::Deref;
use core::ptr::NonNull;
use core::sync::atomic::AtomicBool;
use core::sync::atomic::Ordering;

/// A cell that may only be initialized once and exactly once
pub struct OnceLock<T> {
    /// The actual contents of the `OnceLock`
    contents: SyncUnsafeCell<Option<T>>,
    /// Whether or not the `OnceLock` is fully initialized
    is_set: AtomicBool,
    /// Whether or not the `OnceLock` is in the process of initializing
    is_setting: AtomicBool,
}

impl<T> OnceLock<T> {
    /// Creates a new, empty `InitCell`
    #[inline]
    pub const fn new() -> Self {
        Self {
            contents: SyncUnsafeCell::new(None),
            is_set: AtomicBool::new(false),
            is_setting: AtomicBool::new(false),
        }
    }

    /// Sets the `InitCell` to the given value, if the value is not already set or being set
    ///
    /// Returns whether or not the setting operation was successful. Fails if already set or there
    /// are concurrent setters
    #[inline]
    pub fn set(&self, value: T) -> Result<(), T> {
        if self.is_setting.swap(true, Ordering::Relaxed) {
            Err(value)
        } else {
            assert!(
                !self.is_set.load(Ordering::Relaxed),
                "Init cell should not already be set"
            );
            {
                let inner_pointer = self.contents.get();
                let inner_ref =
                // SAFETY: This is the only thing that can possibly access the contents at this
                // time and so a mutable reference is safe
                unsafe { inner_pointer.as_mut() }.expect("Contents should never be null addressed");

                assert!(
                    matches!(inner_ref.replace(value), None),
                    "Init cell should not be already be set"
                );
            }
            assert!(
                !self.is_set.swap(true, Ordering::Release),
                "Init cell should not already be set"
            );

            Ok(())
        }
    }

    /// Gets the reference to the underlying value.
    ///
    /// Returns `None` if the cell is empty, or being initialized. This method never blocks.
    #[inline]
    pub fn get(&self) -> Option<&T> {
        if self.is_set.load(Ordering::Acquire) {
            let inner_pointer = self.contents.get();
            let inner_ref =
                // SAFETY: All initializers are done by this point, and so no one else has a
                // mutable reference to the contents
                unsafe { inner_pointer.as_ref() }.expect("Contents should never be null addressed");

            inner_ref.as_ref()
        } else {
            None
        }
    }
}

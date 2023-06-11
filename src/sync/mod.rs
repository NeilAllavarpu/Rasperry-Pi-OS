use core::arch::aarch64::{__sev, __wfe};
use core::cell::UnsafeCell;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, Ordering};

/// A spinlock mutex
pub struct SpinLock<T: ?Sized> {
    /// Whether or not the spinlock is taken
    is_locked: AtomicBool,
    /// The protected data
    data: UnsafeCell<T>,
}

// SAFETY: The spinlock guarantees thread safety
unsafe impl<T> Sync for SpinLock<T> {}

impl<T> SpinLock<T> {
    /// Creates a spinlock around the given data
    #[inline]
    pub const fn new(data: T) -> Self {
        Self {
            data: UnsafeCell::new(data),
            is_locked: AtomicBool::new(false),
        }
    }

    /// Locks the mutex. The mutex is automatically unlocked when the returned `MutexGuard` is
    /// dropped
    #[inline]
    pub fn lock(&self) -> MutexGuard<T> {
        while self.is_locked.swap(true, Ordering::Acquire) {
            while self.is_locked.load(Ordering::Relaxed) {
                #[cfg(target_arch = "aarch64")]
                // SAFETY: This only compiles for `aarch64` targets
                unsafe {
                    __wfe();
                };
            }
        }

        MutexGuard(self)
    }

    /// Unlocks the mutex
    ///
    /// # Safety
    ///
    /// This must only be called by the destructor of the `MutexGuard` that locked this mutex
    #[inline]
    unsafe fn unlock(&self) {
        self.is_locked.store(false, Ordering::Release);
        #[cfg(target_arch = "aarch64")]
        // SAFETY: This only compiles for `aarch64` targets
        unsafe {
            __sev();
        }
    }
}

pub struct MutexGuard<'locked, T>(&'locked SpinLock<T>);

impl<'locked, T> MutexGuard<'locked, T> {
    /// Returns a pointer to the spinlock's data
    const fn get_pointer(&self) -> NonNull<T> {
        // SAFETY: pointers to `data` are nonnull
        unsafe { NonNull::new_unchecked(self.0.data.get()) }
    }
}

impl<'locked, T> Deref for MutexGuard<'locked, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        // SAFETY: Since the lock has been acquired, we have exclusive mutable access to the
        // interior
        unsafe { self.get_pointer().as_ref() }
    }
}

impl<'locked, T> DerefMut for MutexGuard<'locked, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: Since the lock has been acquired, we have exclusive mutable access to the
        // interior
        unsafe { self.get_pointer().as_mut() }
    }
}

impl<'locked, T> Drop for MutexGuard<'locked, T> {
    #[inline]
    fn drop(&mut self) {
        // SAFETY: We trust the creator of this guard to do so only for proper locking, and so this
        // is the correct time to unlock the mutex
        unsafe {
            self.0.unlock();
        }
    }
}

use core::arch::asm;
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use core::{
    cell::UnsafeCell,
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, Ordering},
};

fn wfe() {
    unsafe {
        asm!("wfe", options(nomem, nostack, preserves_flags));
    }
}

fn sev() {
    unsafe {
        asm!("sev", options(nomem, nostack, preserves_flags));
    }
}
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
    pub const fn new(data: T) -> Self {
        Self {
            data: UnsafeCell::new(data),
            is_locked: AtomicBool::new(false),
        }
    }

    pub fn lock(&self) -> MutexGuard<T> {
        while self.is_locked.swap(true, Ordering::Acquire) {
            core::hint::spin_loop();
        }

        unsafe { MutexGuard::new(self) }
    }

    unsafe fn unlock(&self) {
        debug_assert!(self.is_locked.swap(false, Ordering::Release));
    }
}

pub struct MutexGuard<'a, T> {
    parent: &'a SpinLock<T>,
}

impl<'a, T> MutexGuard<'a, T> {
    /// Creates a new MutexGuard for the *locked* spinlock
    ///
    /// # Safety
    ///
    /// The spinlock must be locked for the duration of this `MutexGuard` to guarantee we have
    /// exclusive access to the data inside
    const unsafe fn new(parent: &'a SpinLock<T>) -> Self {
        Self { parent }
    }

    /// Returns a pointer to the spinlock's data
    fn get_pointer(&self) -> NonNull<T> {
        // SAFETY: pointers to `data` are nonnull
        unsafe { NonNull::new_unchecked(self.parent.data.get()) }
    }
}

impl<'a, T> Deref for MutexGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: Since the lock has been acquired, we have exclusive mutable access to the
        // interior
        unsafe { self.get_pointer().as_ref() }
    }
}

impl<'a, T> DerefMut for MutexGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: Since the lock has been acquired, we have exclusive mutable access to the
        // interior
        unsafe { self.get_pointer().as_mut() }
    }
}

impl<'a, T> Drop for MutexGuard<'a, T> {
    fn drop(&mut self) {
        unsafe {
            self.parent.unlock();
        }
    }
}

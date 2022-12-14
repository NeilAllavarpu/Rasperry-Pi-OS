use crate::{architecture, kernel};
use aarch64_cpu::asm::{sev, wfe};
use core::{
    cell::UnsafeCell,
    ptr::{self, drop_in_place},
    sync::atomic::{AtomicBool, Ordering},
};

/// A spinlock mutex
pub struct SpinLock<T> {
    /// The protected data
    inner: UnsafeCell<T>,
    /// Whether or not the spinlock is taken
    is_locked: AtomicBool,
    /// State of the interrupts, prior to being locked
    guard: UnsafeCell<architecture::exception::Guard>,
}

impl<T> SpinLock<T> {
    /// Creates a spinlock around the given data
    pub const fn new(data: T) -> Self {
        Self {
            inner: UnsafeCell::new(data),
            is_locked: AtomicBool::new(false),
            guard: UnsafeCell::new(
                // SAFETY: This state is never used, as it is overwritten upon locking
                unsafe { architecture::exception::Guard::default() },
            ),
        }
    }
}

// SAFETY: The spinlock guarantees thread safety
unsafe impl<T> Send for SpinLock<T> {}
// SAFETY: The spinlock guarantees thread safety
unsafe impl<T> Sync for SpinLock<T> {}

impl<T> kernel::Mutex for SpinLock<T> {
    type State = T;

    fn lock(&self) -> kernel::MutexGuard<Self> {
        let guard = architecture::exception::Guard::new();
        if self.is_locked.swap(true, Ordering::AcqRel) {
            drop(guard);
            wfe();
            return self.lock();
        }

        // SAFETY:
        // Since the lock has been acquired, setting the internal state is safe,
        // creating the lock guard is safe, and dereferencing the raw pointer to
        // create a unique mutable reference is also safe.
        unsafe {
            ptr::write(self.guard.get(), guard);
            kernel::MutexGuard::new(self, &mut *self.inner.get())
        }
    }

    unsafe fn unlock(&self) {
        self.is_locked.store(false, Ordering::Release);
        sev();
        // SAFETY: `guard` was set by `lock` and so must be valid
        unsafe {
            drop_in_place(self.guard.get());
        }
    }
}

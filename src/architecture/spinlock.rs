use crate::{
    architecture::{self, exception},
    kernel,
};
use aarch64_cpu::asm::{sev, wfe};
use core::{
    cell::{RefCell, UnsafeCell},
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, Ordering},
};

/// A spinlock mutex
pub struct SpinLock<T: ?Sized> {
    /// Whether or not the spinlock is taken
    is_locked: AtomicBool,
    /// State of the interrupts, prior to being locked
    guard: RefCell<MaybeUninit<exception::Guard>>,
    /// The protected data
    inner: UnsafeCell<T>,
}

impl<T> SpinLock<T> {
    /// Creates a spinlock around the given data
    pub const fn new(data: T) -> Self {
        Self {
            inner: UnsafeCell::new(data),
            is_locked: AtomicBool::new(false),
            guard: RefCell::new(MaybeUninit::uninit()),
        }
    }
}

// SAFETY: The spinlock guarantees thread safety
unsafe impl<T> Send for SpinLock<T> {}
// SAFETY: The spinlock guarantees thread safety
unsafe impl<T> Sync for SpinLock<T> {}

impl<T: ?Sized> kernel::Mutex for SpinLock<T> {
    type State = T;

    fn lock(&self) -> kernel::MutexGuard<Self> {
        let mut guard = architecture::exception::Guard::new();
        while self.is_locked.swap(true, Ordering::Acquire) {
            drop(guard);
            wfe();
            guard = architecture::exception::Guard::new();
        }

        // SAFETY:
        // Since the lock has been acquired, setting the internal state is safe,
        // creating the lock guard is safe, and dereferencing the raw pointer to
        // create a unique mutable reference is also safe. Writing over the
        // previous guard is also safe because there should never be a valid
        // guard remaining - either this stores the uninitialized guard, which
        // should never be dropped, or this stores a stale previous guard, which
        // has already been dropped by `unlock`
        unsafe {
            self.guard.borrow_mut().write(guard);
            kernel::MutexGuard::new(self, &mut *self.inner.get())
        }
    }

    unsafe fn unlock(&self) {
        // SAFETY: `guard` was set by `lock` and so must be valid
        let _guard = unsafe { self.guard.borrow_mut().assume_init_read() };
        self.is_locked.store(false, Ordering::Release);
        sev();
    }
}

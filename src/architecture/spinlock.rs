use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, Ordering};

/// A spinlock mutex
pub struct Spinlock<T> {
    inner: UnsafeCell<T>,
    is_locked: AtomicBool,
}

impl<T> Spinlock<T> {
    pub const fn new(data: T) -> Self {
        Self {
            inner: UnsafeCell::new(data),
            is_locked: AtomicBool::new(false),
        }
    }
}

unsafe impl<T> Send for Spinlock<T> {}
unsafe impl<T> Sync for Spinlock<T> {}

impl<T> crate::Mutex for Spinlock<T> {
    type State = T;

    fn lock<'a, R>(&'a self, f: impl FnOnce(&'a mut Self::State) -> R) -> R {
        use aarch64_cpu::asm::{sev, wfe};
        while self.is_locked.swap(true, Ordering::AcqRel) {
            core::hint::spin_loop();
            wfe();
        }

        let result: R = f(unsafe { &mut *self.inner.get() });

        self.is_locked.store(false, Ordering::Release);
        sev();
        result
    }
}

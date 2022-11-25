use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicBool, Ordering},
};

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

impl<T> crate::kernel::Mutex for Spinlock<T> {
    type State = T;

    fn lock<'a, R>(&'a self, f: impl FnOnce(&'a mut Self::State) -> R) -> R {
        use crate::architecture::exception;
        use aarch64_cpu::asm::{sev, wfe};
        let mut state: exception::ExceptionMasks = unsafe { exception::disable() };
        while self.is_locked.swap(true, Ordering::AcqRel) {
            unsafe {
                exception::restore(state);
            }

            wfe();

            state = unsafe { exception::disable() };
        }

        let result: R = f(unsafe { &mut *self.inner.get() });

        self.is_locked.store(false, Ordering::Release);
        sev();
        unsafe {
            exception::restore(state);
        }
        result
    }
}

use core::arch::aarch64::{__sev, __wfe};
use core::cell::{Cell, UnsafeCell};
use core::ops::{Deref, DerefMut};
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use core::{hint, mem};

// use crate::println;

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
                core::hint::spin_loop();
            }
        }

        MutexGuard(self, Cell::new(true))
    }

    /// Unlocks the mutex
    ///
    /// # Safety
    ///
    /// This must only be called by the destructor of the `MutexGuard` that locked this mutex
    #[inline]
    unsafe fn unlock(&self) {
        self.is_locked.store(false, Ordering::Release);
    }
}

pub struct MutexGuard<'locked, T>(&'locked SpinLock<T>, Cell<bool>);

impl<T> MutexGuard<'_, T> {
    pub fn unlock(&self) {
        assert!(self.1.get());
        self.1.set(false);
        unsafe { self.0.unlock() }
    }
}

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
        assert!(self.1.get());
        // SAFETY: Since the lock has been acquired, we have exclusive mutable access to the
        // interior
        unsafe { self.get_pointer().as_ref() }
    }
}

impl<'locked, T> DerefMut for MutexGuard<'locked, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        assert!(self.1.get());
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

/// A spinlock reader-writer lock
pub struct RwLock<T: ?Sized> {
    /// Whether or not the read-write lock is taken, and in what state
    /// * 0: The lock is not held by anyone
    /// * u8::MAX: The lock is held by a writer
    /// * Any other value: The lock is held by `state` readers
    state: AtomicU8,
    /// The protected data
    data: UnsafeCell<T>,
}

// SAFETY: The reader-writer lock guarantees thread safety
unsafe impl<T> Sync for RwLock<T> {}

impl<T> RwLock<T> {
    const UNLOCKED: u8 = 0;
    const MAX_READERS: u8 = u8::MAX - 1;
    const WRITER: u8 = u8::MAX;
    /// Creates a spinlock around the given data
    #[inline]
    pub const fn new(data: T) -> Self {
        Self {
            data: UnsafeCell::new(data),
            state: AtomicU8::new(0),
        }
    }

    /// Locks the mutex. The mutex is automatically unlocked when the returned `MutexGuard` is
    /// dropped
    #[inline]
    pub fn read(&self) -> ReadGuard<T> {
        while self
            .state
            .fetch_update(Ordering::Relaxed, Ordering::Acquire, |state| match state {
                Self::MAX_READERS | Self::WRITER => None,
                state => Some(state + 1),
            })
            .is_err()
        {
            hint::spin_loop();
        }

        ReadGuard(self)
    }

    /// Locks the mutex. The mutex is automatically unlocked when the returned `MutexGuard` is
    /// dropped
    #[inline]
    pub fn write(&self) -> WriteGuard<T> {
        while self
            .state
            .fetch_update(Ordering::Relaxed, Ordering::Acquire, |state| match state {
                Self::UNLOCKED => Some(Self::WRITER),
                _ => None,
            })
            .is_err()
        {
            hint::spin_loop();
        }
        WriteGuard(self)
    }

    /// Unlocks the reader end of a reader-writer lock
    ///
    /// # Safety
    ///
    /// This must only be called by the destructor of the `ReadGuard` that locked this RwLock
    #[inline]
    unsafe fn unlock_read(&self) {
        self.state.fetch_sub(1, Ordering::Relaxed);
    }

    /// Unlocks the writer end of a reader-writer lock
    ///
    /// # Safety
    ///
    /// This must only be called by the destructor of the `WriteGuard` that locked this RwLock
    #[inline]
    unsafe fn unlock_write(&self) {
        self.state.store(Self::UNLOCKED, Ordering::Release);
    }

    unsafe fn downgrade_write(&self) {
        self.state.store(1, Ordering::Release);
    }
}

pub struct ReadGuard<'locked, T>(&'locked RwLock<T>);

impl<'locked, T> ReadGuard<'locked, T> {
    /// Returns a pointer to the spinlock's data
    const fn get_pointer(&self) -> NonNull<T> {
        // SAFETY: pointers to `data` are nonnull
        unsafe { NonNull::new_unchecked(self.0.data.get()) }
    }

    pub fn state(&self) -> u8 {
        self.0.state.load(Ordering::Relaxed)
    }
}

impl<'locked, T> Deref for ReadGuard<'locked, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        // SAFETY: Since the lock has been acquired, we have exclusive mutable access to the
        // interior
        unsafe { self.get_pointer().as_ref() }
    }
}

impl<'locked, T> Drop for ReadGuard<'locked, T> {
    #[inline]
    fn drop(&mut self) {
        // SAFETY: We trust the creator of this guard to do so only for proper locking, and so this
        // is the correct time to unlock the mutex
        unsafe {
            self.0.unlock_read();
        }
    }
}

pub struct WriteGuard<'locked, T>(&'locked RwLock<T>);

impl<'locked, T> WriteGuard<'locked, T> {
    /// Returns a pointer to the spinlock's data
    const fn get_pointer(&self) -> NonNull<T> {
        // SAFETY: pointers to `data` are nonnull
        unsafe { NonNull::new_unchecked(self.0.data.get()) }
    }

    pub fn downgrade(guard: Self) -> ReadGuard<'locked, T> {
        let rw = guard.0;
        // Forget the guard so that it does not auto unlock
        mem::forget(guard);
        unsafe { rw.downgrade_write() };
        ReadGuard(rw)
    }

    pub fn state(&self) -> u8 {
        self.0.state.load(Ordering::Relaxed)
    }
}

impl<'locked, T> Deref for WriteGuard<'locked, T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        // SAFETY: Since the lock has been acquired, we have exclusive mutable access to the
        // interior
        unsafe { self.get_pointer().as_ref() }
    }
}

impl<'locked, T> DerefMut for WriteGuard<'locked, T> {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: Since the lock has been acquired, we have exclusive mutable access to the
        // interior
        unsafe { self.get_pointer().as_mut() }
    }
}

impl<'locked, T> Drop for WriteGuard<'locked, T> {
    #[inline]
    fn drop(&mut self) {
        // SAFETY: We trust the creator of this guard to do so only for proper locking, and so this
        // is the correct time to unlock the mutex
        unsafe {
            self.0.unlock_write();
        }
    }
}

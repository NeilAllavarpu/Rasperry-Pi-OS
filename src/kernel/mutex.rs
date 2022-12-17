use core::ops::{Deref, DerefMut};

/// Guarantess single-access of the enclosed data
pub trait Mutex {
    /// The type of state that is wrapped by this mutex.
    type State: ?Sized;

    /// Locks the mutex, preventing any other thread from accessing the protected state
    /// Returns a temporary guard to the protected state
    fn lock(&self) -> Guard<Self>;

    /// Unlocks the mutex, allowing other threads to acquire the lock
    /// # Safety
    /// Only a `Guard` should manually unlock this, after having acquired the lock
    unsafe fn unlock(&self);
}

/// Provides protected access to the data of a `Mutex`. Dereferencing the `Guard` will provide access to the data, and the `Mutex` remains locked while the `Guard` persists. When the `Guard` is dropped, the `Mutex` is unlocked.
pub struct Guard<'a, Lock: Mutex + ?Sized> {
    /// The enclosing mutex
    mutex: &'a Lock,
    /// The mutex's state
    data: &'a mut Lock::State,
}

impl<'a, Lock: Mutex + ?Sized> Guard<'a, Lock> {
    /// Creates a new `Guard` for the given mutex
    /// # Safety
    /// The mutex must be locked before creating this guard
    /// Only one guard should be active at any given time
    pub unsafe fn new(mutex: &'a Lock, data: &'a mut Lock::State) -> Self {
        Self { mutex, data }
    }
}

impl<'a, Lock: Mutex + ?Sized> Drop for Guard<'a, Lock> {
    fn drop(&mut self) {
        // SAFETY: By assumption, this guard has the lock on the mutex, and so can release it
        unsafe {
            self.mutex.unlock();
        }
    }
}

impl<'a, Lock: Mutex + ?Sized> Deref for Guard<'a, Lock> {
    type Target = Lock::State;

    fn deref(&self) -> &Self::Target {
        self.data
    }
}

impl<'a, Lock: Mutex + ?Sized> DerefMut for Guard<'a, Lock> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.data
    }
}

use crate::{architecture::SpinLock, kernel::Mutex};
use core::{
    cell::{Ref, RefCell, RefMut},
    mem,
    ops::{Deref, DerefMut},
};

/// A reader-writer lock
///
/// This type of lock allows a number of readers or at most one writer at any
/// point in time. The write portion of this lock typically allows modification
/// of the underlying data (exclusive access) and the read portion of this lock
/// typically allows for read-only access (shared access).
///
/// In comparison, a `Mutex` does not distinguish between readers or writers that
/// acquire the lock, therefore blocking any threads waiting for the lock to
/// become available. An `RwLock` will allow any number of readers to acquire
/// the lock as long as a writer is not holding the lock.
///
/// The priority policy of the lock is dependent on the underlying
/// implementation, and this type does not guarantee that any particular policy
/// will be used. In particular, a writer which is waiting to acquire the lock
/// in `write` might or might not block concurrent calls to `read`
///
/// The type parameter T represents the data that this lock protects. It is
/// required that T satisfies Send to be shared across threads and Sync to allow
/// concurrent access through readers. The RAII guards returned from the locking
/// methods implement `Deref` (and `DerefMut` for the write methods) to allow
/// access to the content of the lock.
pub struct RwLock<T> {
    /// The protected data
    data: RefCell<T>,
    /// How many readers are currently accessing the resource
    num_readers: SpinLock<u64>,
    /// Whether or not the resource is fully available
    is_taken: SpinLock<()>,
}

impl<T: Send + Sync> RwLock<T> {
    /// Creates a new instance of an `RwLock<T>` which is unlocked.
    pub const fn new(initial: T) -> Self {
        Self {
            data: RefCell::new(initial),
            num_readers: SpinLock::new(0),
            is_taken: SpinLock::new(()),
        }
    }

    /// Locks this `RwLock` with shared read access, blocking the current thread
    /// until it can be acquired.
    ///
    /// The calling thread will be blocked until there are no more writers which
    /// hold the lock. There may be other readers currently inside the lock when
    /// this method returns. This method does not provide any guarantees with
    /// respect to the ordering of whether contentious readers or writers will
    /// acquire the lock first.
    ///
    /// Returns an RAII guard which will release this thread’s shared access
    /// once it is dropped.
    pub fn read(&self) -> RwLockReadGuard<T> {
        {
            let mut readers = self.num_readers.lock();
            if *readers == 0 {
                // Intentionally `forget` the guard so that we can manually
                // unlock it later
                #[allow(clippy::mem_forget)]
                mem::forget(self.is_taken.lock());
            }
            *readers += 1;
        }
        // SAFETY: We have just locked the `RwLock` for readers
        unsafe { RwLockReadGuard::new(self) }
    }

    /// Locks this `RwLock` with exclusive write access, blocking the current
    /// thread until it can be acquired.
    ///
    /// This function will not return while other writers or other readers
    /// currently have access to the lock.
    ///
    /// Returns an RAII guard which will drop the write access of this `RwLock`
    /// when dropped.
    pub fn write(&self) -> RwLockWriteGuard<T> {
        // Intentionally `forget` the guard so that we can manually unlock it
        // later
        #[allow(clippy::mem_forget)]
        mem::forget(self.is_taken.lock());
        // SAFETY: We have exclusively locked access to the underlying data
        unsafe { RwLockWriteGuard::new(self) }
    }

    /// Decrements the reader count, and unlocks the resource for writers if
    /// applicable
    /// # Safety
    /// Must be only invoked when a reader is yielding access to the protected
    /// data
    unsafe fn read_unlock(&self) {
        let mut readers = self.num_readers.lock();
        *readers -= 1;
        if *readers == 0 {
            // SAFETY: We have properly locked this in `read`, and are properly
            // unlocking it here
            unsafe {
                self.is_taken.unlock();
            }
        }
    }

    /// Releases exclusive mutable access to the underlying data
    /// # Safety
    /// Must only be invoked when a writer is yielding access to the protected
    /// data
    unsafe fn write_unlock(&self) {
        // SAFETY: We have properly locked this in `write`, and are properly
        // unlocking it here
        unsafe {
            self.is_taken.unlock();
        }
    }
}

/// RAII structure used to release the shared read access of a lock when dropped.
///
/// This structure is created by the `read` method on `RwLock`
#[allow(clippy::module_name_repetitions)]
pub struct RwLockReadGuard<'a, T: Send + Sync> {
    /// The enclosing `RwLock`
    rwlock: &'a RwLock<T>,
    /// The `Ref` to the underlying data
    reference: Ref<'a, T>,
}

impl<'a, T: Send + Sync> RwLockReadGuard<'a, T> {
    /// Creates a new `RwLockReadGuard` for the given `RwLock`
    /// # Safety
    /// The `RwLock` must be reader-locked before creating this guard.
    /// No `RwLockWriteGuard` should be active while this guard is active
    unsafe fn new(rwlock: &'a RwLock<T>) -> Self {
        Self {
            rwlock,
            reference: rwlock.data.borrow(),
        }
    }
}

impl<'a, T: Send + Sync> Drop for RwLockReadGuard<'a, T> {
    fn drop(&mut self) {
        // SAFETY: By assumption, the `RwLock` is safely read-locked, so we can
        // attempt to read-unlock it
        unsafe {
            self.rwlock.read_unlock();
        }
    }
}

impl<'a, T: Send + Sync> Deref for RwLockReadGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.reference
    }
}

/// RAII structure used to release the exclusive write access of a lock when
/// dropped.
///
/// This structure is created by the `write` method on `RwLock`
#[allow(clippy::module_name_repetitions)]
pub struct RwLockWriteGuard<'a, T: Send + Sync> {
    /// The enclosing `RwLock`
    rwlock: &'a RwLock<T>,
    /// The `Ref` to the underlying data
    reference: RefMut<'a, T>,
}

impl<'a, T: Send + Sync> RwLockWriteGuard<'a, T> {
    /// Creates a new `RwLockWriteGuard` for the given `RwLock`
    /// # Safety
    /// The `RwLock` must be writer-locked before creating this guard.
    /// No other guards should be active while this guard is active
    unsafe fn new(rwlock: &'a RwLock<T>) -> Self {
        Self {
            rwlock,
            reference: rwlock.data.borrow_mut(),
        }
    }
}

impl<'a, T: Send + Sync> Drop for RwLockWriteGuard<'a, T> {
    fn drop(&mut self) {
        // SAFETY: By assumption, the `RwLock` is safely writer-locked, so we can
        // attempt to writer-unlock it
        unsafe {
            self.rwlock.write_unlock();
        }
    }
}

impl<'a, T: Send + Sync> Deref for RwLockWriteGuard<'a, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.reference
    }
}

impl<'a, T: Send + Sync> DerefMut for RwLockWriteGuard<'a, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.reference
    }
}

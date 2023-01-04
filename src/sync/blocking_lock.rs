use super::{Mutex, MutexGuard};
use crate::{
    collections::ArcStack,
    thread::{block, schedule, Tcb, Thread},
};
use aarch64_cpu::asm::barrier::{self, isb};
use core::{
    cell::UnsafeCell,
    sync::atomic::{AtomicI64, Ordering},
};

/// A blocking mutex.
pub struct BlockingLock<T> {
    /// Count of threads depending on this lock
    /// * 1 indicates that the lock is available
    /// * Nonpositive indicates the number of threads waiting for a taken lock
    waiting_count: AtomicI64,
    /// Threads currently blocked on the lock
    blocked_threads: ArcStack<Tcb>,
    /// The protected state
    state: UnsafeCell<T>,
}

impl<T> BlockingLock<T> {
    /// Creates a new `BlockingLock` containing the given state
    pub const fn new(initial: T) -> Self {
        Self {
            waiting_count: AtomicI64::new(1),
            blocked_threads: ArcStack::new(),
            state: UnsafeCell::new(initial),
        }
    }
}
impl<T: ~const Default> const Default for BlockingLock<T> {
    fn default() -> Self {
        Self::new(Default::default())
    }
}

impl<T> Mutex for BlockingLock<T> {
    type State = T;

    fn lock(&self) -> MutexGuard<Self> {
        // If someone had already taken the lock (WAS LESS THAN 1)
        if self.waiting_count.fetch_sub(1, Ordering::Acquire) != 1 {
            // SAFETY: Threads are fixed in place on the heap, and persist since
            // the strong count is at least one
            block(|thread| self.blocked_threads.push(thread));

            assert!(self.waiting_count.load(Ordering::Acquire) <= 0);
        }
        // SAFETY: At this point, we have ensured mutual exclusion
        unsafe { MutexGuard::new(self, self.state.get().as_mut().expect("Should not be null")) }
    }

    unsafe fn unlock(&self) {
        // If there were other threads waiting for the lock (WAS -1)
        if self.waiting_count.fetch_add(1, Ordering::Release) + 1 != 1 {
            loop {
                if let Some(thread) = self.blocked_threads.pop() {
                    // SAFETY: This thread was taken from an `Arc`
                    schedule(Thread(thread));
                    break;
                }
                isb(barrier::SY);
            }
        }
    }
}

// SAFETY: The mutual exclusion provided by `BlockingLock` provides Sync
unsafe impl<T> Sync for BlockingLock<T> {}
// SAFETY: The mutual exclusion provided by `BlockingLock` provides Send
unsafe impl<T> Send for BlockingLock<T> {}

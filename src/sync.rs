/// A Reader-Writer lock
mod rw_lock;
pub use rw_lock::*;

/// Combines an atomic reference with a stamp
mod atomic_stamped_reference;
pub use atomic_stamped_reference::*;

/// A mutual exclusion primitive useful for protecting shared data
mod mutex;
pub use mutex::*;

/// A busy-wait mutex
mod spinlock;
pub use spinlock::SpinLock;

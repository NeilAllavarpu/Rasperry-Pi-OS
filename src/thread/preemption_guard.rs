use core::sync::atomic::{compiler_fence, Ordering};

use super::Thread;

/// A guard that disables preemption while active
pub struct PreemptionGuard {
    /// Whether or not this thread was preemptible before the creation of this guard
    was_preemptible: bool,
}

impl PreemptionGuard {
    /// Creates a new preemption-safe guard
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let guard = Self {
            was_preemptible: super::current().0.local.preemptible.replace(false),
        };
        // Compiler fence: make sure that the disabling of preemption change is
        // committed before we execute important work
        compiler_fence(Ordering::Release);
        guard
    }
}

impl Drop for PreemptionGuard {
    fn drop(&mut self) {
        if self.was_preemptible {
            let Thread(current) = super::current();

            assert!(!current.is_idle());
            current.local.preemptible.set(true);
            if current.local.pending_preemption.replace(false) {
                super::yield_now();
            }
        }
    }
}

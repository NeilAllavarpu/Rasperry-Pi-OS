use core::sync::atomic::{compiler_fence, Ordering};

use crate::architecture::{self, thread::me};

/// A guard that disables preemption while active
pub struct PreemptionGuard {
    /// Whether or not this thread was preemptible before the creation of this guard
    was_preemptible: bool,
}

impl PreemptionGuard {
    /// Creates a new preemption-safe guard
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        me(|me| {
            let guard = Self {
                was_preemptible: me.preemptible,
            };
            me.preemptible = false;
            // Compiler fence: make sure that the disabling of preemption change is
            // committed before we execute important work
            compiler_fence(Ordering::Release);
            guard
        })
    }
}

impl Drop for PreemptionGuard {
    fn drop(&mut self) {
        if self.was_preemptible {
            me(|me| {
                me.preemptible = true;
                if me.pending_preemption {
                    me.pending_preemption = false;
                    architecture::thread::preempt();
                }
            });
        }
    }
}

use core::{
    mem::replace,
    sync::atomic::{compiler_fence, Ordering},
};

use crate::{architecture::thread::me, kernel};

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
                was_preemptible: replace(&mut me.preemptible, false),
            };
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
                assert!(me.id > 4);
                me.preemptible = true;
                if replace(&mut me.pending_preemption, false) {
                    kernel::thread::switch();
                }
            });
        }
    }
}

use aarch64_cpu::registers::CNTP_CTL_EL0;
use tock_registers::interfaces::{ReadWriteable, Readable};

/// A spinlock mutex
pub struct TimerIrqGuard {
    /// Whether or not timer IRQs were masked prior to creation of this guard
    was_irq_masked: bool,
}

impl TimerIrqGuard {
    /// Disables timer IRQs until this guard is dropped
    pub fn new() -> Self {
        let guard = Self {
            was_irq_masked: CNTP_CTL_EL0.is_set(CNTP_CTL_EL0::IMASK),
        };
        CNTP_CTL_EL0.modify(CNTP_CTL_EL0::IMASK::SET);
        guard
    }
}

impl Drop for TimerIrqGuard {
    fn drop(&mut self) {
        if !self.was_irq_masked {
            CNTP_CTL_EL0.modify(CNTP_CTL_EL0::IMASK::CLEAR);
        }
    }
}

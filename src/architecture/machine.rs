// The boot sequence

use aarch64_cpu::{registers::{CNTFRQ_EL0, MPIDR_EL1, CNTPCT_EL0}, asm::barrier};
use tock_registers::interfaces::Readable;

pub fn core_id() -> u8 {
    (MPIDR_EL1.get() & 0b11) as u8
}

pub fn timer_frequency() -> core::num::NonZeroU32 {
    // The upper 32 bits are reserved to 0
    (CNTFRQ_EL0.get() as u32)
        .try_into()
        .expect("The clock frequency should be nonzero")
}

pub fn current_tick() -> u64 {
    // Prevent that the counter is read ahead of time due to out-of-order execution.
    barrier::isb(barrier::SY);
    CNTPCT_EL0.get()
}

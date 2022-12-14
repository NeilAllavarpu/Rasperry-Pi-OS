use crate::kernel::time::Tick;
use aarch64_cpu::{
    asm::barrier,
    registers::{CNTFRQ_EL0, CNTPCT_EL0},
};
use core::num::NonZeroU32;
use tock_registers::interfaces::Readable;

/// Returns the frequency of the system timer, in Hz
pub fn frequency() -> NonZeroU32 {
    // The upper 32 bits are reserved to 0
    u32::try_from(CNTFRQ_EL0.get())
        .expect("The clock frequency should fit into 32 bits")
        .try_into()
        .expect("The clock frequency should not be 0")
}

/// Returns the current value of the system timer
pub fn current_tick() -> Tick {
    // Prevent that the counter is read ahead of time due to out-of-order execution.
    barrier::isb(barrier::SY);
    Tick::new(CNTPCT_EL0.get())
}
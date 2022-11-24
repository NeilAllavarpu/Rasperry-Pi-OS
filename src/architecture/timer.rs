use aarch64_cpu::{
    asm::barrier,
    registers::{CNTFRQ_EL0, CNTPCT_EL0, CNTP_CTL_EL0, CNTP_TVAL_EL0},
};
use tock_registers::interfaces::{ReadWriteable, Readable, Writeable};

use crate::{log, timer::TimerValue};

pub fn timer_frequency() -> core::num::NonZeroU32 {
    // The upper 32 bits are reserved to 0
    (CNTFRQ_EL0.get() as u32)
        .try_into()
        .expect("The clock frequency should be nonzero")
}

pub fn current_tick() -> TimerValue {
    // Prevent that the counter is read ahead of time due to out-of-order execution.
    barrier::isb(barrier::SY);
    TimerValue::new(CNTPCT_EL0.get())
}

use crate::kernel::timer::TimerValue;
use aarch64_cpu::{
    asm::barrier,
    registers::{CNTFRQ_EL0, CNTPCT_EL0},
};
use core::num::NonZeroU32;
use tock_registers::interfaces::Readable;

pub fn timer_frequency() -> NonZeroU32 {
    // The upper 32 bits are reserved to 0
    u32::try_from(CNTFRQ_EL0.get()).unwrap().try_into().unwrap()
}

pub fn current_tick() -> TimerValue {
    // Prevent that the counter is read ahead of time due to out-of-order execution.
    barrier::isb(barrier::SY);
    TimerValue::new(CNTPCT_EL0.get())
}

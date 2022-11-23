// The boot sequence

use crate::PrivilegeLevel;
use aarch64_cpu::{
    asm::barrier,
    registers::{CurrentEL, CNTFRQ_EL0, CNTPCT_EL0, MPIDR_EL1, TPIDR_EL1},
};
use tock_registers::interfaces::Readable;

pub fn core_id() -> u8 {
    (MPIDR_EL1.get() & 0b11) as u8
}

pub fn thread_id() -> u64 {
    TPIDR_EL1.get()
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

/// Exception level
pub fn exception_level() -> PrivilegeLevel {
    match CurrentEL.read_as_enum(CurrentEL::EL) {
        Some(CurrentEL::EL::Value::EL2) => PrivilegeLevel::Hypervisor,
        Some(CurrentEL::EL::Value::EL1) => PrivilegeLevel::Kernel,
        Some(CurrentEL::EL::Value::EL0) => PrivilegeLevel::User,
        _ => PrivilegeLevel::Unknown,
    }
}

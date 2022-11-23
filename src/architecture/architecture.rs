// Architecture-specific (ARM) code
mod boot;
mod machine;
use aarch64_cpu::{registers::{HCR_EL2, SPSR_EL2, ELR_EL2, SP_EL1, SP}, asm::eret};
pub use machine::*;
mod spinlock;
pub use spinlock::*;
mod config;
pub use config::*;
use tock_registers::interfaces::{Readable, Writeable};

use crate::{call_once, PrivilegeLevel, call_once_per_core};
extern "C" {
    fn _start();
}

/// Switches the core from EL2 to EL1
/// Switches to the given stack pointer
/// Jumps to the main init sequence
#[no_mangle]
fn el2_init() {
    call_once_per_core!();
    // Make sure this is running in EL2
    assert_eq!(exception_level(), PrivilegeLevel::Hypervisor);
    // Enable 64 bit mode for EL1
    HCR_EL2.write(HCR_EL2::RW::EL1IsAarch64);
    // Disable interrupts in EL1 mode, and switch the stack pointer on a per-exception level basis
    SPSR_EL2.write(
        SPSR_EL2::D::Masked
            + SPSR_EL2::A::Masked
            + SPSR_EL2::I::Masked
            + SPSR_EL2::F::Masked
            + SPSR_EL2::M::EL1h,
    );
    // Begin execution with the main init sequence
    ELR_EL2.set(crate::init as *const () as u64);
    // Set the stack pointer when execution resumes
    SP_EL1.set(SP.get());
    eret();
}

pub fn init() {
    call_once!();
    config::init();
}

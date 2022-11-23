// Architecture-specific (ARM) code
mod boot;
mod machine;
use aarch64_cpu::{registers::{HCR_EL2, SPSR_EL2, ELR_EL2, SP_EL1}, asm::eret};
pub use machine::*;
mod spinlock;
pub use spinlock::*;
mod config;
pub use config::*;
use tock_registers::interfaces::Writeable;

use crate::{call_once, exception::PrivilegeLevel, init};
extern "C" {
    fn _start();
}

#[no_mangle]
fn el2_init() {
    call_once!();
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
    ELR_EL2.set(init as *const () as u64);
    // Set an appropriate stack pointer
    SP_EL1.set(_start as *const () as u64);
    eret();
}

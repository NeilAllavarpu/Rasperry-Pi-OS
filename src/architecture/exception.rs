use crate::{call_once_per_core, kernel::exception::PrivilegeLevel};
use aarch64_cpu::{
    asm::barrier,
    registers::{CurrentEL, DAIF, SCTLR_EL1, VBAR_EL1},
};
use tock_registers::interfaces::{ReadWriteable, Readable, Writeable};

// The exception assembly
core::arch::global_asm!(include_str!("exception.s"));

/// Exception level
pub fn exception_level() -> PrivilegeLevel {
    match CurrentEL.read_as_enum(CurrentEL::EL) {
        Some(CurrentEL::EL::Value::EL2) => PrivilegeLevel::Hypervisor,
        Some(CurrentEL::EL::Value::EL1) => PrivilegeLevel::Kernel,
        Some(CurrentEL::EL::Value::EL0) => PrivilegeLevel::User,
        _ => PrivilegeLevel::Unknown,
    }
}

pub fn init() {}

/// Ready exception handling by setting the exception vector base address register.
pub fn per_core_init() {
    extern "Rust" {
        static _exception_vector: core::cell::UnsafeCell<()>;
    }
    call_once_per_core!();

    VBAR_EL1.set(unsafe { _exception_vector.get().to_bits().try_into().unwrap() });

    // Force VBAR update to complete before next instruction.
    barrier::isb(barrier::SY);
}

/// Turns on interrupts
/// # Safety
/// This function should only be used to enable interrupts when cores begin to run
/// At all other times, restoring interrupts should be preferred instead
pub fn enable() {
    call_once_per_core!();
    assert!(
        DAIF.matches_all(DAIF::D::Masked + DAIF::A::Masked + DAIF::I::Masked + DAIF::F::Masked),
        "Interrupts must be disabled to enable them"
    );
    DAIF.write(DAIF::D::Unmasked + DAIF::A::Unmasked + DAIF::I::Unmasked + DAIF::F::Unmasked);

    SCTLR_EL1.modify(SCTLR_EL1::A::Enable);
}

pub struct Masks {
    prior: u64,
}

/// Disables interrupts
/// # Safety
/// Must be paired with a `restore` to ensure that the interrupt state is preserved correctly
pub unsafe fn disable() -> Masks {
    let state = Masks { prior: DAIF.get() };
    DAIF.set(0);
    state
}

/// Re-enables interrupts after having been disabled
/// # Safety
/// The given interrupt state must be from the return value of the most recent `disable` on this thread
pub unsafe fn restore(state: &Masks) {
    DAIF.set(state.prior);
}

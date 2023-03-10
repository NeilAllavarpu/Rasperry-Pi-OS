use crate::{architecture, call_once_per_core, kernel::exception::PrivilegeLevel};
use aarch64_cpu::{
    asm::barrier,
    registers::{CurrentEL, CNTP_CTL_EL0, DAIF, SCTLR_EL1, VBAR_EL1},
};
use tock_registers::interfaces::{ReadWriteable, Readable, Writeable};

// The exception assembly
core::arch::global_asm!(include_str!("exception.s"));

/// Exception level
pub fn el() -> PrivilegeLevel {
    match CurrentEL.read_as_enum(CurrentEL::EL) {
        Some(CurrentEL::EL::Value::EL2) => PrivilegeLevel::Hypervisor,
        Some(CurrentEL::EL::Value::EL1) => PrivilegeLevel::Kernel,
        Some(CurrentEL::EL::Value::EL0) => PrivilegeLevel::User,
        _ => PrivilegeLevel::Unknown,
    }
}

/// Initializes certain exceptions
pub fn init() {}

/// Ready exception handling by setting the exception vector base address register.
pub fn per_core_init() {
    extern "Rust" {
        static _exception_vector: core::cell::UnsafeCell<()>;
    }
    call_once_per_core!();

    VBAR_EL1.set(architecture::usize_to_u64(
        // SAFETY: the exception vector is defined in exception.S
        unsafe { _exception_vector.get() }.to_bits(),
    ));

    // Force VBAR update to complete before next instruction.
    barrier::isb(barrier::SY);

    // Turn on alignment checks
    SCTLR_EL1.modify(SCTLR_EL1::A::Enable + SCTLR_EL1::NAA::Enable + SCTLR_EL1::SA::Enable);
    // Enable timer exceptions
    CNTP_CTL_EL0.modify(CNTP_CTL_EL0::IMASK::CLEAR);
}

/// Checks if exceptions are fully disabled
pub fn are_disabled() -> bool {
    DAIF.matches_all(DAIF::D::Masked + DAIF::A::Masked + DAIF::I::Masked + DAIF::F::Masked)
}

/// Turns on exceptions
/// # Safety
/// This function should only be used to enable exceptions when it is certain that exceptions were disable but enabling them is OK
pub unsafe fn enable() {
    assert!(are_disabled(), "Interrupts must be disabled to enable them");
    barrier::isb(barrier::SY);
    DAIF.write(DAIF::D::Unmasked + DAIF::A::Unmasked + DAIF::I::Unmasked + DAIF::F::Unmasked);
}

/// Disables exceptions
/// # Safety
/// Exceptions must be re-enabled by the caller
pub unsafe fn disable() {
    assert!(
        !are_disabled(),
        "Interrupts must be enabled to disable them"
    );
    DAIF.write(DAIF::D::Masked + DAIF::A::Masked + DAIF::I::Masked + DAIF::F::Masked);
    // Ensure that the changes are fully committed before continuing
    barrier::isb(barrier::SY);
}

/// An exception `Guard` masks exceptions while alive,
/// and restores the prior mask state upon being dropped
pub struct Guard {
    /// Whether or not exceptions were disabled beforehand
    was_disabled: bool,
}

impl Guard {
    /// Creates a new exception guard, masking exceptions
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        let guard = Self {
            was_disabled: are_disabled(),
        };
        if !guard.was_disabled {
            // SAFETY: We have just checked that interrupts are enabled, and we
            // are intending to protect interupts for the duration of an
            // possible guards
            unsafe {
                disable();
            }
        }

        guard
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        if !self.was_disabled {
            // SAFETY: Interrupts were disabled prior, and are properly being
            // restored now - no exception guards are held on this thread/core,
            // and exceptions were disabled by a prior exception guard
            unsafe { enable() }
        }
    }
}

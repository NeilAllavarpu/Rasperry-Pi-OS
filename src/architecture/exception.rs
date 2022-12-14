use crate::{architecture, call_once_per_core, kernel::exception::PrivilegeLevel};
use aarch64_cpu::{
    asm::barrier,
    registers::{CurrentEL, DAIF, SCTLR_EL1, VBAR_EL1},
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

/// Initializes exception handlers
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

/// Stores the mask state of exceptions at some point in time
pub struct Guard {
    /// The mask states
    daif: u64,
    /// For debug purposes
    should_drop: bool,
}

impl Guard {
    /// Creates a default mask state that should not be used directly
    /// # Safety
    /// This value should never be properly used: it is intended solely as a default
    pub const unsafe fn default() -> Self {
        Self {
            daif: 0,
            should_drop: false,
        }
    }

    /// C
    pub fn new() -> Self {
        let guard = Self {
            daif: DAIF.get(),
            should_drop: true,
        };
        DAIF.set(0);
        guard
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        assert!(self.should_drop);
        DAIF.set(self.daif);
    }
}
use crate::{call_once_per_core, kernel::exception::PrivilegeLevel};
use aarch64_cpu::{
    asm::barrier,
    registers::{CurrentEL, DAIF, ESR_EL1, SCTLR_EL1, SPSR_EL1, VBAR_EL1},
};
use tock_registers::{
    interfaces::{ReadWriteable, Readable, Writeable},
    registers::InMemoryRegister,
};

/// Wrapper structs for memory copies of registers.
#[repr(transparent)]
pub struct SpsrEL1(InMemoryRegister<u64, SPSR_EL1::Register>);
pub struct EsrEL1(InMemoryRegister<u64, ESR_EL1::Register>);

#[repr(C)]
pub struct ExceptionContext {
    /// General purpose registers.
    pub gpr: [u64; 30],

    /// Link register
    pub lr: u64,

    /// Exception link register
    pub elr_el1: u64,

    /// Saved program status
    pub spsr_el1: SpsrEL1,

    /// Exception syndrome register
    pub esr_el1: EsrEL1,
}

impl ExceptionContext {}

// The exception assembly
core::arch::global_asm!(include_str!("exception.s"));

/// Default exception handler
#[no_mangle]
extern "C" fn unhandled_exception(_context: &ExceptionContext) {
    panic!("Unhandled exception!\n");
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

pub fn init() -> () {}

/// Ready exception handling by setting the exception vector base address register.
pub fn per_core_init() -> () {
    call_once_per_core!();
    extern "Rust" {
        static _exception_vector: core::cell::UnsafeCell<()>;
    }

    VBAR_EL1.set(unsafe { _exception_vector.get() as u64 });

    // Force VBAR update to complete before next instruction.
    barrier::isb(barrier::SY);
}

/// I'm scared
pub fn enable() -> () {
    call_once_per_core!();
    assert!(
        DAIF.matches_all(DAIF::D::Masked + DAIF::A::Masked + DAIF::I::Masked + DAIF::F::Masked),
        "Interrupts must be disabled to enable them"
    );
    DAIF.write(DAIF::D::Unmasked + DAIF::A::Unmasked + DAIF::I::Unmasked + DAIF::F::Unmasked);

    SCTLR_EL1.modify(SCTLR_EL1::A::Enable);
}

pub struct ExceptionMasks {
    prior: u64,
}

/// Must be paired with a `restore`
pub unsafe fn disable() -> ExceptionMasks {
    let state = ExceptionMasks { prior: DAIF.get() };
    DAIF.set(0);
    state
}

/// Must be paired with a `disable`
pub unsafe fn restore(state: ExceptionMasks) -> () {
    DAIF.set(state.prior);
}

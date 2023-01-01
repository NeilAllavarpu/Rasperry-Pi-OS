use crate::{architecture, kernel, kernel::exception::PrivilegeLevel};
use aarch64_cpu::{
    asm::eret,
    registers::{CNTHCTL_EL2, CNTVOFF_EL2, ELR_EL2, HCR_EL2, SP, SPSR_EL2, SP_EL1},
};
use tock_registers::interfaces::{ReadWriteable, Readable, Writeable};

// The boot sequence
core::arch::global_asm!(include_str!("boot.s"));

/// Switches the core from EL2 to EL1\
/// Switches to the given stack pointer\
/// Jumps to the main init sequence\
#[no_mangle]
extern "C" fn el2_init() -> ! {
    // Make sure this is running in EL2
    assert_eq!(
        architecture::exception::el(),
        PrivilegeLevel::Hypervisor,
        "The boot sequence must be running in EL2"
    );
    // Enable 64 bit mode for EL1
    // Prevent exceptions from going to EL2
    HCR_EL2.modify(
        HCR_EL2::RW::EL1IsAarch64
            + HCR_EL2::TGE::DisableTrapGeneralExceptionsToEl2
            + HCR_EL2::E2H::DisableOsAtEl2
            + HCR_EL2::IMO::DisableVirtualIRQ
            + HCR_EL2::FMO::DisableVirtualFIQ
            + HCR_EL2::VM::Disable,
    );

    // Disable interrupts in EL1 mode, and switch the stack pointer on a per-exception level basis

    CNTHCTL_EL2.write(CNTHCTL_EL2::EL1PCEN::SET + CNTHCTL_EL2::EL1PCTEN::SET);
    CNTVOFF_EL2.set(0);
    SPSR_EL2.modify(
        SPSR_EL2::D::Masked
            + SPSR_EL2::A::Masked
            + SPSR_EL2::I::Masked
            + SPSR_EL2::F::Masked
            + SPSR_EL2::M::EL1h,
    );
    // Begin execution with the main init sequence
    ELR_EL2.set(architecture::usize_to_u64(
        #[allow(clippy::fn_to_numeric_cast_any)]
        #[allow(clippy::as_conversions)]
        (kernel::init as *const ()).to_bits(),
    ));
    // Set the stack pointer when execution resumes
    SP_EL1.set(SP.get());
    eret();
}

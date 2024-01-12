//! Instruction abort specific handling

use super::page_fault::StatusCode;
use crate::{
    exception::page_fault::{self, AccessType, PageFaultInfo},
    machine,
};
use bitfield_struct::bitfield;

/// The instruction syndrome whenever an Instruction Abort is taken
#[bitfield(u32)]
pub struct InstructionAbortIS {
    /// Level of translation at which the data abort occurred. Not always meaningful.
    #[bits(2)]
    level: u8,
    /// Status code indicating the cause of the data abort
    #[bits(4)]
    status_code: StatusCode,
    _res0: bool,
    /// For a stage 2 fault, indicates whether the fault was a stage 2 fault on an access made for a stage 1 translation table walk:
    was_stage_2: bool,
    ___res0: bool,
    /// External abort type. This bit can provide an IMPLEMENTATION DEFINED classification of
    /// External aborts.
    external_abort_type: bool,
    /// `FAR` not Valid, for a synchronous External abort other than a synchronous External abort
    /// on a translation table walk
    far_not_valid: bool,
    /// Load/Store Type. Used when a Translation fault, Access flag fault, or Permission fault
    /// generates a Data Abort.
    // #[bits(2)]
    // load_store_type: LoadStoreType,
    // __res0: bool,
    // /// Whether or not the data operation has acquire-release semantics
    // ///
    // /// This field is UNKNOWN when the value of ISV is UNKNOWN.
    // is_acquire_release: bool,
    // /// Sixty Four bit general-purpose register transfer. Width of the register accessed by the instruction is 64-bit.
    // ///
    // /// This field is UNKNOWN when the value of ISV is UNKNOWN.
    // is_64bit: bool,
    // /// Syndrome Register Transfer. The register number of the Wt/Xt/Rt operand of the faulting
    // /// instruction.
    // ///
    // /// If the exception was taken from an Exception level that is using AArch32, then this is the
    // /// AArch64 view of the register.
    // ///
    // /// This field is UNKNOWN when the value of ISV is UNKNOWN.
    // #[bits(5)]
    // destination_register: u8,
    // /// Syndrome Sign Extend. For a byte, halfword, or word load operation, indicates whether the
    // /// data item must be sign extended.
    // ///
    // /// This field is UNKNOWN when the value of ISV is UNKNOWN.
    // needs_sign_extension: bool,
    // /// Indicates the size of the access attempted by the faulting operation.
    // ///
    // /// This field is UNKNOWN when the value of ISV is UNKNOWN.
    // #[bits(2)]
    // access_size: u8,
    // /// Indicates whether the syndrome information in the next few bits is valid
    // instruction_syndrome_valid: bool,
    #[bits(14)]
    __: u32,
    #[bits(7)]
    ___: u32,
}

impl InstructionAbortIS {
    /// Gets the faulting address for an instruction abort, if valid
    fn faulting_address(self) -> Option<u64> {
        (!self.far_not_valid()).then(machine::faulting_address)
    }
}

/// Handles an instruciton abort
pub fn handle(iss: InstructionAbortIS, x0: usize, x1: usize) -> (usize, usize) {
    // assert!(iss.instruction_syndrome_valid());
    page_fault::resolve_page_fault(
        &PageFaultInfo {
            access_type: AccessType::Instruction,
            code: iss.status_code(),
            level: iss.level(),
            faulting_address: iss.faulting_address(),
            access_bytes: 4,
        },
        x0,
        x1,
    )
}

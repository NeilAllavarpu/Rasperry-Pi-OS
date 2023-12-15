//! Instruction abort specific handling

use crate::impl_u32;
use bitfield_struct::bitfield;
use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::{FromPrimitive, ToPrimitive};

/// The reason why the data abort was raised
#[derive(FromPrimitive, ToPrimitive, Debug)]
#[expect(clippy::missing_docs_in_private_items)]
enum InstructionFaultStatusCode {
    AddressSizeFault = 0b0000,
    TranslationFault = 0b0001,
    AccessFlagFault = 0b0010,
    PermissionFault = 0b0011,
    /// Synchronous External abort on translation table walk or hardware update of translation
    /// table
    SynchronousExternalAbort = 0b0101,
    AlignmentFault = 0b1000,
    Other,
}

/// The instruction syndrome whenever a Data Abort is taken
#[bitfield(u32)]
pub struct InstructionAbortIS {
    /// Level of translation at which the data abort occurred. Not always meaningful.
    #[bits(2)]
    level: u8,
    /// Status code indicating the cause of the data abort
    #[bits(4)]
    status_code: InstructionFaultStatusCode,
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
    fn faulting_address(&self) -> Option<usize> {
        (!self.far_not_valid()).then(|| {
            let far: usize;
            // SAFETY: This touches nothing but a read to FAR_EL1, safely
            unsafe {
                core::arch::asm! {
                    "mrs {}, FAR_EL1",
                    out(reg) far,
                    options(nomem, nostack, preserves_flags)
                };
            };
            far
        })
    }
}

/// Handles a data abort
pub fn handle(iss: InstructionAbortIS) -> i64 {
    panic!(
        "Faulting address {:X?}, ISS {iss:X?}",
        iss.faulting_address()
    )
}

impl_u32!(InstructionFaultStatusCode);

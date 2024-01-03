//! Primary exception handlers

use crate::exception::svc::CallCode;
use crate::println;
use bitfield_struct::bitfield;
use core::arch::{asm, global_asm};
use core::fmt;
use macros::AsBits;

use svc::Return;

mod data_abort;
mod gic;
mod instruction_abort;
pub mod page_fault;
mod svc;

/// Indicates the reason for the exception that `ESR_EL1` holds information about
#[derive(AsBits, Debug)]
#[repr(u64)]
enum ExceptionClass {
    /// Unknown reason
    Unknown = 0b000_000,
    /// Trapped `WF*` instruction execution
    ///
    /// Conditional `WF*` instructions that fail their condition code check do not cause an
    /// exception
    TrappedWfiWfe = 0b000_001,
    /// (AArch32) Trapped `MCR` or `MRC` access with `coproc == 0b1111` that is not reported using
    /// `Unknown`.
    TrappedMcrMrc = 0b000_011,
    /// (AArch32) Trapped `MCRR` or `MRRC` access with `(coproc == 0b1111)` that is not reported
    /// using `Unknown`
    TrappedMcrrMrrc = 0b000_100,
    /// (AArch32) Trapped `MCR` or `MRC` access with `coproc == 0b1110`.
    TrappedMcrMrc2 = 0b000_101,
    /// (AArch32) Trapped `LDC` or `STC` access
    ///
    /// The only architected uses of these instruction are:
    /// * An `STC` to write data to memory from `DBGDTRRXint`
    /// * An `LDC` to read data from memory to `DBGDTRTXint`
    TrappedLdcStc = 0b000_110,
    /// Access to SME, SVE, Advanced SIMD or floating-point functionality trapped by
    /// `CPACR_EL1.FPEN`
    ///
    /// Excludes exceptions because SVE or Advanced SIMD and floating-point are not implemented.
    /// These are reported with `Unknown`.
    TrappedSmeSveSimdFp = 0b000_111,
    /// (FEAT_LS64) Trapped execution of an `LD64B` or `ST64B*` instruction
    TrappedLdSt64b = 0b001_010,
    /// (AArch32) Trapped `MRRC` access with `coproc == 0b1110`
    TrappedMrrc2 = 0b001_100,
    /// (FEAT_BTI)
    BranchTargetException = 0b001_101,
    IllegalExecutionState = 0b001_110,
    /// (AArch32) `SVC` instruction execution in AArch32 state
    SvcAArch32 = 0b010_001,
    /// (AArch64) `SVC` instruction execution in AArch64 state
    SvcAArch64 = 0b010_101,
    /// (AArch64) Trapped `MSR`, `MRS` or System instruction execution in AArch64 state, that is
    /// not reported using `Unknown`, `TrappedWfiWfe` or `TrappedSmeSveSimdFp`
    ///
    /// This includes all instructions that cause exceptions that are part of the encoding space
    /// defined in System instruction class encoding overview, except for those exceptions reported
    /// using EC values `Unknown, `TrappedWfiWfe`, or `TrappedSmeSveSimdFp`
    TrappedMsrMrsSystem = 0b011_000,
    /// (FEAT_SVE) Access to SVE functionality trapped as a result of `CPACR_EL1.ZEN`, that is not
    /// reported using `Unknown`.
    TrappedSve = 0b011_001,
    /// (FEAT_TME) Exception from an access to a `TSTART` instruction at EL0 when
    /// `SCTLR_EL1.TME0 == 0` or at EL1 when `SCTLR_EL1.TME == 0`
    TrappedTstart = 0b011_011,
    /// (FEAT_FPAC) Exception from a Pointer Authentication instruction authentication failure
    PointerAuthInsFail = 0b011_100,
    /// (FEAT_SME) Access to SME functionality trapped as a result of `CPACR_EL1.SMEN` or an
    /// attempted execution of an instruction that is illegal because of the value of `PSTATE.SM`
    /// or `PSTATE.ZA`, that is not reported using `Unknown`
    TrappedSme = 0b011_101,
    /// (FEAT_RME) Exception from a Granule Protection Check
    GranuleProtetctionCheck = 0b011_110,
    /// Instruction Abort from EL0
    ///
    /// Used for MMU faults generated by instruction accesses and synchronous External aborts,
    /// including synchronous parity or ECC errors. Not used for debug-related exceptions.
    InstructionAbortEL0 = 0b100_000,
    /// Instruction Abort from EL1
    ///
    /// Used for MMU faults generated by instruction accesses and synchronous External aborts,
    /// including synchronous parity or ECC errors. Not used for debug-related exceptions.
    InstructionAbortEl1 = 0b100_001,
    /// PC Alignment Fault Exception
    PCAlignmentFault = 0b100_010,
    /// Data Abort from EL0
    ///
    /// Used for MMU faults generated by data accesses, alignment faults other than those caused by
    /// Stack Pointer misalignment, and synchronous External aborts, including synchronous parity
    /// or ECC errors. Not used for debug-related exceptions.
    DataAbortEL0 = 0b100_100,
    /// Data Abort from EL1
    ///
    /// Used for MMU faults generated by data accesses, alignment faults other than those caused by
    /// Stack Pointer misalignment, and synchronous External aborts, including synchronous parity
    /// or ECC errors. Not used for debug-related exceptions.
    DataAbortEL1 = 0b100_101,
    /// SP alignment fault exception
    SPAlignmentFault = 0b100_110,
    /// (FEAT_MOPS)
    MemoryOperationException = 0b100_111,
    /// (AArch32) Trapped floating-point exception taken from AArch32 state
    ///
    /// This EC value is valid if the implementation supports trapping of floating-point
    /// exceptions, otherwise it is reserved. Whether a floating-point implementation supports
    /// trapping of floating-point exceptions is IMPLEMENTATION DEFINED.
    TrappedFPAarch32 = 0b101_000,
    /// (AArch32) Trapped floating-point exception taken from AArch64 state
    ///
    /// This EC value is valid if the implementation supports trapping of floating-point
    /// exceptions, otherwise it is reserved. Whether a floating-point implementation supports
    /// trapping of floating-point exceptions is IMPLEMENTATION DEFINED.
    TrappedFPAarch64 = 0b101_100,
    /// SError interrupt
    SError = 0b101_111,
    /// Breakpoint exception from EL0
    BreakpointEL0 = 0b110_000,
    /// Breakpoint exception from EL1
    BreakpointEL1 = 0b110_001,
    /// Software Step exception from EL0
    SoftwareStepEL0 = 0b110_010,
    /// Software Step exception from EL1
    SoftwareStepEL1 = 0b110_011,
    /// Watchpoint exception from EL0
    WatchpointEL0 = 0b110_100,
    /// Watchpoint exception from EL1
    WatchpointEL1 = 0b110_101,
    /// (AArch32) `BKPT` instruction execution in AArch32 state
    BkptAarch32 = 0b111_000,
    /// (AArch64) `BRK` instruction execution in AArch64 state.
    BrkAarch64 = 0b111_100,
}

/// Instruction Length for synchronous exceptions
#[derive(AsBits, Debug)]
#[repr(u64)]
enum InstructionLength {
    Bit16 = 0,
    /// 32-bit instruction trapped. This value is also used when the exception is one of the following:
    /// * An SError interrupt
    /// * An Instruction Abort exception
    /// * A PC alignment fault exception
    /// * An SP alignment fault exception
    /// * A Data Abort exception for which the value of the ISV bit is 0
    /// * An Illegal Execution state exception.
    /// * Any debug exception except for Breakpoint instruction exceptions
    /// * An exception reported using `Unknown`
    Bit32 = 1,
}

// primitive_enum! {
//     InstructionLength, u32,
//     /// 16 bit instruction trapped
//     Bit16 = 0,
//     /// 16 bit instruction trapped
//     Bit32 = 1,
// }

/// Encodes the various possible instruction syndromes as an enum
#[repr(C)]
union InstructionSyndrome {
    /// Data abort instruction syndrome
    data_abort: data_abort::DataAbortIS,
    instruction_abort: instruction_abort::InstructionAbortIS,
    /// SVC instruction syndrome
    svc: svc::SvcIS,
    /// Raw bits for the instruction syndrome. Only the lower 25 bits are meaningful
    raw: u32,
}

impl InstructionSyndrome {
    const fn into_bits(self) -> u64 {
        (unsafe { self.raw }) as u64
    }
    const fn from_bits(value: u64) -> Self {
        Self { raw: value as u32 }
    }
}

#[bitfield(u64)]
struct ExceptionSyndrome {
    #[bits(25)]
    instruction_syndrome: InstructionSyndrome,
    #[bits(1)]
    instruction_length: InstructionLength,
    #[bits(6)]
    exception_class: ExceptionClass,
    #[bits(5)]
    instruction_syndrome_2: u8,
    #[bits(27)]
    _res0: u32,
}

/// The main handler for synchronous EL0 exceptions. Dispatches to sub-handlers in other files
/// Does **not** include `SVC`s
extern "C" fn synchronous_exception_from_el0(x0: u64, x1: u64) {
    let esr: u64;
    // SAFETY: This does not touch anything but ESR_EL1 to safely read its value
    unsafe {
        core::arch::asm! {
            "mrs {}, ESR_EL1",
            out(reg) esr,
            options(nomem, nostack, preserves_flags)
        };
    };

    let esr = ExceptionSyndrome::from(esr);
    let iss = esr.instruction_syndrome();

    #[expect(clippy::wildcard_enum_match_arm)]
    match esr.exception_class() {
        ExceptionClass::DataAbortEL0 | ExceptionClass::DataAbortEL1 => {
            data_abort::handle(
                // SAFETY: This is the correct ISS and set validly
                unsafe { iss.data_abort },
            );
        }
        ExceptionClass::SvcAArch64 => {
            assert_eq!(unsafe { iss.svc }.code(), CallCode::Eret);
            svc::eret_handle(x0, x1)
        }
        ExceptionClass::InstructionAbortEL0 => {
            instruction_abort::handle(
                // SAFETY: This is the correct ISS and set validly
                unsafe { iss.instruction_abort },
            );
        }
        ExceptionClass::BreakpointEL1
        | ExceptionClass::SoftwareStepEL1
        | ExceptionClass::WatchpointEL1
        | ExceptionClass::InstructionAbortEl1 => {
            unreachable!("EL1 exception should not reach the EL0 handler")
        }
        _ => todo!("Handle {:X?}", esr),
    }
}

global_asm!(
    include_str!("./exception.s"),
    from_sp_el0 = sym exception_from_sp_el0,
    irq = sym irq_exception,
    fiq = sym fiq_exception,
    serror = sym serror_exception,
    synchronous = sym synchronous_exception_from_el0,
    SVC_CODE = const ExceptionClass::SvcAArch64 as u64,
    aarch32 = sym exception_aarch32,
    svc = sym svc::handle,
);

/// Sets up exception handling on the current core
pub fn init() {
    extern "C" {
        static _exception_vector: *const extern "C" fn() -> !;
    }
    // SAFETY: This touches nothing but VBAR_EL1 to configure it correctly
    unsafe {
        asm! {
            "msr VBAR_EL1, {}",
            in(reg) core::ptr::addr_of!(_exception_vector),
            options(nomem, nostack, preserves_flags),
        };
    };
    gic::init();
}

/// Handles any IRQ exceptions
extern "C" fn irq_exception() {
    let interrupt_info =
        unsafe { core::ptr::read_volatile((0xFFFF_FFFF_FE64_2000_usize + 0x000C) as *mut u32) };

    // preemption
    if interrupt_info & ((1 << 10) - 1) == 30 {
        let freq: u64;
        unsafe {
            asm!("mrs {}, CNTFRQ_EL0", out(reg) freq);
        }
        unsafe {
            asm!("msr CNTP_TVAL_EL0, {}", in(reg) freq);
        }

        unsafe {
            core::ptr::write_volatile(
                (0xFFFF_FFFF_FE64_2000_usize + 0x0010) as *mut u32,
                interrupt_info,
            )
        }; // eoir
        println!("Handle IRQ {}", interrupt_info);
    } else {
        todo!("Handle IRQ {:X}", interrupt_info);
    }
}

/// Handles any exceptions should `SP_EL0` be erroneously used
extern "C" fn exception_from_sp_el0() -> ! {
    unreachable!("SP_EL0 should never be used at higher exception levels");
}

/// Handles any FIQs should any be erroneously triggered
extern "C" fn fiq_exception() -> ! {
    unreachable!("FIQs should never be triggered");
}

/// Handles any `SErrors` should any be fatally triggered
extern "C" fn serror_exception() -> ! {
    unimplemented!("SErrors are not currently supported");
}

/// Handles any `AArch32` exceptions should any be erroneously triggered
extern "C" fn exception_aarch32() -> ! {
    unimplemented!("AArch32 execution is not currently supported");
}

impl fmt::Debug for InstructionSyndrome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("Instruction Syndrome")
            .field(
                // SAFETY: The raw 32 bit value is always valid to read
                unsafe { &self.raw },
            )
            .finish()
    }
}

impl From<u64> for InstructionSyndrome {
    fn from(value: u64) -> Self {
        InstructionSyndrome {
            #[expect(clippy::expect_used, reason = "This conversion should never fail")]
            raw: u32::try_from(value)
                .expect("Instruction syndromes should not exceed 32 bit values"),
        }
    }
}

impl From<InstructionSyndrome> for u64 {
    fn from(value: InstructionSyndrome) -> Self {
        // SAFETY: The raw 32 bit value is always valid to read
        unsafe { value.raw }.into()
    }
}

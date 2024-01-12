use core::{arch::asm, ptr};

use macros::AsBits;

use crate::{
    execution::{self, ExceptionCode},
    machine::faulting_address,
    println,
};

/// Access type that caused the page fault
#[derive(Clone, Copy, Debug)]
pub(super) enum AccessType {
    /// Loads from memory
    Load,
    /// Stores to memory
    Store,
    /// Instruction fetches
    Instruction,
}

/// The status code describing the variant of page fault
#[derive(AsBits, Debug, Clone, Copy)]
#[repr(u32)]
pub(crate) enum StatusCode {
    AddressSizeFault = 0b0000,
    /// Automatically triggered on a TLB miss
    TranslationFault = 0b0001,
    AccessFlagFault = 0b0010,
    PermissionFault = 0b0011,
    /// Synchronous External abort on translation table walk or hardware update of translation table
    SynchronousExternalAbort = 0b0101,
    AlignmentFault = 0b1000,
}

/// Information describing the source and cause of a page fault
#[derive(Debug)]
pub(super) struct PageFaultInfo {
    /// Memory access type that caused the page fault
    pub access_type: AccessType,
    /// Variant of page fault
    pub code: StatusCode,
    /// Level of translation at which the page fault triggered. Not always meaningful.
    pub level: u8,
    /// Faulting address that caused the page fault, if applicable
    pub faulting_address: Option<u64>,
    /// Byte size of the access that caused the page fault
    pub access_bytes: u8,
}

/// Resolves a page fault by either autofilling the translation, or invoking the execution's page fault handler
pub(super) fn resolve_page_fault(info: &PageFaultInfo, x0: usize, x1: usize) -> (usize, usize) {
    println!("PAGE FAULT: {:X?}", info.faulting_address);
    let current = execution::current()
        .expect("Page faults should not trigger outside the context of an `Execution`");
    let call_signal = {
        if let StatusCode::TranslationFault = info.code {
            // CHECK: Is it possible for a translation fault to have an invalid FAR?
            let addr = usize::try_from(info.faulting_address.unwrap())
                .expect("`u64` should always be a valid `usize`");
            current.with_autotranslate(|| {
                let failed_translation = if let AccessType::Store = info.access_type {
                    current
                        .validate_user_pointer_writeable(ptr::invalid::<u64>(addr))
                        .is_none()
                } else {
                    current
                        .validate_user_pointer(ptr::invalid::<u64>(addr))
                        .is_none()
                };
                if failed_translation {
                    // SAFETY: TLB invalidations are always safe
                    unsafe {
                        asm! {
                            "tlbi VAE1IS, {}",
                            in(reg) (addr >> 12) & ((1 << 36) - 1),
                            options(nomem, nostack, preserves_flags)
                        };
                    }
                }
                failed_translation
            })
        } else {
            true
        }
    };
    if call_signal {
        println!("Call signal handler!");
        unsafe { current.prepare_synchronous_jump(x0, x1) };
        (
            ExceptionCode::PageFault as usize,
            faulting_address().try_into().unwrap(),
        )
    } else {
        (x0, x1)
    }
    // Else, the TLB refill is valid, so we can simply return to usermode
}

//! Information about various features of the machine
use core::arch::asm;

/// Converts a virtual address into a physical address, if there is a valid mapping
pub fn to_physical_addr(addr: usize) -> Option<u64> {
    let mut par_el1: u64;
    // SAFETY: These instructions do not write to anything and only affect the involved registers based on memory in translation tables
    unsafe {
        asm! {
           "at S1E1R, {register}",
           "mrs {register}, PAR_EL1",
           register = inlateout(reg) addr => par_el1,
           options(readonly, nostack, preserves_flags)
        };
    }
    if par_el1 & 1 == 1 {
        None
    } else {
        Some(
            (par_el1 & 0x000F_FFFF_FFFF_F000)
                | u64::try_from(addr & 0xFFF)
                    .expect("12 bit value always fits into a 64 bit value"),
        )
    }
}

/// Returns a unique numeric ID for the current core
pub fn core_id() -> u8 {
    let mpidr_el1: u64;
    unsafe {
        asm! {
            "mrs {}, MPIDR_EL1",
            out(reg) mpidr_el1,
            options(nomem, nostack, pure, preserves_flags),
        }
    }
    u8::try_from(mpidr_el1 & 0b11).expect("Core ID should fit into a u8")
}

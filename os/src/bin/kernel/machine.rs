//! Information about various features of the machine
use core::arch::asm;

use bitfield_struct::bitfield;
use macros::AsBits;

use crate::exception::page_fault::StatusCode;

/// Shareability attributes describing a memory region
#[repr(u64)]
#[derive(Debug, AsBits)]
enum Shareability {
    Non = 0b00,
    Outer = 0b10,
    Inner = 0b11,
}
#[bitfield(u64)]
pub struct ValidAddr {
    _fault: bool, // should be 0 here
    #[bits(6)]
    __: u8,
    #[bits(2)]
    shareability: Shareability,
    _ns: bool,
    _impl_defined: bool,
    _nse: bool,
    #[bits(40)]
    pa_shifted: u64,
    #[bits(4)]
    ___: u8,
    attr: u8,
}

impl ValidAddr {
    /// Returns the (properly shifted) virtual address, excluding any offset
    pub const fn pa(self) -> u64 {
        self.pa_shifted() << 12
    }
}
#[bitfield(u32)]
pub struct InvalidAddr {
    _fault: bool, // should be 1 here
    #[bits(2)]
    level: u8,
    #[bits(4)]
    status: StatusCode,
    #[bits(25)]
    ___: u32,
}

/// Converts a virtual address into a physical address, if there is a valid mapping
pub fn to_physical_addr(addr: usize) -> Result<ValidAddr, InvalidAddr> {
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
        Err(InvalidAddr(
            u32::try_from(par_el1 & u64::from(u32::MAX))
                .expect("Mask should coerce the `u32` to be in bounds"),
        ))
    } else {
        Ok(ValidAddr(par_el1))
    }
}

/// Returns `FAR_EL1`
pub fn faulting_address() -> u64 {
    let far;
    // SAFETY: This touches nothing but a read to FAR_EL1, safely
    unsafe {
        core::arch::asm! {
            "mrs {}, FAR_EL1",
            out(reg) far,
            options(nomem, nostack, preserves_flags)
        };
    };
    far
}

/// Returns a unique numeric ID for the current core
pub fn core_id() -> u8 {
    let mpidr_el1: u64;
    unsafe {
        asm! {
            "mrs {}, MPIDR_EL1",
            out(reg) mpidr_el1,
            options(nomem, nostack, preserves_flags),
        }
    }
    u8::try_from(mpidr_el1 & 0b11).expect("Core ID should fit into a u8")
}

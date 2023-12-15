//! Information about various features of the machine
use core::{
    alloc::Allocator,
    arch::{
        aarch64::{self, OSH},
        asm,
    },
    ffi::CStr,
    fmt::Debug,
    mem::{self, ManuallyDrop, MaybeUninit},
    num::NonZeroUsize,
    ptr::{addr_of, NonNull},
    slice,
};
use device_tree::{dtb::DeviceTree, node::Node};

use alloc::{borrow::ToOwned, boxed::Box, collections::BTreeMap, string::String, vec::Vec};
use stdos::cell::OnceLock;

use crate::{
    boot::{TABLE_ENTRY_BASE, TRANSLATION_TABLE},
    mailbox::Mailbox,
    print, println,
};

#[derive(Debug)]
pub struct MachineInfo {
    pub cores: u8,
    pub memory: Box<[(u64, u64)]>,
}

pub static INFO: OnceLock<MachineInfo> = OnceLock::new();

pub fn to_physical_addr(addr: usize) -> Option<u64> {
    let mut par_el1: u64;
    unsafe {
        asm! {
           "at S1E1R, {}",
           "mrs {}, PAR_EL1",
           in(reg) addr,
           out(reg) par_el1,
           options(nomem, nostack, preserves_flags)
        }
    };
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

/// Initializes the global `INFO` with machine-derived information
pub fn get_info(device_tree_address: NonNull<u64>) {
    const PAGE_MASK: u64 = !((1 << 21) - 1);
    const OTHER_MASK: u64 = !PAGE_MASK;
    const MAILBOX_VADDR: usize = 0xFFFF_FFFF_FE40_0000 + 0xB880;

    // https://developer.arm.com/documentation/100095/0003/System-Control/AArch64-register-descriptions/L2-Control-Register--EL1
    let cores_reg: u64;
    // SAFETY: This instruction is well-defined on the target here
    unsafe {
        asm! {
            "mrs {}, S3_1_c11_c0_2",
            out(reg) cores_reg,
            options(nomem, nostack, pure, preserves_flags),
        }
    };

    let device_tree = DeviceTree::from_bytes(unsafe {
        NonNull::slice_from_raw_parts(device_tree_address, 0x2000).as_ref()
    })
    .unwrap();

    INFO.set(MachineInfo {
        cores: u8::try_from(1 + ((cores_reg >> 24) & 0b11))
            .expect("Number of cores should be 1, 2, 3, or 4"),
        memory: device_tree
            .root()
            .memory()
            .into_iter()
            .flat_map(|region| region.regions())
            .map(|&(start, size)| (start, start + size))
            .collect(),
    })
    .expect("Machine only be initialized once");
}

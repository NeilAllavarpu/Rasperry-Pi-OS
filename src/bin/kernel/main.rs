//! The privileged kernel of the operating system
//!
//! This manages hardware resources at as basic a level as possible. This includes control over
//! physical pages (but not virtual memory), scheduling timeslices, and interrupts.

#![no_main]
#![no_std]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::complexity)]
#![warn(clippy::correctness)]
#![warn(clippy::pedantic)]
#![warn(clippy::suspicious)]
#![warn(clippy::perf)]
#![warn(clippy::style)]
#![allow(clippy::blanket_clippy_restriction_lints)]
#![warn(clippy::restriction)]
#![feature(asm_const)]
#![feature(const_nonnull_new)]
#![feature(const_option)]
#![feature(generic_arg_infer)]
#![feature(naked_functions)]
#![feature(stdsimd)]
#![feature(strict_provenance)]
#![feature(panic_info_message)]
#![feature(pointer_byte_offsets)]
#![feature(ptr_mask)]
#![feature(ptr_metadata)]
#![allow(clippy::inline_asm_x86_intel_syntax)]
#![allow(clippy::mod_module_files)]

use core::arch::aarch64::{__wfe, __wfi};
use core::arch::asm;
use core::num::NonZeroUsize;
use core::ptr::NonNull;
use core::sync::atomic::AtomicBool;
use core::sync::atomic::Ordering;
use stdos::cell::InitCell;
use stdos::heap::{AllocatorBackend, BuddyAllocator};
use stdos::os::vm::load_elf;
use stdos::os::vm::AddressSpace;

mod boot;
mod memory_layout;

use memory_layout::{FS_ELF, FS_TRANSLATION_TABLE};

struct Backend;
impl AllocatorBackend for Backend {
    fn grow(&mut self, _: NonNull<()>, _: NonZeroUsize) -> bool {
        false
    }
}

extern crate alloc;

#[global_allocator]
static ALLOCATOR: InitCell<BuddyAllocator<Backend>> = InitCell::new();

/// The primary initialization sequence for the kernel in EL1, that only runs on one core. This
/// also prepares to launch the file system process
extern "C" fn init(elf_size: usize) -> ! {
    static IS_FIRST: AtomicBool = AtomicBool::new(true);
    static IS_INITIALIZING: AtomicBool = AtomicBool::new(true);

    if IS_FIRST.swap(false, Ordering::Relaxed) {
        let mut address_space = unsafe { AddressSpace::<16, 25>::new(FS_TRANSLATION_TABLE.va) };
        let (entry, bss_start, bss_end) = load_elf(
            &mut address_space,
            unsafe { NonNull::from_raw_parts(FS_ELF.va, elf_size).as_ref() },
            FS_ELF.pa.try_into().unwrap(),
        )
        .unwrap();

        // SAFETY: Both addresses are aligned
        unsafe {
            address_space.map_range(0x1FF_0000, 0, 0x1_0000, true, false, false);
        }

        IS_INITIALIZING.store(false, Ordering::Release);

        unsafe {
            asm!(
                "msr ttbr0_el1, {}",
                "isb sy",
                "br {}",
                in (reg) FS_TRANSLATION_TABLE.pa,
                in (reg) entry,
                in ("x0") 0x1FF_4000_u64,
                in ("x1") bss_start,
                in ("x2") bss_end,
                options(noreturn)
            );
        }
    } else {
        while IS_INITIALIZING.load(Ordering::Acquire) {
            // SAFETY: Executing `wfe` is a safe delay
            unsafe { __wfe() };
        }

        loop {}
    }
}

/// Panics are unhandled error conditions - the entire system may be forced to shut down
#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {
        // SAFETY: Executing `wfi` is a safe delay
        unsafe { __wfi() }
    }
}

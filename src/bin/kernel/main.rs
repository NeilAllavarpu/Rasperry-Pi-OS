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
#![feature(atomic_from_ptr)]
#![feature(const_nonnull_new)]
#![feature(const_option)]
#![feature(generic_arg_infer)]
#![feature(lint_reasons)]
#![feature(naked_functions)]
#![feature(stdsimd)]
#![feature(strict_provenance)]
#![feature(sync_unsafe_cell)]
#![feature(never_type)]
#![feature(panic_info_message)]
#![feature(pointer_byte_offsets)]
#![feature(ptr_mask)]
#![feature(ptr_metadata)]
#![feature(stmt_expr_attributes)]
#![allow(clippy::implicit_return)]
#![allow(clippy::inline_asm_x86_intel_syntax)]
#![allow(clippy::mod_module_files)]
#![allow(clippy::semicolon_outside_block)]
#![feature(const_ptr_as_ref)]
#![feature(pointer_is_aligned)]
#![feature(const_pointer_is_aligned)]
#![feature(const_mut_refs)]
#![allow(clippy::allow_attributes)]
use core::arch::aarch64::{__wfe, __wfi};
use core::arch::asm;
use core::fmt::Write;
use core::num::NonZeroUsize;
use core::ptr::NonNull;
use core::slice;
use core::sync::atomic::Ordering;
use core::sync::atomic::{AtomicBool, AtomicU8};
use stdos::cell::InitCell;
use stdos::heap::{AllocatorBackend, BuddyAllocator};
use stdos::os::vm::load_elf;
use stdos::os::vm::AddressSpace;

mod boot;
mod execution;
mod memory_layout;

use memory_layout::{FS_ELF, FS_TRANSLATION_TABLE};

struct Backend;
impl AllocatorBackend for Backend {
    fn grow(&mut self, _: NonNull<()>, _: NonZeroUsize) -> bool {
        false
    }
}

extern crate alloc;

pub const FS_TTBR0: u64 = 0x0;
pub const FS_TTBR0_VIRTUAL: u64 = 0x0;

#[global_allocator]
static ALLOCATOR: InitCell<BuddyAllocator<Backend>> = InitCell::new();
#[no_mangle]
fn main() -> () {}
/// The primary initialization sequence for the kernel in EL1, that only runs on one core. This
/// also prepares to launch the file system process
extern "C" fn init() -> ! {
    static IS_NOT_FIRST: AtomicBool = AtomicBool::new(false);
    static IS_INITIALIZED: AtomicBool = AtomicBool::new(false);
    static C: AtomicU8 = AtomicU8::new(0);
    let v = C.fetch_add(1, Ordering::Relaxed);
    if v != 0 {
        //IS_NOT_FIRST.swap(true, Ordering::Relaxed) {
        //while !IS_INITIALIZED.load(Ordering::Acquire) {
        loop {
            // SAFETY: Executing `wfe` is a safe delay
            unsafe { __wfe() }
        }
    } else {
        extern "Rust" {
            static __bss_end: ();
        }
        const PAGE_SIZE_1: usize = (1 << 16) - 1;
        let heap_start = unsafe { NonNull::from(&__bss_end) };
        let heap_end = heap_start.map_addr(|addr| {
            NonZeroUsize::new((addr.saturating_add(PAGE_SIZE_1)).get() & !PAGE_SIZE_1).unwrap()
        });
        //unsafe { ALLOCATOR.set(BuddyAllocator::new(heap_start, heap_end, Backend {}).unwrap()) };
        let mut address_space = unsafe { AddressSpace::<16, 25>::new(FS_TRANSLATION_TABLE.va) };
        let (entry, bss_start, bss_end) = load_elf(
            &mut address_space,
            unsafe { NonNull::from_raw_parts(FS_ELF.va, FS_ELF.size.get()).as_ref() },
            unsafe { FS_ELF.pa }.try_into().unwrap(),
        )
        .expect("File system ELF file should be valid");

        // SAFETY: Both addresses are aligned
        unsafe {
            address_space.map_range(0x1FF_0000, 0, 0x1_0000, true, false, false);
        }

        IS_INITIALIZED.store(true, Ordering::Release);

        unsafe {
            asm!(
                "sev",
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

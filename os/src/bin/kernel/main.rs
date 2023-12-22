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
#![expect(clippy::blanket_clippy_restriction_lints)]
#![warn(clippy::restriction)]
#![expect(clippy::allow_attributes_without_reason)]
#![expect(clippy::default_numeric_fallback)]
#![expect(clippy::implicit_return)]
#![expect(clippy::inline_asm_x86_intel_syntax)]
#![expect(clippy::question_mark_used)]
#![expect(clippy::semicolon_outside_block)]
#![expect(clippy::separated_literal_suffix)]
#![expect(clippy::mod_module_files)]
#![expect(clippy::shadow_reuse)]
#![expect(clippy::single_call_fn)]
#![expect(clippy::unimplemented)]
#![expect(clippy::unreachable)]
#![expect(clippy::expect_used)]
#![expect(clippy::pub_with_shorthand)]
#![feature(allocator_api)]
#![feature(alloc_layout_extra)]
#![feature(asm_const)]
#![feature(const_mut_refs)]
#![feature(exposed_provenance)]
#![feature(generic_arg_infer)]
#![feature(iterator_try_collect)]
#![feature(lint_reasons)]
#![feature(naked_functions)]
#![feature(panic_info_message)]
#![feature(pointer_is_aligned)]
#![feature(slice_ptr_get)]
#![feature(stdsimd)]
#![feature(stmt_expr_attributes)]
#![feature(strict_provenance)]
#![feature(strict_provenance_atomic_ptr)]

use alloc::sync::Arc;
use bump_allocator::BumpAllocator;
use core::arch::asm;
use core::fmt::Write;
use core::num::NonZeroUsize;
use core::panic::PanicInfo;
use core::ptr::{self, addr_of_mut, NonNull};
use core::sync::atomic::{AtomicBool, AtomicPtr, AtomicU8, AtomicUsize, Ordering};
use core::{hint, mem};
use device_tree::dtb::DeviceTree;
use stdos::cell::OnceLock;
use stdos::sync::SpinLock;

mod boot;
mod bump_allocator;
mod exception;
mod execution;
mod machine;
mod mailbox;
mod memory;
mod uart;
use uart::Uart;

extern crate alloc;

use crate::boot::STACK_SIZE;
use crate::execution::{ExceptionCode, Execution, UserContext};
use crate::memory::PAGE_ALLOCATOR;

/// Physical address of the init program's top-level translation table
const INIT_TRANSLATION_ADDRESS: u64 = 0x0;

/// The global UART for all prints
static UART: OnceLock<SpinLock<Uart>> = OnceLock::new();

#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        writeln!(&mut $crate::UART.get().expect("UART should be initialized").lock(), $($arg)*).unwrap();
    }};
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        write!(&mut $crate::UART.get().expect("UART should be initialized").lock(), $($arg)*).unwrap();
    }};
}

/// The global heap allocator for the kernel
#[global_allocator]
static mut KERNEL_ALLOCATOR: BumpAllocator = BumpAllocator::empty();

/// ABI-required alignment of the stack pointer
const STACK_ALIGNMENT: usize = 16;

/// The primary initialization sequence for the kernel in EL1
extern "C" fn main(device_tree_address: *mut u64, device_tree_size: usize) -> ! {
    /// Set only when the global initialization sequence (stuff that only runs once total) is
    /// finished
    static GLOBAL_SETUP_DONE: AtomicBool = AtomicBool::new(false);
    static NUM_READY: AtomicU8 = AtomicU8::new(0);
    NUM_READY.fetch_add(1, Ordering::Relaxed);
    if machine::core_id() == 0 {
        extern "C" {
            static mut __bss_end: u8;
        }

        exception::init();

        let mut uart =
                // SAFETY: This points to a valid, permanent UART register map in memory. No other
                // code accesses this concurrently
                unsafe { Uart::new(NonZeroUsize::new(0xFFFF_FFFF_FE20_1000).expect("Value is nonzero")) }.expect("Should be a valid MMIO UART");

        writeln!(&mut uart, "What just happened? Why am I here?").unwrap();
        assert!(
            matches!(UART.set(SpinLock::new(uart)), Ok(())),
            "UART should not already be initialized"
        );

        let stack_end = NonNull::new(
            // SAFETY: This is only used to derive a pointer, and so is always safe
            unsafe { addr_of_mut!(__bss_end) },
        )
        .expect("Stacks should not be at null addresses")
        .map_addr(|bss_end| {
            bss_end
                .get()
                .checked_next_multiple_of(STACK_ALIGNMENT) // Align the stack pointer
                .and_then(|y| y.checked_add(STACK_SIZE * 4)) // Add space for the stack
                .and_then(NonZeroUsize::new)
                .expect("Stack region should be nonnull and in bounds")
        });

        let mut heap = NonNull::slice_from_raw_parts(
            stack_end,
            0xFFFF_FFFF_FE20_0000_usize
                .checked_sub(stack_end.addr().get())
                .expect("Stack end should fit in main kernel memory"),
        );
        // SAFETY: This memory should not be in use by any other part of this program
        let heap = unsafe { heap.as_mut() };

        // SAFETY: The heap should not be in use by anyone else yet, and has not allocated anything yet
        unsafe {
            KERNEL_ALLOCATOR.set(heap);
        }

        // let device_tree_address = device_tree_address
        let device_tree_address =
            NonNull::new(device_tree_address).expect("Device tree addresses should be nonnull");
        assert!(
            u16::try_from(device_tree_size).is_ok(),
            "Device tree should fit into a single 64K page"
        );
        assert!(
            device_tree_address.is_aligned(),
            "Device tree should be aligned to an 8-byte boundary",
        );

        let device_tree_memory = NonNull::slice_from_raw_parts(
            device_tree_address,
            device_tree_size.div_ceil(mem::size_of::<u64>()),
        );
        // SAFETY:
        // * The boot sequence promises that this memory will not be modified by anyone but us, and we are not modifying it anywhere else
        // * We have verified above that the pointer is aligned
        // * The value is properly initialized
        let device_tree_memory = unsafe { device_tree_memory.as_ref() };

        // .expect("Device tree address should be nonnull and correctly in virtual memory");
        // let info = machine::get_info(device_tree_address);
        let device_tree =
            DeviceTree::from_bytes(device_tree_memory).expect("Device tree should be valid");

        // TODO: Incorporate reserved memory regions from the device tree
        unsafe {
            memory::init(
                device_tree.root().memory().iter().flat_map(|region| {
                    region.regions().iter().map(|&(start, size)| {
                        (
                            start,
                            start
                                .checked_add(size)
                                .expect("Memory regions should not overflow"),
                        )
                    })
                }),
                &[(0x8_0000, 0x18_0000)].iter().copied(),
            );
        }

        // TODO: better mechanism...
        let page = PAGE_ALLOCATOR.get().unwrap().alloc().unwrap();
        assert_eq!(page.addr(), 0);

        GLOBAL_SETUP_DONE.store(true, Ordering::Release);

        let ctx_ptr = ptr::from_exposed_addr_mut::<UserContext>(0x10);
        let ctx_ptr2 = ctx_ptr.map_addr(|x| x | 0xFFFF_FFFF_FE00_0000_usize);
        unsafe {
            ctx_ptr2.write_volatile(UserContext {
                exception_vector: AtomicUsize::new(0x1000),
                exception_stack: AtomicPtr::new(0x20 as *mut _),
            });
        }

        let tcr;
        unsafe {
            asm! {
                "mrs {}, TCR_EL1",
                out(reg) tcr
            };
        }

        let init = Arc::new(Execution::new(tcr, 0x0, ctx_ptr));
        init.add_writable_page(page);

        let num_cores = device_tree.root().cpus().iter().count();
        while usize::from(NUM_READY.load(Ordering::Relaxed)) != num_cores {
            hint::spin_loop();
        }

        init.jump_into(ExceptionCode::Resumption, &[])
    } else {
        while !GLOBAL_SETUP_DONE.load(Ordering::Acquire) {
            hint::spin_loop();
        }
        execution::idle_loop()
    }
}

/// Panics are unhandled error conditions - the entire system may be forced to shut down
#[panic_handler]
#[expect(
    unused_must_use,
    reason = "Ignoring any failure conditions as a panic is already a failure condition"
)]
fn panic(info: &PanicInfo) -> ! {
    // Make sure that this doesn't overlap with other peripheral accesses
    if let Some(uart) = UART.get() {
        let mut uart = uart.lock();
        write!(&mut uart, "PANIC occurred");
        if let Some(location) = info.location() {
            write!(
                &mut uart,
                " (at {}:{}:{})",
                location.file(),
                location.line(),
                location.column()
            );
        }
        if let Some(args) = info.message() {
            write!(&mut uart, ": ");
            uart.write_fmt(*args);
        }
        writeln!(&mut uart);
        drop(uart);
    }
    loop {
        hint::spin_loop();
    }
}

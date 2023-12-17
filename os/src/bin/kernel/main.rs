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
#![expect(clippy::todo)]
#![expect(clippy::panic)]
#![feature(allocator_api)]
#![feature(alloc_layout_extra)]
#![feature(asm_const)]
#![feature(btreemap_alloc)]
#![feature(const_mut_refs)]
#![feature(const_option)]
#![feature(const_ptr_as_ref)]
#![feature(ascii_char)]
#![feature(exposed_provenance)]
#![feature(generic_arg_infer)]
#![feature(lint_reasons)]
#![feature(maybe_uninit_array_assume_init)]
#![feature(naked_functions)]
#![feature(panic_info_message)]
#![feature(pointer_is_aligned)]
#![feature(slice_ptr_get)]
#![feature(slice_take)]
#![feature(stdsimd)]
#![feature(stmt_expr_attributes)]
#![feature(iterator_try_collect)]
#![feature(let_chains)]
#![feature(anonymous_lifetime_in_impl_trait)]
#![feature(strict_provenance)]

use bump_allocator::BumpAllocator;
use core::fmt::Write;
use core::num::NonZeroUsize;
use core::panic::PanicInfo;
use core::ptr::{addr_of_mut, NonNull};
use core::sync::atomic::{AtomicBool, Ordering};
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
    if machine::core_id() == 0 {
        let device_tree_physical;
        {
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

            println!("Made it to DTB");

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
            println!("Made itadsfasfsa to DTB");

            // .expect("Device tree address should be nonnull and correctly in virtual memory");
            // let info = machine::get_info(device_tree_address);
            let device_tree =
                DeviceTree::from_bytes(device_tree_memory).expect("Device tree should be valid");
            println!("Masdfasfahdslfjakhsdfjklahsljkdfhsajkhfajshflkde it to DTB");

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
                    &[(0x8_0000, 0x18_0000), (0, 0x1_0000)].iter().copied(),
                );
            }

            GLOBAL_SETUP_DONE.store(true, Ordering::Release);

            device_tree_physical = machine::to_physical_addr(device_tree_memory.as_ptr().addr())
                .expect("Device tree should be at a valid virtual address");
        }

        // SAFETY: This correctly sets up a non-returning jump into usermode
        unsafe {
            core::arch::asm! {
                "msr TTBR0_EL1, x0",
                "msr SPSR_EL1, {SPSR_EL1}",
                "msr ELR_EL1, {ELR_EL1}",
                "eret",
                in("x0") INIT_TRANSLATION_ADDRESS,
                SPSR_EL1 = in(reg) 0b0_u64,
                ELR_EL1 = in(reg) 0x1000_usize,
                in("x1") device_tree_physical,
                options(noreturn, nostack)
            }
        }
    } else {
        while !GLOBAL_SETUP_DONE.load(Ordering::Acquire) {
            hint::spin_loop();
        }
    }

    loop {
        hint::spin_loop();
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

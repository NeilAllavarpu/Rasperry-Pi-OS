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
#![feature(ptr_from_ref)]
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
use core::hint;
use core::num::NonZeroUsize;
use core::panic::PanicInfo;
use core::ptr::{addr_of_mut, NonNull};
use core::sync::atomic::{AtomicBool, Ordering};
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

#[global_allocator]
static mut GLOB: BumpAllocator = BumpAllocator::empty();

/// The primary initialization sequence for the kernel in EL1
extern "C" fn main(device_tree_address: Option<NonNull<u64>>) -> ! {
    /// Set only when the global initialization sequence (stuff that only runs once total) is
    /// finished
    static GLOBAL_SETUP_DONE: AtomicBool = AtomicBool::new(false);
    if machine::core_id() == 0 {
        extern "C" {
            static mut __bss_end: u8;
        }
        exception::init();

        // Create a virtual mapping so that we can access the UART
        #[expect(
            clippy::unwrap_used,
            reason = "No reasonable way to recover from failure here"
        )]
        let mut uart =
        // SAFETY: This points to a valid, permanent UART register map in memory. No other
        // code accesses this concurrently
            unsafe { Uart::new(NonZeroUsize::new(0xFFFF_FFFF_FE20_1000).unwrap()) }.unwrap();

        writeln!(&mut uart, "What just happened? Why am I here?").unwrap();
        assert!(
            matches!(UART.set(SpinLock::new(uart)), Ok(())),
            "UART should not already be initialized"
        );

        let stack_end = NonNull::new(unsafe { addr_of_mut!(__bss_end) })
            .unwrap()
            .map_addr(|bss_end| {
                NonZeroUsize::new(bss_end.get().next_multiple_of(16) + STACK_SIZE * 4).unwrap()
            });

        unsafe {
            GLOB.set(
                stack_end,
                stack_end.with_addr(NonZeroUsize::new(0xFFFF_FFFF_FE20_0000).unwrap()),
            );
        }

        machine::get_info(
            device_tree_address
                .expect("Device tree address should be nonnull and correctly in virtual memory"),
        );

        unsafe {
            memory::init(
                machine::INFO.get().unwrap().memory.iter(),
                &[(0x8_0000, 0x18_0000), (0, 0x1_0000)].iter(),
            );
        }

        GLOBAL_SETUP_DONE.store(true, Ordering::Release);

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

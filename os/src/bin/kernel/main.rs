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
#![expect(clippy::todo)]
#![expect(clippy::panic)]
#![feature(asm_const)]
#![feature(const_mut_refs)]
#![feature(generic_arg_infer)]
#![feature(lint_reasons)]
#![feature(maybe_uninit_array_assume_init)]
#![feature(naked_functions)]
#![feature(strict_provenance)]
#![feature(panic_info_message)]
#![feature(pointer_is_aligned)]
#![feature(stmt_expr_attributes)]

use core::fmt::Write;
use core::hint;
use core::num::NonZeroUsize;
use core::panic::PanicInfo;
use core::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use stdos::cell::InitCell;
use stdos::sync::SpinLock;

mod boot;
mod exception;
mod uart;
mod vm;
use uart::Uart;

/// Physical address of the init program's top-level translation table
const INIT_TRANSLATION_ADDRESS: u64 = 0x0;

/// The global UART for all prints
static UART: InitCell<SpinLock<Uart>> = InitCell::new();

#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        writeln!(&mut $crate::UART.lock(), $($arg)*).unwrap();
    }};
}

/// The primary initialization sequence for the kernel in EL1
extern "C" fn main() -> ! {
    /// Set only when the global initialization sequence (stuff that only runs once total) is
    /// finished
    static GLOBAL_SETUP_DONE: AtomicBool = AtomicBool::new(false);
    /// Set once all cores have reached this point
    static CORES_BOOTED: AtomicU8 = AtomicU8::new(0);

    let ticket = CORES_BOOTED.fetch_add(1, Ordering::Relaxed);
    if ticket == 0 {
        exception::init();
        // Create a virtual mapping so that we can access the UART
        assert!(
            vm::ADDRESS_SPACE
                .lock()
                .map(0xFFFF_FFFF_FFFF_0000, 0xFE20_0000, true, true),
            "Mapping the UART should not fail"
        );
        #[expect(
            clippy::unwrap_used,
            reason = "No reasonable way to recover from failure here"
        )]
        let mut uart =
        // SAFETY: This points to a valid, permanent UART register map in memory. No other
        // code accesses this concurrently
            unsafe { Uart::new(NonZeroUsize::new(0xFFFF_FFFF_FFFF_1000).unwrap()) }.unwrap();
        writeln!(&mut uart, "What just happened? Why am I here?").unwrap();
        // SAFETY: This is the boot sequence and no one else is accessing the UART yet
        unsafe {
            UART.set(SpinLock::new(uart));
        }

        GLOBAL_SETUP_DONE.store(true, Ordering::Release);

        while CORES_BOOTED.load(Ordering::SeqCst) != 4 {
            hint::spin_loop();
        }

        // SAFETY: This correctly sets up a non-returning jump into usermode
        unsafe {
            core::arch::asm! {
                "ldr x0, ={TTBR0_EL1}",
                "msr TTBR0_EL1, x0",
                "ldr x0, =0x1000",
                "msr ELR_EL1, x0",
                "mov x0, 0b1111000000",
                "msr SPSR_EL1, x0",
                "eret",
                TTBR0_EL1 = const INIT_TRANSLATION_ADDRESS,
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

    // TODO: Last one here, deallocate the null page
}

/// Panics are unhandled error conditions - the entire system may be forced to shut down
#[panic_handler]
#[expect(
    unused_must_use,
    reason = "Ignoring any failure conditions as a panic is already a failure condition"
)]
fn panic(info: &PanicInfo) -> ! {
    let mut uart = UART.lock();
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
    loop {
        hint::spin_loop();
    }
}

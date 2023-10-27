//! crate
#![no_main]
#![no_std]
#![warn(clippy::all)]
#![warn(clippy::restriction)]
#![warn(clippy::complexity)]
#![deny(clippy::correctness)]
#![warn(clippy::nursery)]
#![warn(clippy::pedantic)]
#![deny(clippy::perf)]
#![warn(clippy::style)]
#![deny(clippy::suspicious)]
#![deny(unsafe_op_in_unsafe_fn)]
#![expect(clippy::blanket_clippy_restriction_lints, reason = "Paranoid linting")]
#![expect(
    clippy::inline_asm_x86_intel_syntax,
    reason = "Not relevant for target architecture"
)]
#![expect(clippy::implicit_return, reason = "Desired format")]
#![expect(clippy::question_mark_used, reason = "Desired format")]
#![feature(lint_reasons)]
#![feature(maybe_uninit_array_assume_init)]
#![feature(naked_functions)]
#![feature(pointer_is_aligned)]
#![feature(ptr_as_uninit)]
#![feature(stdsimd)]
#![feature(strict_provenance)]
#![expect(
    clippy::allow_attributes,
    reason = "Cannot disable for only a macro's output"
)]
#![expect(
    clippy::allow_attributes_without_reason,
    reason = "Cannot disable for only a macro's output"
)]
#![expect(
    clippy::default_numeric_fallback,
    reason = "Cannot disable for only a macro's output"
)]
#![expect(clippy::integer_division, reason = "Intentional")]
#![expect(clippy::shadow_reuse, reason = "Desired format")]
#![expect(clippy::separated_literal_suffix, reason = "Desired format")]
#![expect(clippy::single_call_fn, reason = "Intentional")]
#![expect(clippy::little_endian_bytes, reason = "Intentional")]
#![feature(stmt_expr_attributes)]

mod gpio;
mod uart;

use core::arch;
use core::arch::aarch64;
use core::hint;
use core::mem::MaybeUninit;
use core::num::NonZeroUsize;
use core::panic::PanicInfo;
use core::ptr;
use core::ptr::NonNull;
use gpio::FunctionSelect;
use gpio::Gpio;
use gpio::Pull;
use uart::IoError;
use uart::Uart;

/// Byte to indicate to the server of a request
const SERVER_REQUEST: u8 = b'\x1B';

/// The boot sequence for the bootloader
/// * Moves the code segment of the bootloader out of the way to make room for the loaded kernel
/// * Prepares Rust execution
#[no_mangle]
#[naked]
#[link_section = ".init"]
extern "C" fn _start() -> ! {
    // SAFETY: This is boot code to set up a stack, reposition the bootloader, and hand control
    // off to Rust
    unsafe {
        arch::asm!(
            // Set up a stack pointer, save all state that we may affect, and disable interrupts
            "mov x30, 0x3F50",
            "mov sp, x30",
            "stp x0, x1, [sp]",
            "mrs x0, DAIF",
            "msr DAIFSet, 0b1111",
            "mrs x1, NZCV",
            "orr x0, x0, x1",
            "stp x2, x3, [sp, 0x10]",
            "stp x4, x5, [sp, 0x20]",
            "stp x6, x7, [sp, 0x30]",
            "stp x8, x9, [sp, 0x40]",
            "stp x10, x11, [sp, 0x50]",
            "stp x12, x13, [sp, 0x60]",
            "stp x14, x15, [sp, 0x70]",
            "stp x16, x17, [sp, 0x80]",
            "stp x18, x0, [sp, 0x90]",
            "stp fp, lr, [sp, 0xA0]",

            "mov x5, :abs_g0:__text_start", // Copy this code section to the lower address
            "sub x5, x5, 16",
            "mov x1, :abs_g0:__data_end",

            "mov x0, x4", // x4 should contain the branched-to address, i.e. _start
                          // we want to load the new kernel to this exact same spot
            "0:",
            "ldp x2, x3, [x4], #16",
            "stp x2, x3, [x5, 16]!",
            "dc cvau, x5",
            "cmp x1, x5",
            "b.hi 0b",
            "stp xzr, xzr, [x0]",

            "dmb ishst", // Make sure the copying finishes, and then invalidate the instruction cache as necessary
            "ic ialluis",
            "dsb ish",
            "isb",

            "mov x1, :abs_g0:{}",
            "mov lr, :abs_g0:0f", // Manually set the LR since the return physical address is different from where we are currently executing
            "br x1", // Jump to main
            "0:",

            "dmb ishst", // Make sure all the updates are fully complete
            "ic ialluis",
            "dsb ish",
            "isb sy",

            // Restore state and jump to the new kernel
            "ldp x2, x3, [sp, 0x10]",
            "ldp x4, x5, [sp, 0x20]",
            "ldp x6, x7, [sp, 0x30]",
            "ldp x8, x9, [sp, 0x40]",
            "ldp x10, x11, [sp, 0x50]",
            "ldp x12, x13, [sp, 0x60]",
            "ldp x14, x15, [sp, 0x70]",
            "ldp x16, x17, [sp, 0x80]",
            "ldp x18, x0, [sp, 0x90]",
            "ldp fp, lr, [sp, 0xA0]",

            "msr DAIF, x0",
            "msr NZCV, x0",

            "ldp x0, x1, [sp]",

            "br x4",
            sym main,
            options(noreturn))
    }
}

extern "C" fn main(load_addr: usize) {
    // We require both a write barrier before the first write to a new peripheral and a read
    // barrier after the last read from the old peripheral

    // SAFETY: This is properly defined on the target where it runs, the Raspberry Pi in 64-bit mode
    unsafe { aarch64::__dmb(aarch64::OSHST) }

    #[expect(
        clippy::unwrap_used,
        reason = "This pointer is aligned and nonnull, so this should never fail"
    )]
    // SAFETY: This points to a valid, permanent GPIO register map in physical memory. No other
    // code accesses this while this bootloader is running
    let mut gpio = unsafe { Gpio::new(NonZeroUsize::new(0x4_7E20_0000).unwrap()) }.unwrap();
    // Select pins 14 and 15 as appropriate TX/RX pins
    gpio.select_function(14, FunctionSelect::Alt0);
    gpio.select_function(15, FunctionSelect::Alt0);
    gpio.select_pull(14, Pull::Up);
    gpio.select_pull(15, Pull::Up);

    // SAFETY: This is properly defined on the target where it runs, the Raspberry Pi in 64-bit mode
    unsafe { aarch64::__dmb(aarch64::OSH) }

    #[expect(
        clippy::unwrap_used,
        reason = "The pointer is aligned and nonnull, so this should never fail"
    )]
    // SAFETY: This points to a valid, permanent UART register map in physical memory. No other
    // code accesses this while this bootloader is running
    let mut uart = unsafe { Uart::new(NonZeroUsize::new(0x4_7E20_1000).unwrap()) }.unwrap();

    // Ignore any residual reads that may be left
    uart.clear_reads();

    loop {
        match try_load_kernel(&mut uart, load_addr) {
            Ok(addr) => {
                // On success, notify the server with a 0 byte.
                #[expect(clippy::expect_used, reason = "No better failure modes decided yet")]
                uart.write_byte(0)
                    .expect("Unrecoverable error: unable to transmit via UART");
                break addr;
            }
            Err(_) => {
                #[expect(
                    clippy::expect_used,
                    reason = "If writing this byte also fails, then we have no choice but to panic"
                )]
                // Any failures should be reported by transmitting a nonzero byte
                uart.write_byte(0xFF)
                    .expect("Unrecoverable error: unable to transmit via UART");
            }
        }
    }
}

/// Attempts to load a kernel according to the agreed-upon protocol.
///
/// Returns an `Ok` containing the loaded kernel address if successful
/// Returns an `Error` if an IO error occurs.
fn try_load_kernel(uart: &mut Uart, address: usize) -> Result<(), IoError> {
    // Write an escape character to begin the loading process, and ask for a kernel
    uart.write_byte(SERVER_REQUEST)?;
    // Ask for a kernel
    uart.write_byte(0)?;
    // Read the size of the kernel
    #[expect(
        clippy::as_conversions,
        reason = "No other way to const-convert a `u32` to a `usize`"
    )]
    let mut kernel_size = [MaybeUninit::uninit(); (u32::BITS / 8) as usize];
    uart.read_bytes(&mut kernel_size)?;
    // SAFETY: The call to `read_bytes` promises to initialize the entire array
    let kernel_size = unsafe { MaybeUninit::array_assume_init(kernel_size) };
    let kernel_size = u32::from_le_bytes(kernel_size);
    // TODO: Decide upon an address based on server input
    let Some(kernel_addr) = NonNull::new(ptr::from_exposed_addr_mut(address)) else {
        uart.write_byte(1)?;
        return Err(IoError::Frame);
    };

    // SAFETY: The region of memory for the kernel is valid and unused by everything else, and the
    // size of the kernel fits into a `u32` which fits into an `isize`
    let kernel = unsafe {
        #[expect(clippy::unwrap_used, reason = "This conversion can never fail")]
        NonNull::slice_from_raw_parts(kernel_addr, kernel_size.try_into().unwrap())
            .as_uninit_slice_mut()
    };
    uart.read_bytes(kernel)
}

/// Panic handler: nothing to do but park the core, since the UART is nonfunctional in this case
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop {
        hint::spin_loop();
    }
}

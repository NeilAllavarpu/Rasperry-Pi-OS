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
#![feature(const_option)]
#![feature(maybe_uninit_array_assume_init)]
#![feature(naked_functions)]
#![feature(pointer_is_aligned)]
#![feature(ptr_as_uninit)]
#![feature(stdsimd)]
#![feature(strict_provenance)]
#![expect(clippy::allow_attributes, reason = "Cannot disable for only a macro's output")]
#![expect(clippy::allow_attributes_without_reason, reason = "Cannot disable for only a macro's output")]
#![expect(clippy::default_numeric_fallback, reason = "Cannot disable for only a macro's output")]
#![expect(clippy::integer_division, reason = "Intentional")]
#![expect(clippy::shadow_reuse, reason = "Desired format")]
#![expect(clippy::separated_literal_suffix, reason = "Desired format")]
#![feature(stmt_expr_attributes)]

mod gpio;
mod uart;

use core::{
    arch::{aarch64, asm},
    mem::MaybeUninit,
    num::NonZeroUsize,
    panic::PanicInfo,
    ptr::{self, addr_of, NonNull},
};
use gpio::{FunctionSelect, Gpio, Pull};
use uart::{IoError, Uart};


/// The boot sequence for the bootloader
/// * Parks non-primary cores until bootloading is complete
/// * Moves the code segment of the bootloader out of the way to make room for the laoded kernel
/// * Prepares Rust execution
#[no_mangle]
#[naked]
#[link_section = ".init"]
extern "C" fn _start() -> ! {
    // SAFETY: This is boot code to set up a stack, reposition the bootloader, and hand control
    // off to Rust
    unsafe {
        asm!(
            // x4 should contain the branched-to address, i.e. _start
            "msr DAIFset, 0b1111", // Disable interrupts until ready
            "mov w5, :abs_g0:__text_start", // Copy this code section to the lower address
            "mov w6, :abs_g0:__data_end",
            "mov sp, x5",
            "0: ldp x7, x8, [x4], #16",
            "stp x7, x8, [x5], #16",
            "cmp w6, w5",
            "dc cvac, x5", // Clean data caches and invalidate instruction caches to ensure
                           // coherency
            "b.hi 0b",
            "dsb ish",
            "ic ialluis",
            "isb sy",
            "mov w9, :abs_g0:__bss_start", // Zero BSS
            "mov w10, :abs_g0:__bss_end",
            "mov w11, :abs_g0:{}", // Jump to main
            "0: strb wzr, [x9], #1",
            "cmp w10, w9",
            "b.hi 0b",
            "br x11",
            sym main,  
            options(noreturn))
    }
}

extern "C" fn main(x0: usize, x1: usize, x2: usize, x3: usize) -> ! {
    // We require a write barrier before the first write to a new peripheral
    // SAFETY: This should only run on `aarch64` targets
    unsafe { aarch64::__dmb(aarch64::OSHST) }
    #[expect(clippy::unwrap_used,
        reason = "This pointer is aligned and nonnull, so this should never fail")]
    // SAFETY: This points to a valid, permanent GPIO register map in physical memory. No other
    // code accesses this while this bootloader is running
    let mut gpio = unsafe { Gpio::new(NonZeroUsize::new(0xFE20_0000).unwrap()) }.unwrap();
    // Select pins 14 and 15 as appropriate TX/RX pins
    gpio.select_function(14, FunctionSelect::Alt0);
    gpio.select_function(15, FunctionSelect::Alt0);
    gpio.select_pull(14, Pull::Up);
    gpio.select_pull(15, Pull::Up);
    // We require both a write barrier befoer the first write to a new peripheral and a read
    // barrier after the last read from the old peripheral
    // SAFETY: This should only run on `aarch64` targets
    unsafe { aarch64::__dmb(aarch64::OSH) }
    #[expect(clippy::unwrap_used,
        reason = "The pointer is aligned and nonnull, so this should never fail")]

    // SAFETY: This points to a valid, permanent UART register map in physical memory. No other
    // code accesses this while this bootloader is running
    let mut uart = unsafe { Uart::new(NonZeroUsize::new(0xFE20_1000).unwrap()) }.unwrap();
    // Ignore any residual reads that may be left
    uart.clear_reads();

    let kernel_addr = loop {
        match try_load_kernel(&mut uart) {
            Ok(addr) => {
                // On success, notify the server with a 0 byte.
                #[expect(clippy::expect_used, reason = "No better failure modes decided yet")]
                uart.write_byte(0).expect("Unrecoverable error: unable to transmit via UART");
                break addr;
            }
            Err(_) => {
                #[expect(clippy::expect_used, reason = "If writing this byte also fails, then we have no choice but to panic")]
                // Any failures should be reported by transmitting a nonzero byte
                uart.write_byte(0xFF)
                    .expect("Unrecoverable error: unable to transmit via UART");
            }
        }
    };

    // SAFETY: This does not return because of the `br`
    unsafe {
        asm!("ic ialluis",
             "isb", 
             "br x4",
             in("x0") x0,
             in("x1") x1, 
             in("x2") x2,
             in("x3") x3,
             in("x4") kernel_addr.as_ptr(),
             options(noreturn));
    }
}

/// Attempts to load a kernel according to the agreed-upon protocol.
///
/// Returns an `Ok` containing the loaded kernel address if successful
/// Returns an `Error` if an IO error occurs.
fn try_load_kernel(uart: &mut Uart) -> Result<NonNull<()>, IoError> {
    // Write an escape character to begin the loading process, and ask for a kernel
    uart.write_byte(b'\x1B')?;
    // Ask for a kernel
    uart.write_byte(0)?;
    // Read the size of the kernel
    #[expect(clippy::as_conversions, reason = "No other way to const-convert a `u32` to a `usize`")]
    let mut kernel_size = [MaybeUninit::uninit(); (u32::BITS / 8) as usize];
    uart.read_bytes(&mut kernel_size)?;
    // SAFETY: The call to `read_bytes` promises to initialize the entire array
    let kernel_size = unsafe { MaybeUninit::array_assume_init(kernel_size) };
    let kernel_size = u32::from_le_bytes(kernel_size);
    // TODO: Decide upon an address based on server input
    #[expect(clippy::unwrap_used, reason = "This conversion can never fail")]
    let kernel_addr = NonNull::new(ptr::from_exposed_addr_mut(0x8_0000)).unwrap();
    // SAFETY: The region of memory for the kernel is valid and unused by everything else, and the
    // size of the kernel fits into a `u32` which fits into an `isize`
    let kernel = unsafe {
        #[expect(clippy::unwrap_used, reason = "This conversion can never fail")]
        NonNull::slice_from_raw_parts(kernel_addr, kernel_size.try_into().unwrap())
            .as_uninit_slice_mut()
    };
    uart.read_bytes(kernel)?;
    // We can step by 16 by assumption that cache lines are at least 16 bytes long
    for byte in kernel.iter_mut().step_by(16) {
        // SAFETY: This does nothing but clean data caches - nothing is actually affected
        unsafe {
            asm!("dc cvac, {}", in (reg) addr_of!(byte), options(nomem, nostack, preserves_flags));
        };
    }
    Ok(kernel_addr.cast())
}

/// Panic handler: nothing to do but park the core, since the UART is nonfunctional in this case
#[panic_handler]
fn panic(_: &PanicInfo) -> ! {
    loop {
        // SAFETY: This should only run on `aarch64` targets
        unsafe {
            aarch64::__wfi();
        }
    }
}

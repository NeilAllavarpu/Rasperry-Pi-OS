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
#![feature(ptr_from_ref)]
#![feature(const_trait_impl)]
#![feature(ptr_mask)]

mod gpio;
mod uart;
mod mailbox;
mod dma;

use core::num::NonZeroU32;
use core::arch::aarch64::{OSH, OSHST, SY};
use mailbox::{Mailbox, Clock};
use core::{
    arch::{aarch64, asm},
    mem::MaybeUninit,
    num::NonZeroUsize,
    panic::PanicInfo,
    ptr::{self, NonNull},
};
use gpio::{FunctionSelect, Gpio, Pull};
use uart::{IoError, Uart};

/// Byte to indicate to the server of a request
const SERVER_REQUEST: u8 = b'\x1B';

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
            "str xzr, [x4]",
            "mrs x29, SCTLR_EL2", // Disable caching
            "mov x28, #0x103",
            "bic x27, x29, x28",
            "msr SCTLR_EL2, x29",
            "mov w5, :abs_g0:__text_start", // Copy this code section to the lower address
            "mov w6, :abs_g0:__data_end",
            "mov sp, x5",
            "0: ldp x7, x8, [x4], #16",
            "stp x7, x8, [x5], #16",
            "cmp w6, w5",
            "b.hi 0b",
            "dsb ish",
            "isb sy",
            "mov w9, :abs_g0:__bss_start", // Zero BSS
            "mov w10, :abs_g0:__bss_end",
            "mov w11, :abs_g0:{}", 
            "0: strb wzr, [x9], #1",
            "cmp w10, w9",
            "b.hi 0b",
            "br x11", // Jump to main
            sym main,  
            options(noreturn))
    }
}

extern "C" fn main(x0: usize, x1: usize, x2: usize, x3: usize) -> ! {
    // We require a write barrier before the first write to a new peripheral
    // SAFETY: This should only run on `aarch64` targets
    unsafe { aarch64::__dmb(OSHST) }
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

    // SAFETY: See above invocation
    // unsafe { aarch64::__dmb(OSH) }

    // let mut mailbox = unsafe { Mailbox::new(NonZeroUsize::new(0xFE00_0000 + 0xB880).unwrap()).unwrap()};

    // let max_clock_rate = mailbox.get_max_clock_rate(Clock::Uart);
    // We require both a write barrier befoer the first write to a new peripheral and a read
    // barrier after the last read from the old peripheral
    // SAFETY: See above invocation
    unsafe { aarch64::__dmb(OSH) }

    #[expect(clippy::unwrap_used,
        reason = "The pointer is aligned and nonnull, so this should never fail")]
    // SAFETY: This points to a valid, permanent UART register map in physical memory. No other
    // code accesses this while this bootloader is running
    let mut uart = unsafe { Uart::new(NonZeroUsize::new(0xFE20_1000).unwrap()) }.unwrap();
    // Ignore any residual reads that may be left
    uart.clear_reads();
    
    // if let Some(max_clock_rate) = max_clock_rate {
    //     let _: Result<u32, IoError> = try_upgrade_baud(&mut uart, &mut mailbox, max_clock_rate.get() / 16);
    //     unsafe { aarch64::__dmb(OSH) }
    // }

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

    // SAFETY: See above invocations
    unsafe { aarch64::__dsb(OSHST) }
    // SAFETY: See above invocations
    unsafe { aarch64::__isb(SY) }
    // SAFETY: This does not return because of the `br`
    unsafe {
        asm!(
             "br x4",
             in("x0") x0,
             in("x1") x1, 
             in("x2") x2,
             in("x3") x3,
             in("x4") kernel_addr.as_ptr(),
             options(noreturn));
    }
}

// TODO: changing baud rate is buggy, via either divider or clock
/// Attempts to upgrade the baud rate to a higher speed.
///
/// Returns an `Ok` containing the new baud rate, or an `Error` if an IO error or logic error
/// occurs
#[expect(dead_code, reason="dead")]
fn try_upgrade_baud(uart: &mut Uart, mailbox: &mut Mailbox, max_rate: u32) -> Result<u32, IoError> {
    // Ask to upgrade the baud rate
    uart.write_byte(SERVER_REQUEST)?;
    uart.write_byte(1)?;
    // Send our max supported baud rate
    uart.write_u32(max_rate)?;
    // Get the final baud rate
    let Some(clock_rate) = NonZeroU32::new(uart.read_u32()?)
        .and_then(|rate| rate.checked_mul(
            #[expect(clippy::unwrap_used, reason="This conversion never fails")]
            NonZeroU32::new(16).unwrap()
        )) else {
            // If parsing fails, notify the server and abort the rate change
            uart.write_byte(1)?;
            return Err(IoError::Parity) // TODO: Make a custom error types
        };


    // Acknowledge transmission
    uart.write_byte(0)?;

    // Set the UART divider to 1; the clock will control the baud rate for us
    uart.set_divider(3, 0);

    // SAFETY: See above invocation
    unsafe { aarch64::__dmb(OSH) }

    // Set the clock rate to 16x the baud rate (it is always divided by 16 for baud rate)
    // 
    mailbox.set_clock_rate(Clock::Uart, clock_rate);

    Ok(clock_rate.get() / 16)
}

/// Attempts to load a kernel according to the agreed-upon protocol.
///
/// Returns an `Ok` containing the loaded kernel address if successful
/// Returns an `Error` if an IO error occurs.
fn try_load_kernel(uart: &mut Uart) -> Result<NonNull<()>, IoError> {
    // Write an escape character to begin the loading process, and ask for a kernel
    uart.write_byte(SERVER_REQUEST)?;
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

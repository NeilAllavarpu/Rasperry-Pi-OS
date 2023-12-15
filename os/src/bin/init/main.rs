#![no_main]
#![no_std]
#![feature(naked_functions)]
#![feature(asm_const)]
use core::alloc::GlobalAlloc;

use stdos::os::syscalls;

const INIT_TABLE_ENTRY_BASE: u64 = (1 << 53) // Privileged execute-never
| (1    << 11) // Non-global entry
| (1    << 10) // Access flag
| (0b11 << 8)  // Shareability
| (1    << 6)  // EL0 accessible
|  0b11; // Valid entry

#[no_mangle]
#[link_section = ".init"]
#[naked]
extern "C" fn _start() -> ! {
    unsafe {
        core::arch::asm! {
            // Allocate a new page for the next stage ELF
            "svc 0x3000", // If the allocation fails, we're in trouble anyways, so don't bother error checking
            "ldr x0, ={TABLE_ENTRY_BASE}",
            "orr x1, x1, x0",
            "mov x0, 8",
            "str x1, [x0]",
            "dmb ish",

            // Read the (possibly misaligned) size of the next stage ELF, into x1
            // x2 contains the start of the actual ELF
            "adr x2, __elf_start",
            "ldrb w0, [x2], 1",
            "ldrb w1, [x2], 1",
            "orr x1, x0, x1, LSL 8",

            // Copy the ELF to its new page
            "mov x0, 0x10000",
            "mov x3, x1",
            "0: subs x3, x3, 1",
            "ldrb w4, [x2, x3]",
            "strb w4, [x0, x3]",
            "b.pl 0b",

            // Zero BSS
            "adr x2, __bss_start",
            "adr x3, __bss_end",
            "0: strb wzr, [x2], 1",
            "cmp x2, x3",
            "b.ls 0b",

            "mov sp, 0x10000",
            "b {main}",
            main = sym main,
            TABLE_ENTRY_BASE = const INIT_TABLE_ENTRY_BASE,
            options(noreturn)
        }
    }
}

struct NoUse {}

unsafe impl GlobalAlloc for NoUse {
    unsafe fn alloc(&self, layout: core::alloc::Layout) -> *mut u8 {
        unreachable!()
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: core::alloc::Layout) {
        unreachable!()
    }
}

#[global_allocator]
static D: NoUse = NoUse {};

/// The entry point of the init program. Spawns all the other programs before exiting
/// # Safety
/// `next_part` and `next_len` must describe a valid, accessible ELF in memory, including padding bytes to the nearest `u64` boundary.
/// # Panics
/// Panics if `next_part` is not page aligned
unsafe extern "C" fn main(next_part: *mut usize, next_len: u16) -> ! {
    syscalls::write("Hello from usermode!\n".as_bytes());

    // SAFETY: The caller promises that the arguments refer to a valid ELF, padding included
    let elf = unsafe {
        core::slice::from_raw_parts(next_part, usize::try_from(next_len).unwrap().div_ceil(8))
    };
    // alloc a new PD
    // elf load into it
    // fork+exec into it
    // - cow fork
    // - replace PD with new one
    loop {
        core::hint::spin_loop();
    }
    syscalls::write("Unreachable!\n".as_bytes());
    syscalls::exit()
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    syscalls::write("Panic!\n".as_bytes());
    syscalls::exit()
}

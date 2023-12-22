#![no_main]
#![no_std]
#![feature(naked_functions)]
#![feature(asm_const)]
#![feature(inline_const)]
#![feature(generic_arg_infer)]
#![feature(strict_provenance_atomic_ptr)]

use core::{
    alloc::GlobalAlloc,
    fmt::Write,
    mem,
    sync::atomic::{AtomicPtr, AtomicU64},
};

use stdos::os::syscalls;

const INIT_TABLE_ENTRY_BASE: u64 = (1 << 53) // Privileged execute-never
| (1    << 11) // Non-global entry
| (1    << 10) // Access flag
| (0b11 << 8)  // Shareability
| (1    << 6)  // EL0 accessible
|  0b11; // Valid entry

struct UserContext {
    exception_vector: extern "C" fn() -> !,
    exception_stack: AtomicPtr<u64>,
    exception_stack_mem: &'static [AtomicU64],
}

static CONTEXT: UserContext = UserContext {
    exception_vector: exception_handler,
    exception_stack: AtomicPtr::new(EXCEPTION_STACK[0].as_ptr()),
    exception_stack_mem: &EXCEPTION_STACK,
};

static EXCEPTION_STACK: [AtomicU64; 8] = [const { AtomicU64::new(0) }; _];

#[no_mangle]
#[link_section = ".init"]
#[naked]
extern "C" fn _start() {
    unsafe {
        core::arch::asm! {
            // Set the exception context
            "adr x0, {user_context}", // User contet
            "mov x1, xzr", // TTBR0_EL1, guaranteed to be 0
            "mov x2, {TCR_EL1}", // TODO: actually make TCR_EL1 meaningful
            "svc 0x4000",
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
            user_context = sym CONTEXT,
            TCR_EL1 = const 0, // TODO
            main = sym main,
            TABLE_ENTRY_BASE = const INIT_TABLE_ENTRY_BASE,
            options(noreturn)
        }
    }
}

struct NoUse {}

unsafe impl GlobalAlloc for NoUse {
    unsafe fn alloc(&self, _: core::alloc::Layout) -> *mut u8 {
        unreachable!()
    }

    unsafe fn dealloc(&self, _: *mut u8, _: core::alloc::Layout) {
        unreachable!()
    }
}

#[global_allocator]
static D: NoUse = NoUse {};

struct Stdout {}
impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        syscalls::write(s.as_bytes());
        Ok(())
    }
}

/// The entry point of the init program. Spawns all the other programs before exiting
/// # Safety
/// `next_part` and `next_len` must describe a valid, accessible ELF in memory, including padding bytes to the nearest `u64` boundary.
/// # Panics
/// Panics if `next_part` is not page aligned
unsafe extern "C" fn main(next_part: *mut usize, next_len: u16) -> ! {
    syscalls::write("Hello from usermode!\n".as_bytes());

    // SAFETY: The caller promises that the arguments refer to a valid ELF, padding included
    let elf = unsafe {
        core::slice::from_raw_parts(
            next_part,
            usize::from(next_len).div_ceil(mem::size_of::<usize>()),
        )
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

extern "C" fn exception_handler() -> ! {
    let mut stdout = Stdout {};
    writeln!(&mut stdout, "code: {}", unsafe {
        *CONTEXT
            .exception_stack
            .load(core::sync::atomic::Ordering::Relaxed)
            .sub(1)
    });
    todo!("Enforce returning")
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    syscalls::write("Panic!\n".as_bytes());
    syscalls::exit()
}

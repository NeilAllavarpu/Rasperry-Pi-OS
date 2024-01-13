#![no_main]
#![no_std]
#![feature(naked_functions)]
#![feature(asm_const)]
#![feature(inline_const)]
#![feature(generic_arg_infer)]
#![feature(strict_provenance_atomic_ptr)]
#![feature(panic_info_message)]
#![feature(strict_provenance)]
#![feature(let_chains)]
#![feature(const_mut_refs)]
#![feature(never_type)]

use common::os::vm::{self, AddressSpace, ADDRESS_SPACE};
use common::println;
use common::sync::SpinLock;
mod exception;
mod syscalls;
use core::sync::atomic::Ordering;
use core::{
    alloc::GlobalAlloc,
    arch::asm,
    fmt::Write,
    hint, mem,
    panic::PanicInfo,
    ptr::NonNull,
    sync::atomic::{AtomicPtr, AtomicU64},
};
use exception::CONTEXT;

use crate::syscalls::getpid;

const INIT_TABLE_ENTRY_BASE: u64 = (1 << 53) // Privileged execute-never
| (1    << 11) // Non-global entry
| (1    << 10) // Access flag
| (0b11 << 8)  // Shareability
| (1    << 6)  // EL0 accessible
|  0b11; // Valid entry

#[no_mangle]
#[link_section = ".init"]
#[naked]
extern "C" fn _start() {
    unsafe {
        core::arch::asm! {
            "mov x0, 0xFF8",
            "mov x1, xzr",
            "ldr x2, [x1]",
            "str x2, [x0]",

            // Allocate a new page for the next stage ELF
            "svc 0x3000", // If the allocation fails, we're in trouble anyways, so don't bother error checking
            "ldr x0, ={TABLE_ENTRY_BASE}",
            "mov x2, x1",
            "orr x3, x1, x0",
            "mov x0, 8",
            "str x3, [x0]",
            "dmb ish",
            "dsb sy",
            "isb",

            // Read the (possibly misaligned) size of the next stage ELF, into x1
            // x2 contains the start of the actual ELF
            "adr x3, __elf_start",
            "ldrb w0, [x3], 1",
            "ldrb w1, [x3], 1",
            "orr x1, x0, x1, LSL 8",

            // Copy the ELF to its new page
            "mov x0, 0x10000",
            "mov sp, 0x10000",
            "mov x4, x1",
            "0: subs x4, x4, 1",
            "ldrb w5, [x3, x4]",
            "strb w5, [x0, x4]",
            "b.pl 0b",

            // Zero BSS
            "adr x3, __bss_start",
            "adr x4, __bss_end",
            "0: strb wzr, [x3], 1",
            "cmp x3, x4",
            "b.ls 0b",

            // Set the exception context
            "adr x0, {user_context}", // User context

            // "adr x3, {main}", // Set the RA to be `main` when we reenter the program
            // "ldr x4, [x0, 8]",
            // "str x3, [x4], 8", // Store the RA
            // "str x4, [x0, 8]", // Bump the exception SP

            "mov x4, 0x10000",
            "str x4, [sp, -0x100]!",
            "str x1, [sp, 0x8]", // Set arguments to main
            "str x2, [sp, 0x10]",

            "mov x1, xzr", // TTBR0_EL1, guaranteed to be 0
            "mov x2, {TCR_EL1}", // TODO: actually make TCR_EL1 meaningful
            "svc 0x4000",
            user_context = sym CONTEXT,
            TCR_EL1 = const 0, // TODO
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

fn temporary_map(va: usize, pa: u64) {
    let index = va >> 16;
    unsafe {
        asm! {
            "str {}, [{}]",
            in(reg) INIT_TABLE_ENTRY_BASE | pa,
            in(reg) index * 8,
        }
    }
}

/// The entry point of the init program. Spawns all the other programs before exiting
/// # Safety
/// `next_part` and `next_len` must describe a valid, accessible ELF in memory, including padding bytes to the nearest `u64` boundary.
/// # Panics
/// Panics if `next_part` is not page aligned
unsafe extern "C" fn main(next_part: *mut u64, next_len: u16, pa: usize) -> ! {
    ADDRESS_SPACE.set(SpinLock::new(unsafe {
        AddressSpace::new(NonNull::new(0x1FF_0000 as *mut _).expect("Received a null page table"))
    }));
    let mut uart = Stdout {};
    syscalls::write("Hello from usermode!\n".as_bytes());
    println!("PID: {:X}", getpid());
    assert!(!next_part.is_null());
    assert_eq!(pa & 0xFFFF, 0);

    // SAFETY: The caller promises that the arguments refer to a valid ELF, padding included
    let elf = unsafe {
        core::slice::from_raw_parts(
            next_part,
            usize::from(next_len).div_ceil(mem::size_of::<usize>()),
        )
    };

    // alloc new pd
    let new_pd = syscalls::alloc_page().unwrap();
    writeln!(&mut uart, "got {:X}\n", new_pd);
    temporary_map(0x2_0000, new_pd);
    let virt_new_pd = 0x2_0000 as *mut _;
    let mut address_space: AddressSpace<16, 25> = unsafe {
        AddressSpace::new(NonNull::new(virt_new_pd).expect("Received a null page table"))
    };

    //elf load
    let (entry, bss_start, bss_end, ctx, sp) =
        vm::load_elf(&mut address_space, new_pd, elf, pa.try_into().unwrap(), &[]).unwrap();

    // fork+exec into it

    // syscalls::fork();
    syscalls::exec(ctx as *mut _, new_pd, 0, sp - 0x100).unwrap();

    // - cow fork
    // - replace PD with new one
    loop {
        core::hint::spin_loop();
    }
    syscalls::write("Unreachable!\n".as_bytes());
    syscalls::exit()
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    // Make sure that this doesn't overlap with other peripheral accesses
    let mut uart = Stdout {};

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
    loop {
        hint::spin_loop();
    }
}

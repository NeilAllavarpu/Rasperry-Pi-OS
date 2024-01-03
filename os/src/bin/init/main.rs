#![no_main]
#![no_std]
#![feature(naked_functions)]
#![feature(asm_const)]
#![feature(inline_const)]
#![feature(generic_arg_infer)]
#![feature(strict_provenance_atomic_ptr)]
#![feature(panic_info_message)]
#![feature(strict_provenance)]

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

use stdos::os::{
    syscalls::{self, alloc_page},
    vm::{self, AddressSpace},
};

const INIT_TABLE_ENTRY_BASE: u64 = (1 << 53) // Privileged execute-never
| (1    << 11) // Non-global entry
| (1    << 10) // Access flag
| (0b11 << 8)  // Shareability
| (1    << 6)  // EL0 accessible
|  0b11; // Valid entry

#[repr(C)]
struct UserContext {
    exception_vector: extern "C" fn(),
    exception_stack: AtomicPtr<AtomicU64>,
    register_save_area: [u64; 32],
    exception_stack_mem: &'static [AtomicU64],
}

impl UserContext {
    fn pop(&self) -> Option<u64> {
        let ptr = unsafe {
            CONTEXT
                .exception_stack
                .fetch_ptr_sub(1, Ordering::SeqCst)
                .sub(1)
        };
        let idx =
            (ptr.addr() - CONTEXT.exception_stack_mem.as_ptr().addr()) / mem::size_of::<u64>();
        CONTEXT
            .exception_stack_mem
            .get(idx)
            .map(|atomic| atomic.load(Ordering::Relaxed))
    }
}

static CONTEXT: UserContext = UserContext {
    exception_vector: _exception_handler,
    exception_stack: AtomicPtr::new(EXCEPTION_STACK.as_ptr().cast_mut()),
    exception_stack_mem: &EXCEPTION_STACK,
    register_save_area: {
        let mut area = [0; 32];
        area[0] = 0x1_0000;
        area[29] = 0x5678; // FP
        area[30] = 0x1234; // LR
        area[31] = 0x1_0000; // SP
        area
    },
};

static EXCEPTION_STACK: [AtomicU64; 8] = [const { AtomicU64::new(0) }; _];

#[no_mangle]
#[link_section = ".init"]
#[naked]
extern "C" fn _start() {
    unsafe {
        core::arch::asm! {
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

            "adr x3, {main}", // Set the RA to be `main` when we reenter the program
            "ldr x4, [x0, 8]",
            "str x3, [x4], 8", // Store the RA
            "str x4, [x0, 8]", // Bump the exception SP

            "str x1, [x0, 0x18]", // Set arguments to main
            "str x2, [x0, 0x20]",

            "mov x1, xzr", // TTBR0_EL1, guaranteed to be 0
            "mov x2, {TCR_EL1}", // TODO: actually make TCR_EL1 meaningful
            "svc 0x4000",
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
    let mut uart = Stdout {};
    syscalls::write("Hello from usermode!\n".as_bytes());
    writeln!(&mut uart, "{next_part:X?} {next_len:X?} {pa:X?}");

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
    let new_pd = alloc_page().unwrap();
    writeln!(&mut uart, "got {:X}\n", new_pd);
    temporary_map(0x2_0000, new_pd);
    let virt_new_pd = 0x2_0000 as *mut _;
    let mut address_space: AddressSpace<16, 25> = unsafe {
        AddressSpace::new(NonNull::new(virt_new_pd).expect("Received a null page table"))
    };

    //elf load
    let (entry, bss_start, bss_end, ctx) =
        vm::load_elf(&mut address_space, elf, pa.try_into().unwrap()).unwrap();

    write!(&mut uart, "point {entry:X?} {ctx:X?}\n");

    // // fork+exec into it
    // let val = u32::from_le(unsafe { (entry as *mut u32).read() });
    // let new_ctx =
    //     (entry + u64::from(((val >> 5) & 0x7_FFFF) | ((val >> 29) & 0b11))) as *mut UserContext;

    // write!(&mut uart, "entry {new_ctx:X?}\n");
    // unsafe { *(new_ctx as *mut u64) = entry };

    // syscalls::fork();
    syscalls::exec(ctx as *mut _, new_pd, 0).unwrap();

    // - cow fork
    // - replace PD with new one
    loop {
        core::hint::spin_loop();
    }
    syscalls::write("Unreachable!\n".as_bytes());
    syscalls::exit()
}

#[naked]
extern "C" fn _exception_handler() {
    static HANDLER_TABLE: [extern "C" fn(); 3] = [preemption, resumption, page_fault];
    // save a pair of regs to special save area
    // read the signal code and branch accordingly

    // preemption: backup registers to save area (can't use stack bc of overflow potential)
    // resumption: restore registers from save area
    // pg fault: save necessary registers and call extern "C" handler

    unsafe {
        asm! {
            "cmp sp, xzr",
            "b.eq {resumption}",
            "stp x0, x1, [sp, -16]!",
            "stp x2, x3, [sp, -16]!",
            "adr x0, {ctx}",
            "ldr x1, [x0, 8]!",
            "ldr x2, [x1, -8]!",
            "str x1, [x0]",
            "adr x0, {table}",
            "ldr x0, [x0, x2, LSL #3]",
            "blr x0",
            ctx = sym CONTEXT,
            table = sym HANDLER_TABLE,
            resumption = sym resumption,
            options(noreturn)
        }
    }
}

extern "C" fn page_fault() {
    todo!("pg fault")
}

extern "C" fn preemption() {
    todo!("preempt")
}

#[naked]
extern "C" fn resumption() {
    unsafe {
        asm! {
            "adr x0, {ctx}",
            "ldr x1, [x0, 8]",
            "sub x1, x1, 8",
            "str x1, [x0, 8]",
            "ldnp x2,  x3,  [x0, 0x20]",
            "ldnp x4,  x5,  [x0, 0x30]",
            "ldnp x6,  x7,  [x0, 0x40]",
            "ldnp x8,  x9,  [x0, 0x50]",
            "ldnp x10, x11, [x0, 0x60]",
            "ldnp x12, x13, [x0, 0x70]",
            "ldnp x14, x15, [x0, 0x80]",
            "ldnp x16, x17, [x0, 0x90]",
            "ldnp x18, x19, [x0, 0xA0]",
            "ldnp x20, x21, [x0, 0xB0]",
            "ldnp x22, x23, [x0, 0xC0]",
            "ldnp x24, x25, [x0, 0xD0]",
            "ldnp x26, x27, [x0, 0xE0]",
            "ldnp x30, x29, [x0, 0x100]",
            "mov sp, x29",
            "ldnp x28, x29, [x0, 0xF0]",
            "ldnp x0,  x1,  [x0, 0x10]",
            "svc 0x0",
            ctx = sym CONTEXT,
            options(noreturn)
        }
    }
}

// extern "C" fn other

extern "C" fn exception_handler() -> ! {
    let mut uart = Stdout {};
    match CONTEXT.pop().unwrap() {
        0 => {
            todo!("preepmtion");
        }
        1 => {
            syscalls::write("resume".as_bytes());
            let save_ptr = CONTEXT.register_save_area.as_ptr();
            unsafe {
                asm! {
                    // TODO: reset SP somehow?
                    "ldp x30, x31, [x0], 0x10",
                    "ldp x28, x29, [x0], 0x10",
                    "ldp x26, x27, [x0], 0x10",
                    "ldp x24, x25, [x0], 0x10",
                    "ldp x22, x23, [x0], 0x10",
                    "ldp x20, x21, [x0], 0x10",
                    "ldp x18, x19, [x0], 0x10",
                    "ldp x16, x17, [x0], 0x10",
                    "ldp x14, x15, [x0], 0x10",
                    "ldp x12, x13, [x0], 0x10",
                    "ldp x10, x11, [x0], 0x10",
                    "ldp x8,  x9,  [x0], 0x10",
                    "ldp x6,  x7,  [x0], 0x10",
                    "ldp x4,  x5,  [x0], 0x10",
                    "ldp x2,  x3,  [x0], 0x10",
                    "ldp x0,  x1,  [x0]",
                    "svc 0x0",
                    in("x0") save_ptr,
                    options(nostack, noreturn)
                }
            }
            unreachable!();
            // todo!("resumption, target {:?}", CONTEXT.pop());
        }
        2 => {
            unreachable!("pg fault!");
        }
        x => todo!("signal {x}"),
    }
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

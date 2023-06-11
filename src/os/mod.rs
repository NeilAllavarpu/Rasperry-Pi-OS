pub mod syscalls;
pub mod vm;

use crate::cell::InitCell;
use crate::sync::SpinLock;
use core::arch::asm;
use core::ptr::NonNull;
use vm::AddressSpace;

/// The entry point of a program. Maps a stack for itself and then hands control off to Rust
/// initialization code
#[no_mangle]
#[naked]
#[linkage = "weak"]
extern "C" fn _start(ttbr0_virtual: *mut ()) -> ! {
    /// Size of the base translation table, in bytes
    const TTBR0_SIZE: usize = 4096;
    /// Page size, in bytes
    const PAGE_SIZE: usize = 0x1_0000;
    /// Page descriptor template to be used for the stack
    const DESCRIPTOR_BASE: u64 = (1 << 54) // Unprivileged execute-never
            | (1  << 53) // Privileged execute-never
            | (1 << 10) // Access flag
            | (0b11 << 8) // Shareability
            | (1 << 6)
            | 0b11; // Valid entry

    // SAFETY: This is proper ASM to traverse a linear table and set the appropriate entry for the
    // stack, and then jump off to Rust code
    unsafe {
        asm!(
            "cmp x1, x2",
            "b.eq 1f",
            "0: strb wzr, [x1], #1",
            "cmp x1, x2",
            "b.ne 0b",
            "1:",
            "mov x20, x0",                  // Backup the virtual address of the translation table
            "mov x0, #0x100000",            // TODO: Ask the kernel for a physical frame here, and
                                            // handle errors somehow
            "ldr x3, ={DESCRIPTOR}",        // Load the basic descriptor template for the stack
            "orr x3, x3, x0",               // Apply the given frame
            "mov x0, x20",                  // Restore the virtual address of the translation table
            "add x1, x20, {SIZE}",          // Start at the very last descriptor
            "0:",                           // While we haven't mapped the stack
            "   ldr x2, [x1, #-8]!",       // Load the appropriate descriptor
            "   tbnz x2, #0, 0b",           // Skip if the descriptor is nonzero
            "str x3, [x1]",                 // Set the stack descriptor
            "sub x1, x1, x0",               // Figure out the offset from the start of the table
            "lsl x1, x1, #{SHIFT_BITS}",    // Scale the offset to form a virtual address
            "add sp, x1, #{PAGE_SIZE}",     // Adjust because stacks grow down from the top of
                                            // their region
            "mov fp, #0",
            "mov lr, #0",
            "b {wrapper}",
            wrapper = sym wrapper,
            SIZE = const TTBR0_SIZE,
            DESCRIPTOR = const DESCRIPTOR_BASE,
            SHIFT_BITS = const (PAGE_SIZE / 8).ilog2(),
            PAGE_SIZE = const PAGE_SIZE,
            options(noreturn))
    }
}

extern "C" {
    fn main();
}

/// Performs initialization functions, and then hands off execution to `main`
extern "C" fn wrapper(ttbr0_virtual: *mut ()) -> ! {
    /// Page size, in bytes
    const PAGE_SIZE: usize = 1 << 16;
    assert!(
        ttbr0_virtual.is_aligned_to(0x1000),
        "Received an unaligned page table"
    );
    // SAFETY: It is up to the caller to guarantee that this is a valid table address
    let address_space: AddressSpace<16, 25> = unsafe {
        #[allow(clippy::expect_used)]
        AddressSpace::new(NonNull::new(ttbr0_virtual).expect("Received a null page table"))
    };
    // SAFETY: No one else can access this address space yet, and will not be able to do so until
    // `main` is called
    unsafe { vm::ADDRESS_SPACE.set(SpinLock::new(address_space)) };
    // SAFETY: It is up to the application to properly define `main`
    unsafe { main() };
    todo!("Handle process cleanup and exit");
}

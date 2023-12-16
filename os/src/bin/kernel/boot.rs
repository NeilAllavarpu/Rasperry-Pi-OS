//! The initial boot sequence of the kernel. Execution begins here and this assembly sets up the
//! necessary virtual memory, stack, BSS, and anything else necessary for safe Rust execution to
//! begin

use core::mem::MaybeUninit;

/// Physical address where the init program is initially loaded to.
const INIT_PHYSICAL_ADDRESS: usize = 0x1000;

/// Page size for the kernel, # bits
/// These are huge pages, 2MB each
const PAGE_BITS: u32 = 21;
/// Number of usable bits in virtual addresses
const ADDRESS_BITS: u32 = 25;

/// Physical address that the kernel is loaded to
const PHYSICAL_LOAD_ADDR: usize = 0x8_0000;
/// Virtual address that the kernel is linked to
const VIRTUAL_LINK_ADDR: usize = 0xFFFF_FFFF_FE08_0000;
/// Shift to transform from physical to virtual addresses
const VIRTUAL_OFFSET: usize = VIRTUAL_LINK_ADDR - PHYSICAL_LOAD_ADDR;

/// Kernel translation table struct
#[repr(C)]
#[repr(align(4096))]
#[expect(
    clippy::as_conversions,
    reason = "Necessary for const conversion to the appropriate type"
)]
pub struct TranslationTable(pub [MaybeUninit<u64>; 1 << (ADDRESS_BITS - PAGE_BITS) as usize]);
/// The kernel's translation table
pub static mut TRANSLATION_TABLE: TranslationTable = TranslationTable([MaybeUninit::uninit(); _]);

/// Number of cores
const NUM_CORES: usize = 4;
/// Stack size per core, in bytes
pub const STACK_SIZE: usize = 0x2000;

/// The initial configuration for `TCR_EL1`, to set up kernel higher-half virtual memory
#[expect(
    clippy::as_conversions,
    reason = "Necessary for const conversion to the appropriate type"
)]
const TCR_EL1_INIT: u64 =
    (1    << 56) // E0PD1: EL0 access to the higher half always generates a fault
    + (1    << 52) // Disable checking the top byte of instruction pointers
    + (1    << 51) // Same as above, for EL0
    + (0xFF << 43) // HW use enabled for certain bits of the page descriptors
    + (1    << 40) // HW managed dirty bits
    + (1    << 39) // HW managed access bits
    + (1    << 38) // Disable checking the top byte of data pointers
    + (1    << 37) // Same as above, for EL0
    + (1    << 36) // 16-bit ASID
    + (0b100<< 32) // 36-bit physical addresses (the RPI only uses 35)
    + (0b10 << 30) // 4K pages in EL1 (We use huge 2MB pages)
    + (0b11 << 28) // Inner-shareable memory for page walks
    + (0b11 << 26) // Outer-cacheable memory for page walks
    + (0b11 << 24) // Inner-cacheable memory for page walks
    + ((64 - (ADDRESS_BITS as u64)) << 16) // 25-bit virtual addresses
    + (0b01 << 14) // 64K pages in EL0
    + (0b11 << 12) // Inner-shareable memory for page walks
    + (0b11 << 10) // Outer-cacheable memory for page walks
    + (0b11 << 8) // Inner-cacheable memory for page walks
    + (64 - (ADDRESS_BITS as u64))
    // 25-bit virtual addresses
;

/// The configuration for `SCTLR_EL1`, to set up basic EL1 functionality and pass on most controls to
/// EL0
const SCTLR_EL1: u64 = (1       << 60) // Disable trapping TPIDR2 accesses
    | (0x1F  << 52) // Disable trapping various memory operations
    | (0b11  << 42) // Allow allocation tags
    | (1     << 33) // Allow memory copy & set instructions
    | (1     << 32) // Disable cache operations at EL0 if no write permissions
    | (1     << 28) // Do not trap device accessess at EL0
    | (1     << 26) // Do not trap EL0 cache operations
    | (1     << 22) // EL1 exceptions are context synchronizing
    | (0b101 << 16) // Do not trap WFE/WFI
    | (0b11  << 14) // Do not trap EL0 cache operations
    | (1     << 12) // Instruction caching
    | (1     << 11) // Exception returns are context synchronizing
    | (1     << 6)  // If possible, disable misalignment exceptions
    | (1     << 2)  // Data caching
    |  1; // Enable virtual memory

/// The configuration for `SCTLR_EL2`, to pass on all controls to EL1/EL0
const SCTLR_EL2: u64 = (1 << 60) // Disable trapping TPIDR2 accesses
    | (0x1F  << 52) // Disable trapping various memory operations
    | (0b11  << 42) // Allow allocation tags
    | (1     << 33) // Allow memory copy & set instructions
    | (1     << 32) // Disable cache operations at EL0 if no write permissions
    | (1     << 28) // Do not trap device accessess at EL0
    | (1     << 26) // Do not trap EL0 cache operations
    | (1     << 22) // EL1 exceptions are context synchronizing
    | (0b101 << 16) // Do not trap WFE/WFI
    | (0b11  << 14) // Do not trap EL0 cache operations
    | (1     << 12) // Instruction caching
    | (1     << 11) // Exception returns are context synchronizing
    | (1     << 10) // Enable more EL0 instructions
    | (1     << 6)  // If possible, disable misalignment exceptions
    | (1     << 5)  // Enable EL0 system barriers
    | (1     << 2); // Data caching

/// The configuration for `HCR_EL2`, to disable hypervisor controls
const HCR_EL2: u64 = (1 << 56) // Allow allocation tag access
    | (1 << 41) // Disables pointer authentication trapping
    | (1 << 40) // Same as above
    | (1 << 39) // Allows access to TME
    | (1 << 38) // Allows incoherency if inner and outer cacheability differ
    | (1 << 31) // EL1 is 64-bit
    | (1 << 29); // Disables HVC instruction

/// The base bits for a kernel table entry to normal memory
pub const TABLE_ENTRY_BASE: u64 = (1 << 54)  // Unprivileged execute-never
    | (1    << 10) // Access flag
    | (0b11 << 8)  // Shareability
    |  0b01; // Valid entry (Block descriptor)

/// The bits for the init program table entry
const INIT_TABLE_ENTRY_BASE: u64 = (1    << 53) // Privileged execute-never
    | (1    << 11) // Non-global entry
    | (1    << 10) // Access flag
    | (0b11 << 8)  // Shareability
    | (1    << 6)  // EL0 accessible
    |  0b11; // Valid entry

/// The configuration for `CPACR_EL1`, to disable various trapping events
const CPACR_EL1: u64 = (0b11 << 24) // Disable SME trapping
    | (0b11 << 20) // Disable FP trapping
    | (0b11 << 16); // Disable SVE trapping

/// The entry point of the kernel
/// * Clears the BSS
/// * Sets up the kernel page table
/// * Wakes up the other cores
/// # Safety
/// Should never be called manually, only by the bootloader
#[no_mangle]
#[naked]
#[link_section = ".init"]
unsafe extern "C" fn _start() {
    // SAFETY: We need to use this assembly to set a stack pointer
    unsafe {
        core::arch::asm! {
            "msr DAIFSET, 0b1111", // First, disable interrupts

            // Move the DTB to a more suitable location
            "ldr w1, [x0, 4]", // Size, as big endian
            "rev w1, w1",
            "mov w27, w1",
            "add w1, w1, 37",
            "lsr w1, w1, 3",
            "mov x2, 0x10000",
            "0: ldr x3, [x0], 8",
            "str x3, [x2], 8",
            "sub w1, w1, 1",
            "cbnz w1, 0b",

            "mov x0, 0x10000",
            "ldr x1, ={VIRTUAL_OFFSET}",
            "add x0, x0, x1",

            // Move the init program to a suitable location
            "adr x10, __init_start", // The end of the kernel data, and where init data begins

            // Get the number of bytes of init data
            // Since this may be unaligned, we do multiple stores to get the data
            "ldrb w1, [x10]",
            "ldrb w2, [x10, 1]",
            "orr x1, x1, x2, LSL 8",
            "add x2, x10, 2",

            // Copy the contents to the appropriate location
            "ldr x3, ={INIT_PHYSICAL_ADDRESS}",
            "0:",
            "ldrb w4, [x2], 1",
            "strb w4, [x3], 1",
            "sub x1, x1, 1",
            "cbnz x1, 0b",

            // Set the first PTE of the init translation table to be user RWX appropriately
            "ldr x1, ={INIT_ENTRY}",
            "ldr x2, ={INIT_TABLE}",
            "str x1, [x2], 8",

            // Zero BSS
            "adr x1, __bss_end",
            "0: strb wzr, [x10], 1",
            "cmp x10, x1",
            "b.ls 0b",

            // Map the kernel
            // By assumption, the kernel fits within the first 2MB of physical memory
            // So we can map it with a single 2MB huge page
            "ldr x10, ={TABLE_ENTRY_BASE}", // Load metadata bits
            "adr x1, {TRANSLATION_TABLE}", // Get table address

            "str x10, [x1], 8", // Store table entry into the translation table
                               // Since the PA is 0, no extra work is needed to set it

            "orr x10, x10, 0b100",       // Mark as device memory
            "ldr x2, ={UART_ADDRESS}", // Do the same for the UART in the next page
            "orr x4, x10, x2",
            "str x4, [x1], 8",
            "ldr x2, =0x47E000000", // Do the same for the UART in the next page
            "orr x4, x10, x2",
            "str x4, [x1], 8",

            "ldr x2, =0x4C0040000", // gicc
            "orr x4, x10, x2",
            "str x4, [x1], 8",

            // Wake up other cores
            "mov x10, 0xE0",
            "adr x1, 1f",
            "str x1, [x10]",
            "str x1, [x10, 0x8]",
            "str x1, [x10, 0x10]",

            "dsb ishst", // Wait for the memory stores to complete before
            "sev",       // waking up the remaining cores

            "1:",
            "msr DAIFSET, 0b1111",

            "ldr x10, ={CPACR_EL1}",
            "msr CPACR_EL1, x10",
            "msr CPTR_EL2, xzr", // Disable trapping various functionality

            // Enable cache operation broadcasting
            "mrs x10, S3_1_c15_c2_1",
            "orr x10, x10, 0b1000000",
            "msr S3_1_c15_c2_1, x10",

            // Put the correct virtual return address for the ERET
            "adr x10, {main}",
            "ldr x30, ={VIRTUAL_OFFSET}",
            "add x10, x10, x30",
            "msr ELR_EL2, x10",

            "ldr x10, ={HCR_EL2}",
            "msr HCR_EL2, x10",

            "ldr x10, ={MAIR_EL1}",
            "msr MAIR_EL1, x10",

            "ldr x10, ={SCTLR_EL1}",
            "msr SCTLR_EL1, x10",

            "ldr x10, ={SCTLR_EL2}",
            "msr SCTLR_EL2, x10",

            // Select a stack and adjust to virtual addresses
            "mrs x1, MPIDR_EL1",
            "and x1, x1, 0b11",
            "add x1, x1, 1",

            "adr x10, __bss_end", // Start at end of BSS, rounded up to next multiple of 16 for alignment
            "add x10, x10, 15",
            "and x10, x10, -16",
            "mov x2, {STACK_SIZE}", // Add correct offset to find appropriate stack location
            "madd x10, x1, x2, x10",
            "add x10, x10, x30",
            "msr SP_EL1, x10",

            "ldr x10, ={SPSR_EL2}",
            "msr SPSR_EL2, x10",

            "ldr x10, ={TCR_EL1}",
            "msr TCR_EL1, x10",

            "ldr x10, =0b11",
            "msr CNTHCTL_EL2, x10",

            "ldr x10, =0b11",
            "msr CNTKCTL_EL1, x10",
            "mov x10, 0b1",
            "msr CNTP_CTL_EL0, x10",
            "msr CNTP_CVAL_EL0, xzr",

            "adr x10, {TRANSLATION_TABLE}",
            "orr x10, x10, 1",
            "msr TTBR1_EL1, x10",
            "mov w1, w27",

            "eret",
            CPACR_EL1 = const CPACR_EL1,
            HCR_EL2 = const HCR_EL2,
            INIT_ENTRY = const INIT_TABLE_ENTRY_BASE,
            INIT_PHYSICAL_ADDRESS = const INIT_PHYSICAL_ADDRESS,
            INIT_TABLE = const crate::INIT_TRANSLATION_ADDRESS,
            main = sym crate::main, // Main initialization sequence
            MAIR_EL1 = const 0xFF, // Attribute for normal memory at index 0, and device memory
                                   // at index 1
            SCTLR_EL1 = const SCTLR_EL1,
            SCTLR_EL2 = const SCTLR_EL2,
            SPSR_EL2 = const (0b1111 << 6) | 0b0101, // Use SP_EL1 with interrupts disabled
            STACK_SIZE = const STACK_SIZE,
            TABLE_ENTRY_BASE = const TABLE_ENTRY_BASE,
            TCR_EL1 = const TCR_EL1_INIT,
            TRANSLATION_TABLE = sym TRANSLATION_TABLE,
            UART_ADDRESS = const 0x4_7E20_0000_u64,
            VIRTUAL_OFFSET = const VIRTUAL_OFFSET,
            options(noreturn)
        }
    }
}
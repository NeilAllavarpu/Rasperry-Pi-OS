//! The initial boot sequence of the kernel. Execution begins here and this assembly sets up the
//! necessary virtual memory, stack, BSS, and anything else necessary for safe Rust execution to
//! begin

use core::mem::MaybeUninit;

/// Page size for the kernel, # bits. These are huge pages, 2MB each
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
struct TranslationTable([MaybeUninit<u64>; 1_usize << (ADDRESS_BITS - PAGE_BITS)]);
/// The kernel's translation table
static mut TRANSLATION_TABLE: TranslationTable = TranslationTable([MaybeUninit::uninit(); _]);

/// Stack size per core, in bytes
pub const STACK_SIZE: usize = 0x2000;

core::arch::global_asm! {
    ".section .init",
    "_start:",
    "msr DAIFSET, 0b1111", // First, disable interrupts
    "ldr x8, ={VIRTUAL_OFFSET}",

    // Move the DTB to a more suitable location
    "ldr w1, [x0, 4]", // Size, as big endian
    "rev w1, w1",      // Convert to host (little) endianness
    "and x1, x1, 0xFFFF", // Only handle sizes under 64K for simplicity
                          // (fits into a single page that can be passed to the init program)
    "mov x2, 0x10000",
    "mov x3, x1",
    "0: subs x3, x3, 8",
    "ldr x4, [x0], 8",
    "str x4, [x2], 8",
    "b.pl 0b",

    "add x0, x8, 0x10000", // Put the VA into x0 for the main init sequence

    // Move the init program to a suitable location
    "adr x2, __init_start", // The end of the kernel data, and where init data begins

    // Get the number of bytes of init data
    // Since this may be unaligned, we do multiple loads to get the data
    "ldrb w3, [x2], 1",
    "ldrb w4, [x2], 1",
    "orr x3, x3, x4, LSL 8",

    // Copy the contents to the appropriate location
    "ldr x4, ={INIT_PHYSICAL_ADDRESS}",
    "0: subs x3, x3, 1",
    "ldrb w5, [x2], 1",
    "strb w5, [x4], 1",
    "b.pl 0b",

    // Set the first PTE of the init translation table to be user RWX appropriately
    "ldr x3, ={INIT_ENTRY}",
    "ldr x4, ={INIT_TABLE}",
    "str x3, [x4]",

    // Zero BSS
    "adr x2, __bss_start",
    "adr x3, __bss_end",
    "0: strb wzr, [x2], 1",
    "cmp x2, x3",
    "b.ls 0b",

    // Map the kernel
    // By assumption, the kernel fits within the first 2MB of physical memory
    // So we can map it with a single 2MB huge page
    "ldr x3, ={TABLE_ENTRY_BASE}", // Load metadata bits
    "adr x2, {TRANSLATION_TABLE}", // Get table address

    "str x3, [x2], 8", // Store table entry into the translation table
                       // Since the PA is 0, no extra work is needed to set it
    "orr x3, x3, 0b100",       // Mark as device memory
    "ldr x4, ={UART_ADDRESS}", // Do the same for the UART in the next page
    "orr x4, x3, x4",
    "str x4, [x2], 8",
    "ldr x4, =0x47E000000", // mailbox
    "orr x4, x3, x4",
    "str x4, [x2], 8",
    "ldr x4, =0x4C0040000", // gicc
    "orr x4, x3, x4",
    "str x4, [x2]",

    "dmb ishst",

    "adr x2, _start_per_core",
    "mov x3, 0xE0",
    "str x2, [x3]",
    "str x2, [x3, 8]",
    "str x2, [x3, 16]",
    "dsb ishst",
    "isb",
    "sev",

    ".global _start_per_core",
    "_start_per_core: msr DAIFSET, 0b1111",
    "ldr x8, ={VIRTUAL_OFFSET}",

    "ldr x2, ={CPACR_EL1}",
    "msr CPACR_EL1, x2",
    "msr CPTR_EL2, xzr", // Disable trapping various functionality

    // Enable cache operation broadcasting
    "mrs x2, S3_1_c15_c2_1",
    "orr x2, x2, 0b1000000",
    "msr S3_1_c15_c2_1, x2",
    "ldr x30, ={VIRTUAL_OFFSET}",

    // Put the correct virtual return address for the ERET
    "adr x2, {main}",
    "add x2, x2, x8",
    "msr ELR_EL2, x2",

    "ldr x2, ={HCR_EL2}",
    "msr HCR_EL2, x2",

    "ldr x2, ={MAIR_EL1}",
    "msr MAIR_EL1, x2",

    "ldr x2, ={SCTLR_EL1}",
    "msr SCTLR_EL1, x2",

    "ldr x2, ={SCTLR_EL2}",
    "msr SCTLR_EL2, x2",

    // Select a stack and adjust to virtual addresses
    "mrs x2, MPIDR_EL1",
    "and x2, x2, 0b11",
    "add x2, x2, 1",

    "adr x3, __bss_end", // Start at end of BSS, rounded up to next multiple of 16 for alignment
    "add x3, x3, 15",
    "and x3, x3, -16",
    "mov x4, {STACK_SIZE}", // Add correct offset to find appropriate stack location
    "madd x2, x4, x2, x3",
    "add x2, x2, x8",
    "msr SP_EL1, x2",

    "ldr x2, ={SPSR_EL2}",
    "msr SPSR_EL2, x2",

    "ldr x2, ={TCR_EL1}",
    "msr TCR_EL1, x2",

    "ldr x2, =0b11",
    "msr CNTHCTL_EL2, x2",
    "msr CNTKCTL_EL1, x2",
    "mov x2, 0b1",
    "msr CNTP_CTL_EL0, x2",
    "msr CNTP_CVAL_EL0, xzr",

    "adr x2, {TRANSLATION_TABLE}",
    "orr x2, x2, 1",
    "msr TTBR1_EL1, x2",

    "eret",
    CPACR_EL1 = const
          (0b11 << 24) // Disable SME trapping
        | (0b11 << 20) // Disable FP trapping
        | (0b11 << 16), // Disable SVE trapping,
    HCR_EL2 = const
          (1_u64 << 56) // Allow allocation tag access
        | (1     << 41) // Disables pointer authentication trapping
        | (1     << 40) // Same as above
        | (1     << 39) // Allows access to TME
        | (1     << 38) // Allows incoherency if inner and outer cacheability differ
        | (1     << 31) // EL1 is 64-bit
        | (1     << 29), // Disables HVC instruction,
    INIT_ENTRY = const
          (1_u64 << 53) // Privileged execute-never
        | (1     << 11) // Non-global entry
        | (1     << 10) // Access flag
        | (0b11  << 8)  // Shareability
        | (1     << 6)  // EL0 accessible
        |  0b11,       // Valid entry
    INIT_PHYSICAL_ADDRESS = const 0x1000,
    INIT_TABLE = const crate::INIT_TRANSLATION_ADDRESS,
    main = sym crate::main, // Main initialization sequence
    MAIR_EL1 = const 0xFF, // Attribute for normal memory at index 0, and device memory at index 1
    SCTLR_EL1 = const
          (1_u64 << 60) // Disable trapping TPIDR2 accesses
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
        |  1,           // Enable virtual memory,
    SCTLR_EL2 = const
          (1_u64 << 60) // Disable trapping TPIDR2 accesses
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
        | (1     << 2), // Data caching,
    SPSR_EL2 = const (0b1111 << 6) | 0b0101, // Use SP_EL1 with interrupts disabled
    STACK_SIZE = const STACK_SIZE,
    TABLE_ENTRY_BASE = const
          (1_u64 << 54)  // Unprivileged execute-never
        | (1    << 10) // Access flag
        | (0b11 << 8)  // Shareability
        |  0b01, // Valid entry (Block descriptor)
    TCR_EL1 = const
    #[expect(
        clippy::as_conversions,
        reason = "Necessary for const conversion to the appropriate type"
    )](
          (1    << 56) // E0PD1: EL0 access to the higher half always generates a fault
        | (1    << 52) // Disable checking the top byte of instruction pointers
        | (1    << 51) // Same as above, for EL0
        | (0xFF << 43) // HW use enabled for certain bits of the page descriptors
        | (1    << 40) // HW managed dirty bits
        | (1    << 39) // HW managed access bits
        | (1    << 38) // Disable checking the top byte of data pointers
        | (1    << 37) // Same as above, for EL0
        | (1    << 36) // 16-bit ASID
        | (0b100<< 32) // 36-bit physical addresses (the RPI only uses 35)
        | (0b10 << 30) // 4K pages in EL1 (We use huge 2MB pages)
        | (0b11 << 28) // Inner-shareable memory for page walks
        | (0b11 << 26) // Outer-cacheable memory for page walks
        | (0b11 << 24) // Inner-cacheable memory for page walks
        | ((64 - (ADDRESS_BITS as u64)) << 16) // Virtual address bits
        | (0b01 << 14) // 64K pages in EL0
        | (0b11 << 12) // Inner-shareable memory for page walks
        | (0b11 << 10) // Outer-cacheable memory for page walks
        | (0b11 << 8) // Inner-cacheable memory for page walks
        | (64 - (ADDRESS_BITS as u64)) // EL0 virtual address bits
    ),
    TRANSLATION_TABLE = sym TRANSLATION_TABLE,
    UART_ADDRESS = const 0x4_7E20_0000_u64,
    VIRTUAL_OFFSET = const VIRTUAL_OFFSET,
}

use core::{
    arch::asm,
    ptr::{addr_of, addr_of_mut},
};

/// Number of cores
const NUM_CORES: usize = 4;

/// Physical address that the kernel is loaded to
const PHYSICAL_LOAD_ADDR: usize = 0x8_0000;
/// Virtual address that the kernel is linked to
const VIRTUAL_LOAD_ADDR: usize = 0xFFFF_FFFF_FE00_0000;
/// Offset between the virtual and physical addresses
const VIRTUAL_OFFSET: usize = VIRTUAL_LOAD_ADDR - PHYSICAL_LOAD_ADDR;

const PAGE_SIZE: usize = 64 * 1024;
#[allow(clippy::as_conversions)]
const PAGE_SIZE_BITS: u8 = PAGE_SIZE.ilog2() as u8;
const ADDRESS_BITS: u8 = 25;
const VIRTUAL_BASE: usize = 0xFFFF_FFFF_FE00_0000;
//1) & ADDRESS_BITS;
//
#[repr(C)]
#[repr(align(4096))]
#[allow(clippy::as_conversions)]
pub(crate) struct TranslationTable([u64; 1 << (ADDRESS_BITS - PAGE_SIZE_BITS) as usize]);
pub(crate) static mut TRANSLATION_TABLE: TranslationTable = TranslationTable([0; _]);

/// The entry point of the kernel
/// * Clears the BSS
/// * Sets up the kernel page table
/// * Wakes up the other cores
/// # Safety
/// Should never be called manually, only by the bootloader
#[no_mangle]
#[naked]
#[link_section = ".text._start"]
unsafe extern "C" fn _start() -> ! {
    // SAFETY: We need to use this assembly to set a stack pointer
    unsafe {
        asm!(
            "msr DAIFSET, #0b1111", // First, disable interrupts
            // Since this is core 0, give it a stack corresponding to the 0th physical (kernel-sized) page
            "mov sp, {PAGE_SIZE}",
            "b {start_rust}", // Perform the main initialization; this should never return
            PAGE_SIZE = const PAGE_SIZE,
            start_rust  = sym start_rust,
            options(noreturn)
        )
    }
}

#[naked]
/// The per-core entry point of the kernel
/// * Sets up the virtual address configuration
/// * Sets up the execution state to begin running the main kernel initialization
/// * Performs any necessary EL2 configuration
/// * Lowers privilege level to EL1
/// # Safety
/// Should only be called once per core, in the boot sequence
unsafe extern "C" fn _per_core_start() -> ! {
    // SAFETY: We need to use this assembly to set a stack pointer
    unsafe {
        asm!(
            "msr DAIFSET, #0b1111", // First, disable interrupts
            "mrs x0, MPIDR_EL1", // Load the core ID into the other
            "and x0, x0, #0b11", // Mask out higher bits
            "add x0, x0, #1", // Add one to start the stack pointer at the high part of the page
            "lsl x0, x0, {PAGE_SIZE_BITS}", // Scale the index by the page size
            "mov sp, x0", // Set the sp
            "b {per_core_start_rust}", // Perform the remaining initialization; this should never return
            PAGE_SIZE_BITS = const PAGE_SIZE_BITS,
            per_core_start_rust = sym per_core_start_rust,
            options(noreturn)
        )
    }
}

/// The (almost) initial boot code for the kernel;
/// runs on the initial core only
/// # Safety
/// Should only be called once, in the boot process
unsafe extern "C" fn start_rust() -> ! {
    const fn generate_descriptor(
        target: usize,
        _readable: bool,
        writeable: bool,
        executable: bool,
    ) -> u64 {
        (1 << 54) // Unprivileged execute-never
                            | (((!executable) as u64) << 53) // Privileged execute-never
                            | (target& !(PAGE_SIZE - 1)) as u64 // Phyiscal address
                            | (1 << 10) // Access flag
                                        | (0b11 << 8) // Shareability
                            | (((!writeable) as u64) << 7) // Not writeable
                            | 0b11 // Valid entry
    }
    /// Maps the contiguous physical region starting at the given place to the given contiguous virtual address, of size given, with the specified attributes
    fn map_region_general(
        physical_start: *const (),
        virtual_start: *const (),
        size: usize,
        readable: bool,
        writeable: bool,
        executable: bool,
    ) {
        for offset in (0..size).step_by(PAGE_SIZE) {
            #[allow(clippy::as_conversions)]
            let descriptor = generate_descriptor(
                physical_start.addr() + offset,
                readable,
                writeable,
                executable,
            );
            unsafe {
                *TRANSLATION_TABLE
                    .0
                    .get_mut(
                        virtual_start.byte_sub(VIRTUAL_BASE).byte_add(offset).addr() / PAGE_SIZE,
                    )
                    .unwrap() = descriptor;
            }
        }
    }

    /// Maps the given physical region to the virtual addresses shifted up by `VIRTUAL_OFFSET`
    fn map_region(
        region_start: *const (),
        region_end: *const (),
        readable: bool,
        writeable: bool,
        executable: bool,
    ) {
        map_region_general(
            region_start,
            // SAFETY: The virtual address is valid and should not overflow
            unsafe { region_start.byte_add(VIRTUAL_OFFSET) },
            // SAFETY: The range of the section should not overflow
            unsafe { region_end.byte_offset_from(region_start) }.unsigned_abs(),
            readable,
            writeable,
            executable,
        );
    }

    extern "Rust" {
        static __text_start: ();
        static __text_end: ();
        static __rodata_start: ();
        static __rodata_end: ();
        static __data_start: ();
        static __data_end: ();
        static mut __bss_start: u8;
        static __bss_end: u8;
        static __kernel_stack_start: ();
    }

    /// Addresses to write to, in order to wake up the other cores
    #[allow(clippy::as_conversions)]
    const WAKE_CORE_ADDRS: [*mut unsafe extern "C" fn() -> !; 3] =
        [0xE0 as *mut _, 0xE8 as *mut _, 0xF0 as *mut _];

    // SAFETY: This is the initialization sequence, and so the BSS is not being
    // used yet. We need to zero it out beforehand.
    unsafe {
        core::ptr::write_bytes(
            addr_of_mut!(__bss_start),
            0,
            addr_of!(__bss_end)
                .offset_from(addr_of!(__bss_start))
                .unsigned_abs(),
        );
    }

    // Map UART
    // This is temporary for debug purposes in the kernel
    unsafe {
        *TRANSLATION_TABLE.0.last_mut().unwrap() =
            generate_descriptor(0x3F20_0000, true, true, false) | (1 << 2);
    }

    // Map the kernel
    map_region(
        addr_of!(__text_start),
        addr_of!(__text_end),
        true,
        false,
        true,
    );
    map_region(
        addr_of!(__rodata_start),
        addr_of!(__rodata_end),
        true,
        false,
        false,
    );
    map_region(
        addr_of!(__data_start),
        addr_of!(__bss_end).cast(),
        true,
        true,
        false,
    );
    map_region_general(
        core::ptr::null(),
        // SAFETY: The linker script has reserved this virtual address space for kernel stacks
        unsafe { addr_of!(__kernel_stack_start).byte_add(VIRTUAL_OFFSET) },
        NUM_CORES * PAGE_SIZE,
        true,
        true,
        false,
    );

    // Wake up other cores
    for addr in WAKE_CORE_ADDRS {
        // SAFETY: These are currently valid addresses to write to in order to wake the other cores
        unsafe {
            //       *addr = _per_core_start;
        }
    }

    // Ensure all writes complete before waking up the other cores
    unsafe {
        asm!("dsb ISHST", "sev", options(nomem, nostack, preserves_flags));
    }
    // SAFETY: This is the first and only time the per-core-init will be called on this core
    unsafe {
        per_core_start_rust(PAGE_SIZE);
    }
}

/// The per-core finish of booting process
/// * Disables EL2 controls
/// * Enables EL1+0 MMU
/// * Returns into the kernel main init
/// # Safety
/// Should only be called once per core in the boot process
#[allow(clippy::as_conversions)]
unsafe extern "C" fn per_core_start_rust(sp_offset: usize) -> ! {
    extern "Rust" {
        static __kernel_stack_start: ();
    }

    // Set the stack pointer in EL1 to be the top of the given page
    let sp_el1 = // SAFETY: This is properly located in memory and not used by anything else
        unsafe { addr_of!(__kernel_stack_start).byte_add(VIRTUAL_OFFSET + sp_offset) }.addr();

    // Disable EL2 controls
    const HCR_EL2: u64 = (1 << 56) // Allow allocation tag access
        + (1 << 41) // Disables pointer authentication trapping
        + (1 << 40) 
        + (1 << 39) // Allows access to TME
        + (1 << 38) // Allows incoherency if inner and outer cacheability differ
        + (1 << 31) // EL1 is 64-bit
                    //
        + (1 << 29) // Disables HVC instruction
    ;

    // Disable EL2 timer controls
    const CNTHCTL_EL2: u64 = 0b11;
    const CNTVOFF_EL2: u64 = 0;

    // Set up the translation tables in EL1
    // TODO: Check hierarchical permissions?
    const TCR_EL1: u64 = (1 << 56) // E0PD1: EL0 access to the higher half always generates a fault
        + (1 << 52) // Disable checking the top byte of instruction pointers
        + (1 << 51) // Same as above, for EL0
        + (0xFF << 43) // HW use enabled for certain bits of the page descriptors
        + (1 << 40) // HW managed dirty bits
        + (1 << 39) // HW managed access bits
        + (1 << 38) // Disable checking the top byte of data pointers
        + (1 << 37) // Same as above, for EL0
        + (1 << 36) // 16-bit ASIDs
        + (0b11 << 30) // 64K pages in EL1
        + (0b11 << 28) // Inner-shareable memory for page walks
        + (0b11 << 26) // Outer-cacheable memory for page walks
        + (0b11 << 24) // Inner-cacheable memory for page walks
        + ((64 - (ADDRESS_BITS as u64)) << 16) // 25-bit virtual addresses
;
    const MAIR_EL1: u64 = 0xFF; // Attribute for normal memory
    #[allow(clippy::as_conversions)]
    let ttbr1_el1: u64 = addr_of!(TRANSLATION_TABLE) as u64 | 1; // Enable common translations
    const SCTLR_EL1: u64 = (1 << 60) // Disable trapping TPIDR2 accesses
                       | (0x1F << 52) // Disable trapping various memory operations

                    | (0b11 << 42) // Allow allocation tags
                    | (1 << 33) // Allow memory copy & set instructions
                    | (1 << 32) // Disable cache operations at EL0 if no write permissions
                    | (1 << 28) // Do not trap device accessess at EL0
                            | (1 << 26) // Do not trap EL0 cache operations
                            | (1 << 22) // EL1 exceptions are context synchronizing
                            | (0b101 << 16) // Do not trap WFE/WFI
                            | (0b11 << 14) // Do not trap EL0 cache operations
                            | (1 << 12) // Instruction caching
                            | (1 << 11) // Exception returns are context synchronizing
                            | (1 << 6) // If possible, disable misalignment exceptions
                            | (1 << 2) // Data cachinga
                            | 1 // Enable virtual memory
    ;

    // Prepare to return into the kernel main process
    #[allow(clippy::as_conversions)]
    #[allow(clippy::fn_to_numeric_cast_any)]
    //ELR_EL2.set(usize_to_u64(kernel::init as usize + VIRTUAL_OFFSET));
    let elr_el2 = crate::init as usize + VIRTUAL_OFFSET;
    const SPSR_EL2: u64 = (0b1111 << 6) | 0b0101; // Disable interrupts, switch to SP_EL1 stack
                                                  // pointer

    // SAFETY: Clearing the FP/LR is safe because this function never returns
    // and we have set up everything for a proper `eret`, which should be
    // interpreted by the main kernel as the true start of the call stack
    unsafe {
        asm!(
            "msr CNTHCTL_EL2, {}",
            "msr CNTVOFF_EL2, {}",
            "msr ELR_EL2, {}",
            "msr HCR_EL2, {}",
            "msr MAIR_EL1, {}",
            "msr TCR_EL1, {}",
            "msr TTBR1_EL1, {}",
            "msr SCTLR_EL1, {}",
            "msr SP_EL1, {}",
            "msr SPSR_EL2, {}",
            "mov FP, #0",
            "mov LR, #0",
            "eret",
            in(reg) CNTHCTL_EL2,
            in(reg) CNTVOFF_EL2,
            in(reg) elr_el2,
            in(reg) HCR_EL2,
            in(reg) MAIR_EL1,
            in(reg) TCR_EL1,
            in(reg) ttbr1_el1,
            in(reg) SCTLR_EL1,
            in(reg) sp_el1,
            in(reg) SPSR_EL2,
            options(nomem, nostack, noreturn)
        )
    }
}

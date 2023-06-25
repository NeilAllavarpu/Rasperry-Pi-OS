use crate::memory_layout::{FS_ELF, STACKS};
use core::arch::aarch64::ISHST;
use core::arch::{aarch64, asm};
use core::cell::{SyncUnsafeCell, UnsafeCell};
use core::mem::MaybeUninit;
use core::num::NonZeroUsize;
use core::ptr::{self, addr_of, addr_of_mut};
use core::sync::atomic::{self, AtomicPtr, AtomicU16, AtomicUsize};
use core::sync::atomic::{AtomicBool, Ordering};

/// Number of cores
pub const NUM_CORES: usize = 4;

/// Physical address that the kernel is loaded to
const PHYSICAL_LOAD_ADDR: usize = 0x8_0000;
/// Virtual address that the kernel is linked to
const VIRTUAL_LOAD_ADDR: usize = 0xFFFF_FFFF_FE00_0000;
/// Offset between the virtual and physical addresses
const VIRTUAL_OFFSET: usize = VIRTUAL_LOAD_ADDR - PHYSICAL_LOAD_ADDR;

const PAGE_SIZE: usize = 64 * 1024;
const PAGE_SIZE_BITS: u32 = PAGE_SIZE.ilog2();
const ADDRESS_BITS: u32 = 25;
const VIRTUAL_BASE: usize = 0xFFFF_FFFF_FE00_0000;

#[repr(C)]
#[repr(align(4096))]
#[allow(clippy::as_conversions)]
pub(crate) struct TranslationTable([u64; 1 << (ADDRESS_BITS - PAGE_SIZE_BITS) as usize]);
pub(crate) static mut TRANSLATION_TABLE: TranslationTable = TranslationTable([0; _]);

static CORE_COUNT: AtomicU16 = AtomicU16::new(1);
static mut STACK_SIZE: MaybeUninit<usize> = MaybeUninit::uninit();

/// The entry point of the kernel
/// * Clears the BSS
/// * Sets up the kernel page table
/// * Wakes up the other cores
/// # Safety
/// Should never be called manually, only by the bootloader
#[no_mangle]
#[naked]
#[link_section = ".init"]
unsafe extern "C" fn _start() -> ! {
    // SAFETY: We need to use this assembly to set a stack pointer
    unsafe {
        asm!(
            "msr DAIFSET, #0b1111", // First, disable interrupts
            "adr x0, __bss_end",
            "add sp, x0, #0x800",
            "b {start_rust}", // Perform the main initialization; this should never return
            start_rust = sym start_rust,
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
            "adr x0, {COUNTER}",    // Atomically increment the core counter
            "0: ldxrh w1, [x0]",    // The desired index (ID + 1) is held in `w1`
            "add w1, w1, #1",
            "stxrh w2, w1, [x0]",
            "cbnz w2, 0b",
            "ldr x0, {STACK_SIZE}", // Load the configured stack size
            "adr x2, __bss_end",    // Load the offset of the stacks, in physical memory
            "add x2, x2, #15",      // Round the offset up to the nearest multiple of 16, for
                                    // alignment
            "and x2, x2, #{ALIGN_MASK}",
            "madd x0, x0, x1, x2",  // Compute the top of this core's stack
            "mov sp, x0",           // Set the sp
            "b {per_core_start_rust}", // Perform the remaining initialization; this should never return
           STACK_SIZE = sym STACK_SIZE,
            COUNTER = sym CORE_COUNT,
            per_core_start_rust = sym per_core_start_rust,
            ALIGN_MASK = const !0xF_u64,
            options(noreturn)
        )
    }
}

const fn generate_descriptor(
    target: u64,
    _readable: bool,
    writeable: bool,
    executable: bool,
) -> u64 {
    (1 << 54) // Unprivileged execute-never
        | (((!executable) as u64) << 53) // Privileged execute-never
        | (target & !(PAGE_SIZE as u64 - 1)) // Phyiscal address
        | (1 << 10) // Access flag
        | (0b11 << 8) // Shareability
        | (((!writeable) as u64) << 7) // Not writeable
        | 0b11 // Valid entry
}

/// Maps the contiguous physical region starting at the given place to the given contiguous virtual address, of size given, with the specified attributes
fn map_region_general(
    physical_start: u64,
    virtual_start: *const (),
    size: usize,
    readable: bool,
    writeable: bool,
    executable: bool,
) {
    for offset in (0..=size).step_by(PAGE_SIZE) {
        #[allow(clippy::as_conversions)]
        let descriptor = generate_descriptor(
            physical_start + u64::try_from(offset).unwrap(),
            readable,
            writeable,
            executable,
        );
        unsafe {
            *TRANSLATION_TABLE
                .0
                .get_mut(
                    virtual_start
                        .mask(!(PAGE_SIZE - 1))
                        .byte_sub(VIRTUAL_BASE)
                        .byte_add(offset)
                        .addr()
                        / PAGE_SIZE,
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
        region_start.addr().try_into().unwrap(),
        // SAFETY: The virtual address is valid and should not overflow
        unsafe { region_start.byte_add(VIRTUAL_OFFSET) },
        // SAFETY: The range of the section should not overflow
        unsafe { region_end.byte_offset_from(region_start) }.unsigned_abs(),
        readable,
        writeable,
        executable,
    );
}

/// The (almost) initial boot code for the kernel;
/// runs on the initial core only
/// # Safety
/// Should only be called once, in the boot process
unsafe extern "C" fn start_rust() -> ! {
    extern "Rust" {
        static __text_start: ();
        static __elf_start: u32;
        static mut __bss_start: u8;
        static __bss_end: u8;
    }

    /// Addresses to write to, in order to wake up the other cores
    const WAKE_CORE_ADDRS: [usize; 3] = [0xE0, 0xE8, 0xF0];

    // TODO: compute this somehow
    let stack_size = 0x1000;
    // SAFETY: This is the only currently running code, so no other accesses to this static exist
    unsafe {
        STACK_SIZE.write(stack_size);
    }

    // SAFETY: Taking the address of a static is always safe
    let bss_start_addr = unsafe { addr_of_mut!(__bss_start) };

    // Figure out the size of ELF in memory
    // SAFETY: The build system should place a u32 indicating the size of the FS elf
    let fs_elf_size = unsafe { ptr::addr_of!(__elf_start).read_unaligned() }.try_into();
    // SAFETY: `usize` is 64-bit, so a `u32` always fits.
    let fs_elf_size = unsafe { fs_elf_size.unwrap_unchecked() };

    // SAFETY: No one else can be accessing statics at this time
    unsafe {
        FS_ELF.size = #[expect(
            clippy::expect_used,
            reason = "Panics are the only possible error handling at this point"
        )]
        NonZeroUsize::new(fs_elf_size)
            .expect("File system ELF should have nonzero size");
    }

    // SAFETY:
    // * These pointers represent the start and end of the BSS
    // * These pointers are aligned to 16 bytes, so their difference is a multiple of 16 bytes
    // * The difference cannot overflow an `isize` since it fits into a 25 bit address space
    // * The difference does not involve any wrapping around
    let bss_size = unsafe { addr_of!(__bss_end).offset_from(bss_start_addr) }.unsigned_abs();
    // SAFETY: The BSS is valid for writes, and the start is aligned to 16 bytes as per the linker
    // script
    unsafe {
        bss_start_addr.write_bytes(0, bss_size);
    };

    map_region_general(
        0x3F20_0000,
        0xFFFF_FFFF_FF00_0000 as *const (),
        0x2000,
        true,
        true,
        false,
    );

    // Map the kernel
    map_region(
        addr_of!(__text_start),
        addr_of!(__bss_end).cast(),
        true,
        true,
        true,
    );

    map_region_general(
        unsafe { FS_ELF.pa },
        unsafe { FS_ELF.va }.as_ptr(),
        fs_elf_size,
        true,
        true,
        false,
    );

    // Make sure translation table + other globals are written before setting wakeup addresses
    // SAFETY: Data memory barriers are defined on the Raspberry Pi
    unsafe {
        aarch64::__dmb(ISHST);
    };

    // Wake up other cores

    for addr in WAKE_CORE_ADDRS {
        /*#[expect(
            clippy::as_conversions,
            reason = "Unable to cast a function pointer to a pointer or usize otherwise"
        )]
        #[expect(
            clippy::fn_to_numeric_cast_any,
            reason = "Intentional function pointer cast"
        )]*/
        // SAFETY: These are currently valid addresses to write to in order to wake the other
        // cores. and are properly aligned + unaccessed to otherwise
        // unsafe { AtomicUsize::from_ptr(ptr::from_exposed_addr_mut(addr)) }
        //   .store(_per_core_start as usize, Ordering::Relaxed);
    }

    // Ensure all writes complete before waking up the other cores
    // SAFETY: Data synchronization barriers are defined on the Raspberry Pi
    unsafe {
        aarch64::__dsb(ISHST);
    }

    // SAFETY: SEV is defined on the Raspberry Pi
    unsafe {
        // aarch64::__sev();
    }

    // SAFETY: This is the first and only time the per-core-init will be called on this core
    unsafe {
        per_core_start_rust(
            addr_of!(__bss_end)
                .map_addr(|addr| addr.saturating_add(stack_size))
                .addr(),
        );
    }
}

/// The per-core finish of booting process
/// * Disables EL2 controls
/// * Enables EL1+0 MMU
/// * Returns into the kernel main init
/// # Safety
/// Should only be called once per core in the boot process
#[allow(clippy::as_conversions)]
unsafe extern "C" fn per_core_start_rust(sp_physical: usize) -> ! {
    // Set the stack pointer in EL1 to be the top of the given page
    let sp_el1 = VIRTUAL_OFFSET + sp_physical;

    // Disable EL2 controls
    const HCR_EL2: u64 = (1 << 56) // Allow allocation tag access
        + (1 << 41) // Disables pointer authentication trapping
        + (1 << 40) // Same as above
        + (1 << 39) // Allows access to TME
        + (1 << 38) // Allows incoherency if inner and outer cacheability differ
        + (1 << 31) // EL1 is 64-bit
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
        + (0b01 << 14) // 64K pages in EL1
        + (0b11 << 12) // Inner-shareable memory for page walks
        + (0b11 << 10) // Outer-cacheable memory for page walks
        + (0b11 << 8) // Inner-cacheable memory for page walks
        + ((64 - (ADDRESS_BITS as u64)) << 0) // 25-bit virtual addresses
;
    const MAIR_EL1: u64 = 0xFF; // Attribute for normal memory
    #[allow(clippy::as_conversions)]
    let ttbr1_el1 = addr_of!(TRANSLATION_TABLE).addr() | 1; // Enable common translations
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
                            | (1 << 2) // Data caching
                            | 1           // Enable virtual memory
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

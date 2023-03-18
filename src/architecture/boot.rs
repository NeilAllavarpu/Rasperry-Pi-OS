use crate::{
    architecture::usize_to_u64,
    kernel,
    memory::{
        base_attributes_global,
        kernel::{KERNEL_TABLE, PAGE_SIZE, PAGE_SIZE_LOG},
        read_only_attributes, valid_attributes, writeable_attributes, PageDescriptorAttributes,
        Ppn, Vpn,
    },
    sync::Mutex,
};
use aarch64_cpu::{
    asm::{barrier, sev},
    registers::{
        CNTHCTL_EL2, CNTVOFF_EL2, ELR_EL2, HCR_EL2, MAIR_EL1, SCTLR_EL1, SPSR_EL2, SP_EL1, TCR_EL1,
        TTBR1_EL1,
    },
};
use core::{
    arch::asm,
    ptr::{addr_of, addr_of_mut},
};
use tock_registers::interfaces::{ReadWriteable, Writeable};

/// Number of cores
const NUM_CORES: usize = 4;

/// Physical address that the kernel is loaded to
const PHYSICAL_LOAD_ADDR: usize = 0x8_0000;
/// Virtual address that the kernel is linked to
const VIRTUAL_LOAD_ADDR: usize = 0xFFFF_FFFF_FE08_0000;
/// Offset between the virtual and physical addresses
const VIRTUAL_OFFSET: usize = VIRTUAL_LOAD_ADDR - PHYSICAL_LOAD_ADDR;

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
            "lsl x0, x0, {PAGE_SIZE_LOG}", // Scale the index by the page size
            "mov sp, x0", // Set the sp
            "b {per_core_start_rust}", // Perform the remaining initialization; this should never return
            PAGE_SIZE_LOG = const PAGE_SIZE_LOG,
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
    /// Maps the contiguous physical region starting at the given place to the given contiguous virtual address, of size given, with the specified attributes
    fn map_region_general(
        physical_start: *const (),
        virtual_start: *const (),
        size: usize,
        attributes: PageDescriptorAttributes,
    ) {
        for offset in (0..size).step_by(PAGE_SIZE) {
            let vpn = Vpn::from_addr(virtual_start.addr() + offset);
            KERNEL_TABLE
                .lock()
                .get_entry(vpn)
                .expect("Address of virtualized mapping should be valid")
                .set(
                    Ppn::from_addr(physical_start.addr() + offset),
                    base_attributes_global() + valid_attributes() + attributes,
                );
        }
    }

    /// Maps the given physical region to the virtual addresses shifted up by `VIRTUAL_OFFSET`
    fn map_region(
        region_start: *const (),
        region_end: *const (),
        attributes: PageDescriptorAttributes,
    ) {
        map_region_general(
            region_start,
            // SAFETY: The virtual address is valid and should not overflow
            unsafe { region_start.byte_add(VIRTUAL_OFFSET) },
            // SAFETY: The range of the section should not overflow
            unsafe { region_end.byte_offset_from(region_start) }.unsigned_abs(),
            attributes,
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

    // Map the kernel
    map_region(
        addr_of!(__text_start),
        addr_of!(__text_end),
        read_only_attributes(),
    );
    map_region(
        addr_of!(__rodata_start),
        addr_of!(__rodata_end),
        read_only_attributes(),
    );
    map_region(
        addr_of!(__data_start),
        addr_of!(__bss_end).cast(),
        writeable_attributes(),
    );
    map_region_general(
        core::ptr::null(),
        // SAFETY: The linker script has reserved this virtual address space for kernel stacks
        unsafe { addr_of!(__kernel_stack_start).byte_add(VIRTUAL_OFFSET) },
        NUM_CORES * PAGE_SIZE,
        writeable_attributes(),
    );

    // Wake up other cores
    for addr in WAKE_CORE_ADDRS {
        // SAFETY: These are currently valid addresses to write to in order to wake the other cores
        unsafe {
            *addr = _per_core_start;
        }
    }

    // Ensure all writes complete before waking up the other cores
    barrier::dsb(barrier::OSHST);
    sev();
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
unsafe extern "C" fn per_core_start_rust(sp_offset: usize) -> ! {
    extern "Rust" {
        static __kernel_stack_start: ();
    }

    // Set the stack pointer in EL1 to be the top of the given page
    SP_EL1.set(usize_to_u64(
        // SAFETY: This is properly located in memory and not used by anything else
        unsafe { addr_of!(__kernel_stack_start).byte_add(VIRTUAL_OFFSET + sp_offset) }.addr(),
    ));

    // Disable EL2 controls
    HCR_EL2.set((1 << 56) + (1 << 39) + (1 << 38) + (1 << 29)); // Allow allocation tag access
                                                                // Allows access to TME
                                                                // Allows incoherency if inner and outer cacheability differ
                                                                // Disables HVC instruction
    HCR_EL2.modify(
        HCR_EL2::API::DisableTrapPointerAuthInstToEl2
            + HCR_EL2::APK::DisableTrapPointerAuthKeyRegsToEl2
            + HCR_EL2::TEA::CLEAR
            + HCR_EL2::E2H::DisableOsAtEl2
            + HCR_EL2::RW::EL1IsAarch64
            + HCR_EL2::FWB::Disabled
            + HCR_EL2::TGE::DisableTrapGeneralExceptionsToEl2
            + HCR_EL2::DC::CLEAR
            + HCR_EL2::AMO::CLEAR
            + HCR_EL2::IMO::DisableVirtualIRQ
            + HCR_EL2::FMO::DisableVirtualFIQ
            + HCR_EL2::VM::Disable,
    );

    // Disable EL2 timer controls
    CNTHCTL_EL2.write(CNTHCTL_EL2::EL1PCEN::SET + CNTHCTL_EL2::EL1PCTEN::SET);
    CNTVOFF_EL2.set(0);

    // Set up the translation tables in EL1
    // TODO: Check hierarchical permissions?
    TCR_EL1.set((1 << 56) + (0xFF << 43)); // E0PD1: EL0 access to the higher half always generates a fault
                                           // HW use enabled for certain bits of the page descriptors
    TCR_EL1.modify(
        TCR_EL1::EPD0::DisableTTBR0Walks
            + TCR_EL1::TBID1::CLEAR
            + TCR_EL1::TBID0::CLEAR
            + TCR_EL1::HD::Enable
            + TCR_EL1::HA::Enable
            + TCR_EL1::TBI1::Ignored
            + TCR_EL1::TBI0::Ignored
            + TCR_EL1::AS::ASID16Bits
            + TCR_EL1::IPS::Bits_36
            + TCR_EL1::TG1::KiB_64
            + TCR_EL1::SH1::Outer
            + TCR_EL1::ORGN1::WriteBack_ReadAlloc_WriteAlloc_Cacheable
            + TCR_EL1::IRGN1::WriteBack_ReadAlloc_WriteAlloc_Cacheable
            + TCR_EL1::T1SZ.val(39)
            + TCR_EL1::A1::TTBR0
            + TCR_EL1::EPD0::DisableTTBR0Walks,
    );
    TTBR1_EL1.write(
        TTBR1_EL1::BADDR.val(usize_to_u64(addr_of!(*KERNEL_TABLE.lock()).addr()) >> 1)
            + TTBR1_EL1::CnP::SET,
    );

    MAIR_EL1.write(
        MAIR_EL1::Attr0_Normal_Inner::WriteBack_Transient_ReadWriteAlloc
            + MAIR_EL1::Attr0_Normal_Outer::WriteBack_Transient_ReadWriteAlloc,
    );

    SCTLR_EL1.write(
        SCTLR_EL1::A::Enable
            + SCTLR_EL1::C::Cacheable
            + SCTLR_EL1::DZE::DontTrap
            + SCTLR_EL1::EE::LittleEndian
            + SCTLR_EL1::I::Cacheable
            + SCTLR_EL1::M::Enable
            + SCTLR_EL1::NAA::Disable
            + SCTLR_EL1::NTWE::DontTrap
            + SCTLR_EL1::NTWI::DontTrap
            + SCTLR_EL1::SA::Enable
            + SCTLR_EL1::SA0::Enable
            + SCTLR_EL1::UCI::DontTrap
            + SCTLR_EL1::UCT::DontTrap
            + SCTLR_EL1::UMA::Trap
            + SCTLR_EL1::WXN::Disable,
    );

    // Prepare to return into the kernel main process
    #[allow(clippy::as_conversions)]
    #[allow(clippy::fn_to_numeric_cast_any)]
    ELR_EL2.set(usize_to_u64(kernel::init as usize + VIRTUAL_OFFSET));

    SPSR_EL2.write(
        SPSR_EL2::D::Masked
            + SPSR_EL2::A::Masked
            + SPSR_EL2::I::Masked
            + SPSR_EL2::F::Masked
            + SPSR_EL2::M::EL1h,
    );

    // This needs to be inlined so that any function calls that may occur
    // don't clobber this
    // SAFETY: Clearing the FP/LR is safe because this function never returns
    // and we have set up everything for a proper `eret`, which should be
    // interpreted by the main kernel as the true start of the call stack
    unsafe {
        asm!(
            "mov FP, #0",
            "mov LR, #0",
            "eret",
            options(nomem, nostack, noreturn)
        );
    }
}

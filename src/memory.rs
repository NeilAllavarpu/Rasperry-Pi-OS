use crate::{
    board::MMIO_MAPPINGS,
    sync::{BlockingLock, Mutex, SpinLock},
};
use aarch64_cpu::{
    asm::barrier,
    registers::{MAIR_EL1, SCTLR_EL1, TCR_EL1, TTBR0_EL1},
};
use core::ops::Deref;
use tock_registers::{
    fields::FieldValue,
    interfaces::{ReadWriteable, Readable, Writeable},
    register_bitfields,
    registers::InMemoryRegister,
};

#[cfg(not(target_pointer_width = "64"))]
compile_error!("MMU for non-64 bit targets is not supported");

/// Size of kernel granules
const GRANULE_SIZE: usize = 1 << 16;

// A level 3 page descriptor, as per ARMv8-A Architecture Reference Manual Figure D5-17. (64 KiB Granule)
register_bitfields! {usize,
    PAGE_DESCRIPTOR [
        /// Unprivileged execute-never.
        UXN OFFSET(54) NUMBITS(1) [],
        /// Privileged execute-never.
        PXN OFFSET(53) NUMBITS(1) [],
        /// A hint bit indicating that the translation table entry is one of a contiguous set of entries, that might be cached in a single TLB entry.
        CONTIGUOUS OFFSET(52) NUMBITS(1) [],
        /// Dirty Bit Modifier
        DIRTY OFFSET(51) NUMBITS(1) [],
        /// Physical address of the next table descriptor (lvl2) or the page descriptor (lvl3).
        OUTPUT_ADDRESS OFFSET(16) NUMBITS(32) [], // [47:16]
        /// If a lookup using this descriptor is cached in a TLB, determines whether the TLB entry applies to all ASID values, or only to the current ASID value
        NOT_GLOBAL OFFSET(11) NUMBITS(1) [],
        /// Access flag.
        AF OFFSET(10) NUMBITS(1) [],
        /// Shareability field.
        SH OFFSET(8) NUMBITS(2) [
            None = 0b00,
            Outer = 0b10,
            Inner = 0b11
        ],
        /// Whether or not writes are permitted to this region
        NOT_WRITEABLE OFFSET(7) NUMBITS(1) [],
        /// Whether or not EL0 can access this region
        EL0_ACCESSIBLE OFFSET(6) NUMBITS(1) [],
        /// Memory attributes index into the MAIR_EL1 register.
        AttrIndx OFFSET(2) NUMBITS(3) [],
        /// Descriptor type
        TYPE OFFSET(1) NUMBITS(1) [
            /// Behaves identically to encodings with bit[0] set to 0.
            /// This encoding must not be used in level 3 translation tables.
            Reserved_Invalid = 0,
            /// Gives the address and attributes of a 4KB, 16KB, or 64KB page of memory.
            Page = 1
        ],
        /// If a lookup returns an invalid descriptor, the associated input address is unmapped, and any attempt to access it generates a Translation fault.
        VALID OFFSET(0) NUMBITS(1) []
    ]
}

/// Represents a page descriptor in the level 3 translation table (64 KiB granules)
#[repr(transparent)]
struct PageDescriptor(<Self as Deref>::Target);

impl PageDescriptor {
    /// Generates and validates an address field corresponding to the input address
    fn addr(address: usize) -> FieldValue<usize, PAGE_DESCRIPTOR::Register> {
        assert_eq!(address % GRANULE_SIZE, 0);

        let shifted = address >> 16;
        assert!(shifted < (1 << 16));
        PAGE_DESCRIPTOR::OUTPUT_ADDRESS.val(shifted)
    }

    /// Sets the descriptor to be valid, pointing to the given granule
    fn set_valid(&mut self, address: usize) {
        self.write(
            PAGE_DESCRIPTOR::UXN::CLEAR
                + PAGE_DESCRIPTOR::PXN::CLEAR
                + PAGE_DESCRIPTOR::CONTIGUOUS::CLEAR
                + Self::addr(address)
                + PAGE_DESCRIPTOR::NOT_GLOBAL::CLEAR
                + PAGE_DESCRIPTOR::SH::Outer
                + PAGE_DESCRIPTOR::NOT_WRITEABLE::CLEAR
                + PAGE_DESCRIPTOR::EL0_ACCESSIBLE::CLEAR
                + PAGE_DESCRIPTOR::AttrIndx.val(0)
                + PAGE_DESCRIPTOR::TYPE::Page
                + PAGE_DESCRIPTOR::AF::SET
                + PAGE_DESCRIPTOR::VALID::SET,
        );
    }
}

impl const Deref for PageDescriptor {
    type Target = InMemoryRegister<usize, PAGE_DESCRIPTOR::Register>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[repr(C, align(4096))]
/// The kernel translation table
struct TranslationTable {
    /// The actual descriptors
    descriptors: [PageDescriptor; 1 << 9],
}

/// The global translation table for the kernel address space
static KERNEL_TABLE: BlockingLock<TranslationTable> = BlockingLock::new(TranslationTable {
    descriptors: [const { PageDescriptor(InMemoryRegister::new(0)) }; _],
});

/// Sets up the basic kernel tables
pub fn init() {
    let mut table = KERNEL_TABLE.lock();

    for (n, entry) in table.descriptors[..32].iter_mut().enumerate() {
        entry.set_valid(n * GRANULE_SIZE);
    }

    for mapping in MMIO_MAPPINGS.values() {
        table.descriptors[mapping.virtual_addr / GRANULE_SIZE].set_valid(mapping.physical_addr);
        table.descriptors[mapping.virtual_addr / GRANULE_SIZE]
            .modify(PAGE_DESCRIPTOR::AttrIndx.val(1));
    }
}

/// Enables the MMU on each core
pub fn per_core_init() {
    /// Unprivileged access to any address translated by `TTBR1_EL1` will generate a level 0 Translation fault.
    const EOPD1_BIT: u8 = 56;
    {
        /// Prevents blocking from occurring during initialization
        static MUTEX: SpinLock<()> = SpinLock::new(());
        let _guard = MUTEX.lock();

        TTBR0_EL1.set_baddr(
            KERNEL_TABLE
                .lock()
                .descriptors
                .as_ptr()
                .to_bits()
                .try_into()
                .expect("Pointers should be 64 bits"),
        );
    }
    TTBR0_EL1.modify(TTBR0_EL1::CnP::SET);

    let mut tcr = TCR_EL1.extract();
    tcr.set(1 << EOPD1_BIT);
    TCR_EL1.modify_no_read(
        tcr,
        // DS implicitly disabled
        // TODO: Implement MTE
        // E0PD implicitly disabled
        // TODO: Check SVE, TME
        TCR_EL1::TBID1::SET
            + TCR_EL1::TBID0::SET
        // Hierarchical permissions implicitly enabled
            + TCR_EL1::HD::Enable
            + TCR_EL1::HA::Enable
            + TCR_EL1::TBI1::Used
            + TCR_EL1::TBI0::Used
            + TCR_EL1::AS::ASID16Bits
            + TCR_EL1::IPS::Bits_36 // 64GB
            + TCR_EL1::A1::TTBR0
            + TCR_EL1::TG0::KiB_64
            + TCR_EL1::SH0::Inner
            + TCR_EL1::ORGN0::WriteBack_ReadAlloc_WriteAlloc_Cacheable
            + TCR_EL1::IRGN0::WriteBack_ReadAlloc_WriteAlloc_Cacheable
            + TCR_EL1::T0SZ.val(39),
    );

    MAIR_EL1.write(
        MAIR_EL1::Attr0_Normal_Outer::WriteBack_NonTransient_ReadWriteAlloc
            + MAIR_EL1::Attr1_Device::nonGathering_nonReordering_noEarlyWriteAck,
    );
    barrier::isb(barrier::SY);
    SCTLR_EL1.modify(SCTLR_EL1::C::Cacheable + SCTLR_EL1::I::Cacheable + SCTLR_EL1::M::Enable);
    barrier::isb(barrier::SY);
}

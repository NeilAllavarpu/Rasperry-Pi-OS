use core::iter::Step;
use tock_registers::{
    fields::FieldValue, interfaces::Writeable, register_bitfields, registers::InMemoryRegister,
};

#[cfg(not(target_pointer_width = "64"))]
compile_error!("MMU for non-64 bit targets is not supported");

/// The global kernel address space
pub mod kernel;

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
        /// Physical address of the page
        OUTPUT_ADDRESS OFFSET(12) NUMBITS(36) [],
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

/// Generates a typed integer for representing pages
macro_rules! typed_page {
    ($label: ident) => {
        #[derive(Clone, Copy, PartialEq, PartialOrd)]
        pub struct $label<const LOG_GRANULE_SIZE: u8>(pub usize);

        impl<const LOG_GRANULE_SIZE: u8> $label<LOG_GRANULE_SIZE> {
            pub const fn addr(&self) -> usize {
                usize::from(self.0) << LOG_GRANULE_SIZE
            }

            pub const fn from_addr(addr: usize) -> Self {
                Self(addr >> LOG_GRANULE_SIZE)
            }
        }

        impl<const __: u8> Step for $label<__> {
            fn steps_between(start: &Self, end: &Self) -> Option<usize> {
                return end.0.checked_sub(start.0);
            }

            fn forward_checked(start: Self, count: usize) -> Option<Self> {
                return start.0.checked_add(start.0 - count).map(Self);
            }

            fn backward_checked(start: Self, count: usize) -> Option<Self> {
                return start.0.checked_sub(start.0 - count).map(Self);
            }
        }
    };
}

typed_page!(Ppn);
typed_page!(Vpn);

/// Represents a page descriptor in the level 3 translation table (64 KiB granules)
#[repr(transparent)]
pub struct PageDescriptor<const LOG_GRANULE_SIZE: u8>(
    InMemoryRegister<usize, PAGE_DESCRIPTOR::Register>,
);

/// Attributes for a page descriptor
pub type PageDescriptorAttributes = FieldValue<usize, PAGE_DESCRIPTOR::Register>;

/// Base attributes that every valid page descriptor must have
pub fn base_attributes() -> PageDescriptorAttributes {
    PAGE_DESCRIPTOR::UXN::CLEAR
        + PAGE_DESCRIPTOR::PXN::CLEAR
        + PAGE_DESCRIPTOR::CONTIGUOUS::CLEAR
        + PAGE_DESCRIPTOR::NOT_GLOBAL::SET
        + PAGE_DESCRIPTOR::SH::Outer
        + PAGE_DESCRIPTOR::EL0_ACCESSIBLE::CLEAR
        + PAGE_DESCRIPTOR::AttrIndx.val(0)
        + PAGE_DESCRIPTOR::TYPE::Page
}

/// Base attributes that every valid page descriptor must have, as well as marking this as a global page
pub fn base_attributes_global() -> PageDescriptorAttributes {
    PAGE_DESCRIPTOR::UXN::CLEAR
        + PAGE_DESCRIPTOR::PXN::CLEAR
        + PAGE_DESCRIPTOR::CONTIGUOUS::CLEAR
        + PAGE_DESCRIPTOR::NOT_GLOBAL::CLEAR
        + PAGE_DESCRIPTOR::SH::Outer
        + PAGE_DESCRIPTOR::EL0_ACCESSIBLE::CLEAR
        + PAGE_DESCRIPTOR::AttrIndx.val(0)
        + PAGE_DESCRIPTOR::TYPE::Page
}

/// Sets a page descriptor to be valid
pub fn valid_attributes() -> PageDescriptorAttributes {
    PAGE_DESCRIPTOR::AF::SET + PAGE_DESCRIPTOR::VALID::SET
}

/// Sets a page descriptor to be invalid
pub const fn invalid_attributes() -> PageDescriptorAttributes {
    PAGE_DESCRIPTOR::VALID::CLEAR
}

/// Sets a page descriptor to be read-write
pub const fn writeable_attributes() -> PageDescriptorAttributes {
    PAGE_DESCRIPTOR::NOT_WRITEABLE::CLEAR
}

/// Sets a page descriptor to be read-only
pub const fn read_only_attributes() -> PageDescriptorAttributes {
    PAGE_DESCRIPTOR::NOT_WRITEABLE::SET
}

impl<const LOG_GRANULE_SIZE: u8> PageDescriptor<LOG_GRANULE_SIZE> {
    /// Generates and validates an address field corresponding to the input address
    pub const fn addr_attributes(ppn: Ppn<LOG_GRANULE_SIZE>) -> PageDescriptorAttributes {
        PAGE_DESCRIPTOR::OUTPUT_ADDRESS.val(ppn.addr() >> 12)
    }

    /// Sets the descriptor to be valid, pointing to the given granule
    pub fn set(&mut self, ppn: Ppn<LOG_GRANULE_SIZE>, attributes: PageDescriptorAttributes) {
        self.0.write(attributes + Self::addr_attributes(ppn));
    }
}

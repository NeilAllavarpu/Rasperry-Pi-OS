//! Virtual memory management for the kernel itself

use bitfield_struct::bitfield;
use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::{FromPrimitive, ToPrimitive};
use stdos::sync::SpinLock;

/// Memory attributes describing a memory region
#[derive(FromPrimitive, ToPrimitive)]
enum MemoryAttribute {
    Normal = 0,
    Device = 1,
}

impl From<u64> for MemoryAttribute {
    fn from(value: u64) -> Self {
        #[expect(
            clippy::expect_used,
            reason = "This implementation is necessary for bitfield derivation"
        )]
        FromPrimitive::from_u64(value).expect("Invalid memory attribute provided")
    }
}

impl From<MemoryAttribute> for u64 {
    #[inline]
    fn from(value: MemoryAttribute) -> Self {
        // SAFETY: `MemoryAttribute` can always fit into a `u64`
        unsafe { ToPrimitive::to_u64(&value).unwrap_unchecked() }
    }
}

/// Shareability attributes describing a memory region
#[derive(FromPrimitive, ToPrimitive)]
enum Shareability {
    Non = 0b00,
    Outer = 0b10,
    Inner = 0b11,
}

impl From<u64> for Shareability {
    fn from(value: u64) -> Self {
        #[expect(
            clippy::expect_used,
            reason = "This implementation is necessary for bitfield derivation"
        )]
        FromPrimitive::from_u64(value).expect("Invalid shareability attribute provided")
    }
}

impl From<Shareability> for u64 {
    #[inline]
    fn from(value: Shareability) -> Self {
        // SAFETY: `Shareability` can always fit into a `u64`
        unsafe { ToPrimitive::to_u64(&value).unwrap_unchecked() }
    }
}

#[bitfield(u64, debug = false)]
pub struct TranslationDescriptor {
    /// Whether or not this descriptor is valid
    valid: bool,
    /// Must be 1 whenever the descriptor is valid
    res1: bool,
    /// Attributes for this memory region. Used to index into `MAIR_EL1`
    #[bits(3)]
    memory_type: MemoryAttribute,
    _ns: bool,
    /// Whether or not EL0 can access this entry. Always 0 for kernel mappings
    _el0_accessible: bool,
    /// Disables writes to this mapping.
    writeable_never: bool,
    /// Shareability for this memory region
    #[bits(2)]
    shareability: Shareability,
    /// Whether or not this entry has been accessed. Triggers a page fault if this bit is not set
    /// and this descriptor is used.
    access: bool,
    /// If set, this entry is ASID specific. Should be 0 for kernel mappings
    global: bool,
    #[bits(4)]
    _res0: u8,
    #[bits(32)]
    ppn: u64,
    #[bits(2)]
    _res0_2: u8,
    _guarded_page: bool,
    dirty: bool,
    /// Whether or not the nearby mappings map a contiguous range of physical memory, allowing for
    /// TLB caching optimizations
    _contiguous: bool,
    /// Whether or not EL1 can execute in this mapping
    privileged_execute_never: bool,
    /// Whether or not EL0 can execute instructions in this mapping
    unprivileged_execute_never: bool,
    #[bits(4)]
    _ignored2: u8,
    #[bits(4)]
    _hw_use: u8,
    _ignored: bool,
}

impl TranslationDescriptor {
    /// Generates a descriptor with appropriate defaults filled out, and minimal permissions
    ///
    /// Returns `None` if `pa` is too large
    fn valid_base(pa: u64) -> Option<Self> {
        (pa < 0x10_0000_0000_0000).then_some(
            Self::new()
                .with_valid(true)
                .with_res1(true)
                .with_memory_type(MemoryAttribute::Normal)
                .with_shareability(Shareability::Inner)
                .with_access(true)
                .with_ppn(pa >> 16)
                .with_unprivileged_execute_never(true),
        )
    }
}

/// Page size for the kernel
const PAGE_SIZE: usize = 1 << 16;
/// Number of bits in the page offset
const PAGE_SIZE_BITS: u32 = PAGE_SIZE.ilog2();
/// Number of usable bits in virtual addresses
const ADDRESS_BITS: u32 = 25;

/// Kernel translation table struct
#[repr(C)]
#[repr(align(4096))]
#[expect(
    clippy::as_conversions,
    reason = "Necessary for const conversion to the appropriate type"
)]
struct TranslationTable([TranslationDescriptor; 1 << (ADDRESS_BITS - PAGE_SIZE_BITS) as usize]);

/// A wrapper to manage the kernel's address space
pub struct AddressSpace {
    /// The underlying translation table that this address space manages
    table: &'static mut TranslationTable,
}

impl AddressSpace {
    /// Creates a new address space using the specified translation table
    const fn new(table: &'static mut TranslationTable) -> Self {
        Self { table }
    }

    /// Maps the given virtual address range to the given physical address range, with the
    /// specified attributes. Overrides any existing mappings for that region.
    ///
    /// Note that, while not unsafe, if the physical range is not owned by the appropriate process, an
    /// exception may occur. This can however be unsafe if attempting to use data stored in this
    /// address range
    ///
    /// # Panics
    ///
    /// Panics if the virtual address range exceeds the range possible for this address space, or
    /// if the physical range excees the range possible for descriptors
    #[inline]
    pub fn map(&mut self, va: usize, pa: u64, writeable: bool, is_device: bool) -> bool {
        let va_mask = (1_usize << ADDRESS_BITS) - 1;
        let page_mask = (1_usize << PAGE_SIZE_BITS) - 1;
        let page_mask_u64 = (1_u64 << PAGE_SIZE_BITS) - 1;
        // If we aren't mapping a higher half address, or if the virtual/phyiscal addresses is not aligned to
        // a page boundary, return false
        if (va & !va_mask) != !va_mask || (va & page_mask) != 0 || (pa & page_mask_u64) != 0 {
            return false;
        }

        #[expect(
            clippy::integer_division,
            reason = "Intentional rounding division used"
        )]
        #[expect(
            clippy::indexing_slicing,
            reason = "This should never fail, so a panic is appropriate in exceptional cases"
        )]
        self.table.0[(va & va_mask) / PAGE_SIZE] = #[expect(
            clippy::unwrap_used,
            reason = "This descriptor should always be valid to create"
        )]
        TranslationDescriptor::valid_base(pa)
            .unwrap()
            .with_writeable_never(!writeable)
            .with_privileged_execute_never(true)
            .with_memory_type(if is_device {
                MemoryAttribute::Device
            } else {
                MemoryAttribute::Normal
            });
        true
    }
}

/// The raw translation table for the kernel
#[expect(
    private_interfaces,
    reason = "This is only exported for the boot sequence to use and nowhere else"
)]
pub static mut TRANSLATION_TABLE: TranslationTable =
    TranslationTable([TranslationDescriptor::new(); _]);
/// The properly wrapped address space manager for the kernel
pub static ADDRESS_SPACE: SpinLock<AddressSpace> =
    SpinLock::new(AddressSpace::new(unsafe { &mut TRANSLATION_TABLE }));

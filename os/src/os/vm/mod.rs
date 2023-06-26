use crate::os::InitCell;
use crate::sync::SpinLock;
use bitfield_struct::bitfield;
use core::ptr::NonNull;
use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::{FromPrimitive, ToPrimitive};

mod elf;
pub use elf::load_elf;

#[bitfield(u64)]
struct PageDirectoryEntry {
    valid: bool,
    is_pte_pointer: bool,
    #[bits(10)]
    _ignored: u16,
    #[bits(36)]
    page_table_address: u64,
    #[bits(3)]
    res0: u8,
    ignored: u8,
    privilege_xn: bool,
    execute_never: bool,
    el0_accessible: bool,
    writeable: bool,
    _ns_table: bool,
}

impl PageDirectoryEntry {
    fn valid_base(page_table_addr: usize) -> Self {
        Self::new()
            .with_valid(true)
            .with_is_pte_pointer(true)
            .with_page_table_address((page_table_addr >> 12).try_into().unwrap())
            .with_privilege_xn(true)
            .with_execute_never(true)
            .with_el0_accessible(true)
    }
}

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
struct PageTableEntry {
    valid: bool,
    res1: bool,
    #[bits(3)]
    memory_type: MemoryAttribute,
    _ns: bool,
    el0_accessible: bool,
    writeable_never: bool,
    #[bits(2)]
    shareability: Shareability,
    access: bool,
    not_global: bool,
    #[bits(36)]
    pa: u64,
    #[bits(2)]
    _res0_4: u8,
    _guard_page: bool,
    dirty: bool,
    _contiguous: bool,
    privilege_execute_never: bool,
    execute_never: bool,
    #[bits(4)]
    _ignored2: u8,
    #[bits(4)]
    _hw_use: u8,
    _ignored: bool,
}

impl PageTableEntry {
    /// Generates a descriptor with appropriate defaults filled out, and minimal permissions
    ///
    /// Returns `None` if `pa` is too large
    fn valid_base(pa: u64) -> Option<Self> {
        (pa < 0x10_0000_0000_0000).then_some(
            Self::new()
                .with_valid(true)
                .with_res1(true)
                .with_memory_type(MemoryAttribute::Normal)
                .with_el0_accessible(true)
                .with_writeable_never(true)
                .with_shareability(Shareability::Inner)
                .with_access(true)
                .with_not_global(true)
                // SAFETY: `u64::BITS = 64 > 12`
                .with_pa(unsafe { pa.unchecked_shr(12) })
                .with_privilege_execute_never(false)
                .with_execute_never(true),
        )
    }
}

#[repr(transparent)]
/// A final-level translation table, containing descriptors pointing to physical pages
struct PageTable<const PAGE_BITS: u8, const REMAINING_BITS: u8>(
    [PageTableEntry; 1 << (REMAINING_BITS - PAGE_BITS)],
)
where
    [(); 1 << (REMAINING_BITS - PAGE_BITS)]: Sized;

impl<const PAGE_BITS: u8, const REMAINING_BITS: u8> PageTable<PAGE_BITS, REMAINING_BITS>
where
    [(); 1 << (REMAINING_BITS - PAGE_BITS)]: Sized,
{
    /// Gets the entry associated with the appropriate virtual address, assuming all higher-order
    /// bits have been masked out
    fn get_mut(&mut self, address: usize) -> Option<&mut PageTableEntry> {
        self.0.get_mut(address >> PAGE_BITS)
    }
}

#[repr(C)]
pub struct AddressSpace<const PAGE_BITS: u8, const ADDRESS_BITS: u8>
where
    [(); 1 << (ADDRESS_BITS - PAGE_BITS)]: Sized,
{
    /// Pointer to the top-level translation table for this address space
    base_table: NonNull<PageTable<PAGE_BITS, ADDRESS_BITS>>,
}

impl<const PAGE_BITS: u8, const ADDRESS_BITS: u8> AddressSpace<PAGE_BITS, ADDRESS_BITS>
where
    [(); 1 << (ADDRESS_BITS - PAGE_BITS)]: Sized,
{
    /// Creates a new address space where the base table is virtually accessible by the given
    /// pointer
    ///
    /// # Safety
    ///
    /// The table pointer must be valid for the lifetime of the returned address space.
    ///
    /// The table must be properly aligned
    #[inline]
    #[must_use]
    pub const unsafe fn new(base_table: NonNull<()>) -> Self {
        Self {
            base_table: base_table.cast(),
        }
    }

    /// A safe wrapper to extract a mutable reference to the tables
    #[must_use]
    fn table<'address, 'table>(&'address mut self) -> &'table mut PageTable<PAGE_BITS, ADDRESS_BITS>
    where
        'address: 'table,
    {
        // SAFETY: The conditions for teh creation of this address space ensure that this is a
        // safe operation
        unsafe { self.base_table.as_mut() }
    }

    /// Maps the given virtual address range to the given physical address range, with the
    /// specified attributes. Overrides any existing mappings for that region.
    ///
    /// Note that, while not unsafe, if the physical range is not owned by the appropriate process, an
    /// exception may occur. This can however be unsafe if attempting to use data stored in this
    /// address range
    ///
    /// # Safety
    ///
    /// Both `va` and `pa` must be suitably aligned.
    ///
    /// # Panics
    ///
    /// Panics if the virtual address range exceeds the range possible for this address space, or
    /// if the physical range excees the range possible for descriptors
    #[inline]
    pub unsafe fn map_range(
        &mut self,
        va: u64,
        pa: u64,
        size: u64,
        writeable: bool,
        executable: bool,
        is_device: bool,
    ) {
        for offset in (0..size).step_by(1 << PAGE_BITS) {
            *self
                .table()
                .get_mut((va + offset).try_into().unwrap())
                .unwrap() = PageTableEntry::valid_base(pa + offset)
                .unwrap()
                .with_writeable_never(!writeable)
                .with_execute_never(!executable)
                .with_memory_type(if is_device {
                    MemoryAttribute::Device
                } else {
                    MemoryAttribute::Normal
                });
        }
    }
}

pub static ADDRESS_SPACE: InitCell<SpinLock<AddressSpace<16, 25>>> = InitCell::new();

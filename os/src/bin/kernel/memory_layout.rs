//! Memory layout during initialization sequence. The constants in this module are fixed in place,
//! whereas statics may be determined at runtime. For example, the size of the file system ELF is
//! not known statically.

use core::num::NonZeroUsize;
use core::ptr::NonNull;

/// A virtual and physical memory entry. Begins at the specified physical and virtual addresses,
/// and is contiguously mapped for the specified size
pub struct Mapping {
    /// The physical address of the beginning of this mapping
    pub pa: u64,
    /// The virtual address of the beginning of this mapping
    pub va: NonNull<()>,
    /// The size of this contiguous mapping, both in physical and virtual memory
    pub size: NonZeroUsize,
}

impl Mapping {
    /// Creates a new memory mapping at the given location with the given size. Virtual addresses
    /// are automatically masked to be in the higher half
    const fn new(pa: u64, va: usize, size: usize) -> Self {
        const VA_MASK: usize = 0xFFFF_FFFF_FE00_0000;
        Self {
            pa,
            va: #[expect(
                clippy::unwrap_used,
                reason = "This should never generate a null pointer"
            )]
            #[expect(
                clippy::as_conversions,
                reason = "Const pointer creation from exposed address is not yet possible"
            )]
            NonNull::new((VA_MASK | va) as *mut ()).unwrap(),
            size: #[expect(
                clippy::expect_used,
                reason = "This should panic at compile time if not satisifed and be fixed statically"
            )]
            NonZeroUsize::new(size).expect("Mappings should have nonzero size"),
        }
    }
}

/// Space reserved for kernel stacks
pub const STACKS: Mapping = Mapping::new(0, 0x1FF_0000, 0x1_0000);
pub const FS_TRANSLATION_TABLE: Mapping = Mapping::new(0x4000, 0x1FF_4000, 0x4000);
// The size is only known when building the final binary
pub static mut FS_ELF: Mapping = Mapping::new(0x390_0000, 0x100_0000, usize::MAX);

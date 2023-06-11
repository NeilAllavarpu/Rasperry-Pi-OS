use core::num::NonZeroUsize;
use core::ptr::NonNull;

pub struct Mapping {
    pub pa: u64,
    pub va: NonNull<()>,
    pub size: NonZeroUsize,
}

impl Mapping {
    const fn new(pa: u64, va: usize, size: usize) -> Self {
        Self {
            pa,
            va: NonNull::new((va | 0xFFFF_FFFF_FE00_0000) as *mut ()).unwrap(),
            size: NonZeroUsize::new(size).unwrap(),
        }
    }
}

pub const STACKS: Mapping = Mapping::new(0, 0x100_0000, 0x4000);
pub const FS_TRANSLATION_TABLE: Mapping = Mapping::new(0x4000, 0x100_4000, 0x4000);
// The size is only known when building the final binary
pub const FS_ELF: Mapping = Mapping::new(0x1_0000, 0x110_0000, usize::MAX);

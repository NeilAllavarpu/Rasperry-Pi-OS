use crate::{call_once, cell::InitCell, log, sync::BlockingLock};
use core::{
    alloc::{GlobalAlloc, Layout},
    cell::UnsafeCell,
    cmp::max,
    num::NonZeroUsize,
    ptr::NonNull,
};
use smallvec::SmallVec;

/// Set to store free blocks
mod internal_set;
use internal_set::FreeSet;
/// A pointer to the next node in the free set
type NextPtr = Option<NonNull<BlockingLock<FreeBlock>>>;

/// Internally stored data for each free block
struct FreeBlock {
    /// The next free node in the linked list
    next: NextPtr,
}

/// The general purpose heap allocator for the kernel
struct HeapAllocator<const MIN_BLOCK_SIZE: usize> {
    /// The various heap blocks
    free_sets: InitCell<SmallVec<[FreeSet; 12]>>,
}

impl<const MIN_BLOCK_SIZE: usize> HeapAllocator<MIN_BLOCK_SIZE> {
    /// Creates a new, uninitialized heap allocator
    const fn new() -> Self {
        Self {
            free_sets: InitCell::new(),
        }
    }

    /// Initializes the heap allocator
    unsafe fn init(&self, initial_start: NonNull<()>, initial_size: usize) {
        call_once!();
        // SAFETY: This is the init sequence
        unsafe {
            self.free_sets
                .set(SmallVec::from(core::array::from_fn(|_| FreeSet::default())));
        }
        assert_eq!((initial_size >> (self.free_sets.len() - 1)), MIN_BLOCK_SIZE);
        let set = self
            .free_sets
            .get(Self::index_of(
                Layout::from_size_align(initial_size, initial_size).expect("Should work"),
            ))
            .expect("Should be in bounds");
        // SAFETY: The caller ensures that `initial_start` is suitably large and aligned
        unsafe { set.insert(initial_start) };
    }

    /// Constant function to... add one
    const fn add_one(n: u32) -> u32 {
        n + 1
    }

    /// Computes the set index for a given layout
    const fn index_of(layout: Layout) -> usize {
        usize::try_from(
            ((max(layout.align(), layout.size()) - 1) / MIN_BLOCK_SIZE)
                .checked_ilog2()
                .map_or(0, Self::add_one),
        )
        .unwrap_or(usize::MAX)
    }

    /// Computes the block size represented by a given index
    const fn block_size_of(index: usize) -> NonZeroUsize {
        NonZeroUsize::new(MIN_BLOCK_SIZE << index).expect("Block size should not be zero")
    }

    /// Computes the buddy of the given block
    const fn buddy_of(block: NonZeroUsize, block_size: NonZeroUsize) -> NonNull<()> {
        #[allow(clippy::as_conversions)]
        NonNull::new((usize::from(block) ^ usize::from(block_size)) as *mut ())
            .expect("Buddy should not be null")
    }

    /// Logs the heap usage
    /// # Safety
    /// Only to be used for logging. Should not be treated as perfectly accurate or thread safe
    unsafe fn log(&self) {
        for (n, free) in self.free_sets.iter().enumerate() {
            log!(
                "BLOCK SIZE 0x{:X}: {} free blocks",
                Self::block_size_of(n),
                free.len()
            );
        }
    }

    /// Allocates a block for the block size corresponding to the given set
    fn alloc_block(&self, index: usize) -> Option<NonNull<()>> {
        let set = self.free_sets.get(index)?;
        set.pop().or_else(|| {
            let block = self.alloc_block(index + 1)?;
            let block_size = Self::block_size_of(index);
            let buddy = Self::buddy_of(block.addr(), block_size);
            // SAFETY: The buddy block is suitably sized and aligned, and not in use
            assert!(unsafe { set.insert(buddy) });
            Some(block)
        })
    }

    /// Deallocates a block for the block size corresponding to the given set
    /// SAFETY: `ptr` must have been allocated via `alloc_block` for the same
    /// `usize`
    unsafe fn dealloc_block(&self, ptr: NonNull<()>, index: usize) {
        let set = self.free_sets.get(index).expect("Should be in-bounds");
        let block_size = Self::block_size_of(index);
        assert!(ptr.as_ptr().is_aligned_to(block_size.into()));

        // If the "buddy" is already free:
        if set.remove_buddy_or_insert(ptr, block_size) {
            // SAFETY: The merged block was acquired via a higher-level
            // `alloc_block`, so this is safe
            unsafe {
                self.dealloc_block(
                    NonNull::new((usize::from(ptr.addr()) & !usize::from(block_size)) as *mut ())
                        .expect("Merged block should not be null"),
                    index + 1,
                );
            }
        }
    }
}

/// The global kernel heap
#[global_allocator]
static KERNEL_HEAP: HeapAllocator<MIN_BLOCK_SIZE> = HeapAllocator::new();
/// Minimum block size for allocations
const MIN_BLOCK_SIZE: usize = 64;

// SAFETY: This heap should be correct
unsafe impl<const MIN_BLOCK_SIZE: usize> GlobalAlloc for HeapAllocator<MIN_BLOCK_SIZE> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.alloc_block(Self::index_of(layout))
            .map_or(core::ptr::null_mut(), |ptr| ptr.as_ptr().cast())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: The caller verifies the conditions
        unsafe {
            self.dealloc_block(
                NonNull::new(ptr.cast()).expect("Pointer should not be null"),
                Self::index_of(layout),
            );
        }
    }
}

/// Logs statistics regarding heap usage
/// # Safety
/// This function is not thread safe. It is intended to only be used for logging purposes.
pub unsafe fn log_allocator() {
    // SAFETY: By assumption, this is used in a logging, not necessarily safe context
    unsafe { KERNEL_HEAP.log() }
}

/// Initializes the global kernel heap
/// # Safety
/// Must be initialized only once
pub unsafe fn init() {
    extern "Rust" {
        static __heap_start: UnsafeCell<()>;
        static __heap_size: UnsafeCell<()>;
    }
    call_once!();
    // SAFETY: This is the correct time to initialize the heap, and only one core runs this
    unsafe {
        KERNEL_HEAP.init(
            NonNull::new(__heap_start.get()).expect("Heap start should not be null"),
            __heap_size.get().to_bits(),
        );
    }
}

//! A heap implementation and any associated utilities

use crate::sync::SpinLock;
use core::alloc::{AllocError, Allocator, GlobalAlloc, Layout};
use core::ops::Deref;
use core::{num::NonZeroUsize, ptr::NonNull};

mod ordered_map;
use ordered_map::OrderedBuddyMap;

/// Computes the integer base-2 logarithm, casted to u8
const fn ilog2_u8(n: NonZeroUsize) -> u8 {
    // Since `NonZeroUsize::BITS <= 256`, `n.ilog2() <=
    // NonZeroUsize::MAX.ilog2() = (2^256 - 1).ilog2() <= 255 = u8::MAX`
    #[expect(
        clippy::cast_possible_truncation,
        clippy::as_conversions,
        reason = "Const conversion from u32 to u8 is not currently possible"
    )]
    (n.ilog2() as u8)
}

/// Returns the greatest power of 2 less than or equal to `n`
const fn prev_power_of_2(n: NonZeroUsize) -> NonZeroUsize {
    // SAFETY: Since `n <= NonZeroUsize::MAX = 2^NonZeroUsize::BITS - 1`, `n.ilog2() <=
    // NonZeroUsize::MAX.ilog2() < (2^NonZeroUsize::BITS).ilog2() = NonZeroUsize::BITS`
    let computation = unsafe { 1_usize.unchecked_shl(n.ilog2()) };
    // SAFETY: `computation >= 1`
    unsafe { NonZeroUsize::new_unchecked(computation) }
}

/// The backend for the buddy allocator. Allows the allocator to dynamically change size as needed
pub trait AllocatorBackend {
    /// Grows the allocator by the given region, starting at the given end pointer, in place
    ///
    /// Returns true if the resizing succeeds
    fn grow(&mut self, end: NonNull<()>, size: NonZeroUsize) -> bool;
}

/// A buddy allocator
pub struct BuddyAllocator<Backend: AllocatorBackend> {
    /// The start of the region used by this allocator
    start: NonNull<()>,
    /// The current size of the region used by this allocator
    size: usize,
    /// The map storing all free blocks for this allocator, as well as the backend to expand the
    /// heap
    heap: SpinLock<(OrderedBuddyMap, Backend)>,
}

impl<Backend: AllocatorBackend> BuddyAllocator<Backend> {
    /// Creates a buddy allocator with the given initial memory range
    ///
    /// Returns `None` if `end < start`
    ///
    /// # Safety
    ///
    /// * The range must be valid for reads and writes
    /// * The range must not be in use by anything else
    pub unsafe fn new(start: NonNull<()>, end: NonNull<()>, backend: Backend) -> Option<Self> {
        // SAFETY: `start` and `end` are considered as the same allocated object
        unsafe { end.as_ptr().byte_offset_from(start.as_ptr()) }
            .try_into()
            .ok()
            .map(|size| Self {
                start,
                size,
                // SAFETY: The caller guarantees that this range is suitable
                heap: SpinLock::new((unsafe { OrderedBuddyMap::new(start, size) }, backend)),
            })
    }
}

#[expect(clippy::missing_trait_methods, reason = "Defaults are acceptable here")]
// SAFETY: Allocated blocks are persistent until deallocated; the allocator is safe to be moved;
// and allocated blocks can freely be passed among methods
unsafe impl<Backend: AllocatorBackend> Allocator for BuddyAllocator<Backend> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        // Zero size allocations don't need to do anything
        let Some(size) = NonZeroUsize::new(layout.size()) else {
            return Ok(NonNull::slice_from_raw_parts(NonNull::dangling(), 0));
        };
        // Since block alignment is at least as much as block size, rounding up the block size to
        // alignment if necessary guarantees compatibility. Also, block sizes must be powers of two
        let Some(block_size) = NonZeroUsize::new(layout.align())
            .map_or(size, |align| size.max(align))
            .checked_next_power_of_two() else {
                return Err(AllocError {})
            };

        let mut heap = self.heap.lock();
        let (map, backend) = &mut *heap;
        let mut result = map.pop(ilog2_u8(block_size));
        // If initial allocation fails, try to expand the heap and retry
        if result.is_none() {
            /// The minimum size by which to grow the heap, if necessary
            #[expect(clippy::unwrap_used, reason = "Const unwrap cannot panic at runtime")]
            const MIN_GROW_SIZE: NonZeroUsize = NonZeroUsize::new(4096).unwrap();
            // SAFETY: The grower cannot allocate with wrapping around, so the range for the
            // heap does not wrap around the address space. `size` must fit into an `isize`
            // because we cannot use half of the address space, bounding us to an `isize`. This
            // is considered in the same allocated object as the heap range.
            let heap_end_raw = unsafe { self.start.as_ptr().byte_add(self.size) };
            // SAFETY: This cannot be 0, assuming a proper backend implementation
            let heap_end = unsafe { NonNull::new_unchecked(heap_end_raw) };
            result = if backend.grow(
                heap_end,
                NonZeroUsize::new(self.size)
                    .map_or(MIN_GROW_SIZE, prev_power_of_2)
                    .max(MIN_GROW_SIZE),
            ) {
                // SAFETY: This region of memory was just given to use by the grower
                unsafe { map.remove_buddy_or_insert_recursive(heap_end.cast(), ilog2_u8(size)) };
                map.pop(ilog2_u8(block_size))
            } else {
                None
            };
        }
        result
            .map(|block| NonNull::slice_from_raw_parts(block.cast(), block_size.get()))
            .ok_or(AllocError {})
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        // Zero size allocations don't allocate
        let Some(size) = NonZeroUsize::new(layout.size()) else { return };
        #[expect(clippy::expect_used, reason = "Used to verify unsafe preconditions")]
        let block_size = NonZeroUsize::new(layout.align())
            .map_or(size, |align| size.max(align))
            .checked_next_power_of_two()
            .expect("The size of an allocated block should not overflow");

        // SAFETY: The caller guarantees that the given block is appropriately allocated
        unsafe {
            self.heap
                .lock()
                .0
                .remove_buddy_or_insert_recursive(ptr.cast(), ilog2_u8(block_size));
        };
    }
}

unsafe impl<Backend: AllocatorBackend> GlobalAlloc for BuddyAllocator<Backend> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.allocate(layout)
            .map(NonNull::as_mut_ptr)
            .unwrap_or(core::ptr::null_mut())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe {
            self.deallocate(
                NonNull::new(ptr).expect("Allocated pointers were never null"),
                layout,
            )
        }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        self.allocate_zeroed(layout)
            .map(NonNull::as_mut_ptr)
            .unwrap_or(core::ptr::null_mut())
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        let pointer = NonNull::new(ptr).expect("Allocated pointers were never null");
        if new_size < layout.size() {
            unsafe { self.shrink(pointer, layout, new_layout) }
        } else {
            unsafe { self.grow(pointer, layout, new_layout) }
        }
        .map(NonNull::as_mut_ptr)
        .unwrap_or(core::ptr::null_mut())
    }
}

unsafe impl<Backend: AllocatorBackend> GlobalAlloc
    for crate::cell::InitCell<BuddyAllocator<Backend>>
{
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { self.deref().alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { self.deref().dealloc(ptr, layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        unsafe { self.deref().alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        unsafe { self.deref().realloc(ptr, layout, new_size) }
    }
}

// SAFETY: The spinlocks guarantee thread safety for the actual heap operations, and everything
// else is constant after initialization.
unsafe impl<Backend: AllocatorBackend> Sync for BuddyAllocator<Backend> {}

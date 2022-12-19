// use crate::kernel::Mutex;
use crate::{call_once, collections, log};
use core::{
    alloc::{GlobalAlloc, Layout},
    cmp::{max, min},
};

// TODO: Use a paging-based, dynamically sized heap
/// The static start of the heap
#[allow(clippy::as_conversions)]
const HEAP_START: *mut () = 0x20_0000 as *mut ();
/// The static size of the heap
const HEAP_SIZE: usize = 0x20_0000;

/// A free block in a fixed block heap
struct FreeBlock {
    /// Pointer to the next free block
    next: *mut FreeBlock,
}
impl collections::Stackable for FreeBlock {
    fn read_next(&self) -> *mut Self {
        self.next
    }

    unsafe fn set_next(&mut self, next: *mut Self) {
        self.next = next;
    }
}

/// A fixed-block allocator
#[allow(clippy::module_name_repetitions)]
pub struct FixedBlockHeap {
    /// The next free block in the heap
    first_free: collections::UnsafeStack<FreeBlock>,
    /// Block size, in bytes
    block_size: usize,
    /// The size of the heap
    size: usize,
}

impl FixedBlockHeap {
    /// Creates a new, default heap
    /// Should not be used until initialized
    pub const fn new(block_size: usize) -> Self {
        Self {
            first_free: collections::UnsafeStack::new(),
            block_size,
            size: 0,
        }
    }

    /// # Safety
    /// Implements `GlobalAlloc`'s `alloc`
    pub unsafe fn alloc(&mut self, layout: Layout) -> Option<*mut u8> {
        // Unimplemented: larger blocks
        if layout.size() > self.block_size {
            return None;
        }
        #[allow(clippy::as_conversions)]
        self.first_free.pop().map(<*mut FreeBlock>::cast)
    }

    /// # Safety
    /// Implements `GlobalAlloc`'s `dealloc`
    pub unsafe fn dealloc(&mut self, ptr: *mut u8, _layout: Layout) {
        // SAFETY: By assumption, `ptr` was returned from `alloc`, and so
        // respects proper placement and alignment
        unsafe {
            self.first_free.push(ptr.cast());
        }
    }

    /// Initializes the heap over the given range of memory
    /// # Safety
    /// The range of memory given must be appropriate
    pub unsafe fn init(&mut self, start: *mut (), size: usize) {
        assert!(self.block_size.is_power_of_two());

        for block_offset in (0..size).step_by(self.block_size) {
            // SAFETY: By construction, these pointers are all valid pointers
            // to unused, fixed heap space
            unsafe {
                self.first_free
                    .push(start.byte_add(block_offset).cast::<FreeBlock>());
            }
        }

        self.size = size;
    }

    /// Logs statistics regarding heap usage
    /// # Safety
    /// This function is not thread safe. It is intended to only be used for logging purposes.
    unsafe fn log(&self) {
        // SAFETY: This is only used for logging purposes
        let blocks_free = unsafe { self.first_free.depth() };
        log!(
            "HEAP BLOCKS {}B: {} Free blocks, {} In-use blocks",
            self.block_size,
            blocks_free,
            self.size / self.block_size - blocks_free
        );
    }
}

/// Number of blocks to use
const NUM_BLOCKS: usize = 3;
/// Size of individual blocks
#[allow(dead_code)]
const SIZES: [usize; NUM_BLOCKS] = [64, 256, 1024];
/// The general purpose heap allocator for the kernel
struct HeapAllocator {
    /// The various heap blocks
    blocks: [FixedBlockHeap; NUM_BLOCKS],
}

impl HeapAllocator {
    /// Creates a new, uninitialized heap allocator
    const fn new() -> Self {
        Self {
            blocks: [
                FixedBlockHeap::new(64),
                FixedBlockHeap::new(256),
                FixedBlockHeap::new(1024),
            ],
        }
    }

    /// Initializes the heap allocator
    fn init(&mut self) {
        call_once!();
        let mut remaining_size = HEAP_SIZE;
        let mut offset = HEAP_START;
        for heap in self.blocks.iter_mut().rev() {
            // SAFETY: These ranges are constructed as unused and nonoverlapping
            unsafe {
                heap.init(offset, remaining_size * 3 / 4);
                offset = offset.byte_add(remaining_size * 3 / 4);
            }
            remaining_size /= 4;
        }
    }

    /// Returns the index of the heap that would allocate the given `Layout`
    fn get_heap_index(&mut self, layout: Layout) -> Option<usize> {
        let block_size = max(layout.align(), layout.size());
        for (n, heap) in self.blocks.iter().enumerate() {
            if heap.block_size >= block_size {
                return Some(n);
            }
        }
        None
    }

    /// Returns the heap that would allocate the given `Layout`
    fn get_heap(&mut self, layout: Layout) -> Option<&mut FixedBlockHeap> {
        self.get_heap_index(layout)
            .and_then(|i| self.blocks.get_mut(i))
    }

    /// Logs the heap usage
    /// # Safety
    /// Only to be used for logging. Should not be treated as perfectly accurate or thread safe
    unsafe fn log(&self) {
        // SAFETY: By assumption, this is non-thread-safe logging
        unsafe {
            for heap in &self.blocks {
                heap.log();
            }
        }
    }
}

#[global_allocator]
/// The global kernel heap
static mut KERNEL_HEAP: HeapAllocator = HeapAllocator::new();

// SAFETY: This heap should be correct
unsafe impl GlobalAlloc for HeapAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // SAFETY: The kernel heap is designed to be thread safe
        unsafe { KERNEL_HEAP.get_heap(layout) }
            // SAFETY: By assumption, the layout should be valid
            .and_then(|heap| unsafe { heap.alloc(layout) })
            .unwrap_or(core::ptr::null_mut())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: The kernel heap is designed to be thread safe
        if let Some(heap) = unsafe { KERNEL_HEAP.get_heap(layout) } {
            // SAFETY: By assumption, the layout should be valid
            unsafe { heap.dealloc(ptr, layout) };
        }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: the caller must ensure that the `new_size` does not overflow.
        // `layout.align()` comes from a `Layout` and is thus guaranteed to be valid.
        let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        // SAFETY: The kernel heap is designed to be thread safe
        let new_heap = unsafe { KERNEL_HEAP.get_heap_index(new_layout) };
        // SAFETY: The kernel heap is designed to be thread safe
        if unsafe { KERNEL_HEAP.get_heap_index(layout) } == new_heap {
            return if new_heap.is_some() {
                ptr
            } else {
                core::ptr::null_mut()
            };
        }

        // Default reallocation behavior from rust source

        // SAFETY: the caller must ensure that `new_layout` is greater than zero.
        let new_ptr = unsafe { self.alloc(new_layout) };
        if !new_ptr.is_null() {
            // SAFETY: the previously allocated block cannot overlap the newly allocated block.
            // The safety contract for `dealloc` must be upheld by the caller.
            unsafe {
                core::ptr::copy_nonoverlapping(ptr, new_ptr, min(layout.size(), new_size));
                self.dealloc(ptr, layout);
            }
        }
        new_ptr
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
    call_once!();
    // SAFETY: This is the correct time to initialize the heap, and only one core runs this
    unsafe { KERNEL_HEAP.init() }
}

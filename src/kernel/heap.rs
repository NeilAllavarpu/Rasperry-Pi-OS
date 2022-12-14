// use crate::kernel::Mutex;
use crate::{call_once, kernel, log};
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
impl kernel::Stackable for FreeBlock {
    fn read_next(&self) -> *mut Self {
        self.next
    }

    unsafe fn set_next(&mut self, next: *mut Self) {
        self.next = next;
    }
}

/// A fixed-block allocator
struct FixedBlockHeap<const BLOCK_SIZE: usize> {
    /// The next free block in the heap
    first_free: kernel::Stack<FreeBlock>,
    /// The size of the heap
    size: usize,
}

impl<const BLOCK_SIZE: usize> FixedBlockHeap<BLOCK_SIZE> {
    /// Creates a new, default heap
    /// Should not be used until initialized
    const fn new() -> Self {
        Self {
            first_free: kernel::Stack::new(),
            size: 0,
        }
    }

    /// Implements `GlobalAlloc`'s `alloc`
    unsafe fn alloc(&mut self, layout: Layout) -> Option<*mut u8> {
        // Unimplemented: larger blocks
        if layout.size() > BLOCK_SIZE {
            return None;
        }
        #[allow(clippy::as_conversions)]
        self.first_free
            .pop()
            .map(|block| (block as *mut FreeBlock).cast())
    }

    /// Implements `GlobalAlloc`'s `dealloc`
    unsafe fn dealloc(&mut self, ptr: *mut u8, _layout: Layout) {
        self.first_free.push(
            #[allow(clippy::cast_ptr_alignment)]
            // SAFETY: By assumption, `ptr` was returned from `alloc`, and so
            // respects proper placement and alignment
            unsafe { ptr.cast::<FreeBlock>().as_mut() }
                .expect("Casting the dealloc pointer to a FreeBlock should succeed"),
        );
    }

    /// Initializes the heap over the given range of memory
    /// # Safety
    /// The range of memory given must be appropriate
    unsafe fn init(&mut self, start: *mut (), size: usize) {
        assert!(BLOCK_SIZE.is_power_of_two());

        for block_offset in (0..size).step_by(BLOCK_SIZE) {
            self.first_free.push(
                // SAFETY: By construction, these pointers are all valid pointers
                // to unused heap space
                unsafe { start.byte_add(block_offset).cast::<FreeBlock>().as_mut() }
                    .expect("Casting the pointer to a reference should succeed"),
            );
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
            BLOCK_SIZE,
            blocks_free,
            self.size / BLOCK_SIZE - blocks_free
        );
    }
}

/// The general purpose heap allocator for the kernel
struct HeapAllocator {
    /// 32-byte blocks
    b32: FixedBlockHeap<32>,
    /// 128-byte blocks
    b128: FixedBlockHeap<128>,
    /// 512-byte blocks
    b512: FixedBlockHeap<512>,
    // Anything larger can resort to page allotment
}

impl HeapAllocator {
    /// Creates a new, uninitialized heap allocator
    const fn new() -> Self {
        Self {
            b32: FixedBlockHeap::new(),
            b128: FixedBlockHeap::new(),
            b512: FixedBlockHeap::new(),
        }
    }

    /// Initializes the heap allocator
    fn init(&mut self) {
        call_once!();
        // SAFETY: These ranges are chosen to be unused and nonoverlapping
        unsafe {
            self.b512.init(HEAP_START, HEAP_SIZE * 3 / 4);
            self.b128
                .init(HEAP_START.byte_add(HEAP_SIZE * 3 / 4), HEAP_SIZE * 3 / 16);
            self.b32
                .init(HEAP_START.byte_add(HEAP_SIZE * 15 / 16), HEAP_SIZE / 16);
        }
    }

    /// Logs the heap usage
    /// # Safety
    /// Only to be used for logging. Should not be treated as perfectly accurate or thread safe
    unsafe fn log(&self) {
        // SAFETY: By assumption, this is non-thread-safe logging
        unsafe {
            self.b512.log();
            self.b128.log();
            self.b32.log();
        }
    }
}

#[global_allocator]
/// The global kernel heap
static mut KERNEL_HEAP: HeapAllocator = HeapAllocator::new();

// SAFETY: This heap should be correct
unsafe impl GlobalAlloc for HeapAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        match max(layout.align(), layout.size()) {
            // SAFETY: By assumption, the layout should be valid
            0..=32 => unsafe { KERNEL_HEAP.b32.alloc(layout) }.unwrap_or(core::ptr::null_mut()),
            // SAFETY: By assumption, the layout should be valid
            33..=128 => unsafe { KERNEL_HEAP.b128.alloc(layout) }.unwrap_or(core::ptr::null_mut()),
            // SAFETY: By assumption, the layout should be valid
            129..=512 => unsafe { KERNEL_HEAP.b512.alloc(layout) }.unwrap_or(core::ptr::null_mut()),
            _ => core::ptr::null_mut(),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        match max(layout.align(), layout.size()) {
            // SAFETY: By assumption, the pointer and layout should be valid
            0..=32 => unsafe { KERNEL_HEAP.b32.dealloc(ptr, layout) },
            // SAFETY: By assumption, the pointer and layout should be valid
            33..=128 => unsafe { KERNEL_HEAP.b128.dealloc(ptr, layout) },
            // SAFETY: By assumption, the pointer and layout should be valid
            129..=512 => unsafe { KERNEL_HEAP.b512.dealloc(ptr, layout) },
            _ => (),
        }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: the caller must ensure that the `new_size` does not overflow.
        // `layout.align()` comes from a `Layout` and is thus guaranteed to be valid.
        let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        let old_p2 = max(layout.align(), layout.size()).next_power_of_two();
        let new_p2 = max(new_layout.align(), new_layout.size()).next_power_of_two();
        if (old_p2 <= 32 && new_p2 <= 32)
            || (32 < old_p2 && old_p2 <= 128 && 32 < new_p2 && new_p2 <= 128)
            || (128 < old_p2 && old_p2 <= 512 && 128 < new_p2 && new_p2 <= 512)
        {
            // Fits in the same block, no need to reallocate
            return ptr;
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

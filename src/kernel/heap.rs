// use crate::kernel::Mutex;
use crate::{call_once, kernel, log};
use core::{
    alloc::{GlobalAlloc, Layout},
    cell::UnsafeCell,
    cmp::{max, min},
};

const HEAP_START: usize = 0x200000;
const HEAP_SIZE: usize = 0x200000;

struct FreeBlock(*mut FreeBlock);
impl kernel::Stackable for FreeBlock {
    fn read_next(&self) -> *mut Self {
        self.0
    }

    unsafe fn set_next(&mut self, next: *mut Self) -> () {
        self.0 = next;
    }
}

struct FixedBlockHeap<const BLOCK_SIZE: usize> {
    first_free: kernel::Stack<FreeBlock>,
    size: usize,
}

impl<const BLOCK_SIZE: usize> FixedBlockHeap<BLOCK_SIZE> {
    const fn new() -> Self {
        Self {
            first_free: kernel::Stack::new(),
            size: 0,
        }
    }

    unsafe fn alloc(&mut self, layout: Layout) -> Option<*mut u8> {
        // Unimplemented: larger blocks
        if layout.size() > BLOCK_SIZE {
            return None;
        }
        let block = self.first_free.pop();
        if block.is_none() {
            // For now, simply warn if the heap is out of memory
            log!("Out of heap space!")
        }
        block.map(|block| block as *mut FreeBlock as *mut u8)
    }

    unsafe fn dealloc(&mut self, ptr: *mut u8, _layout: Layout) -> () {
        self.first_free.push(&mut *(ptr as *mut FreeBlock));
    }

    /// **SAFETY**: The range of memory given must be appropriate
    unsafe fn init(&mut self, start: usize, size: usize) {
        assert!(BLOCK_SIZE.is_power_of_two());

        for block_offset in (0..size).step_by(BLOCK_SIZE) {
            self.first_free
                .push(&mut *((start + block_offset) as *mut FreeBlock))
        }

        self.size = size
    }

    unsafe fn log(&self) {
        let blocks_free = self.first_free.depth();
        log!(
            "HEAP BLOCKS {}B: {} Free blocks, {} In-use blocks",
            BLOCK_SIZE,
            blocks_free,
            self.size / BLOCK_SIZE - blocks_free
        )
    }
}

struct HeapAllocator {
    b32: UnsafeCell<FixedBlockHeap<32>>,
    b128: UnsafeCell<FixedBlockHeap<128>>,
    b512: UnsafeCell<FixedBlockHeap<512>>,
    // Anything larger can resort to page allotment
}

impl HeapAllocator {
    const fn new() -> Self {
        Self {
            b32: UnsafeCell::new(FixedBlockHeap::new()),
            b128: UnsafeCell::new(FixedBlockHeap::new()),
            b512: UnsafeCell::new(FixedBlockHeap::new()),
        }
    }

    fn init(&self) -> () {
        call_once!();
        unsafe { (*self.b512.get()).init(HEAP_START, HEAP_SIZE * 3 / 4) }
        unsafe { (*self.b128.get()).init(HEAP_START + HEAP_SIZE * 3 / 4, HEAP_SIZE * 3 / 16) }
        unsafe { (*self.b32.get()).init(HEAP_START + HEAP_SIZE * 15 / 16, HEAP_SIZE / 16) }
    }

    unsafe fn log(&self) -> () {
        (*self.b512.get()).log();
        (*self.b128.get()).log();
        (*self.b32.get()).log();
    }
}

#[global_allocator]
static mut KERNEL_HEAP: HeapAllocator = HeapAllocator::new();

unsafe impl GlobalAlloc for HeapAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        match max(layout.align(), layout.size()) {
            0..=32 => KERNEL_HEAP
                .b32
                .get_mut()
                .alloc(layout)
                .unwrap_or(core::ptr::null_mut()),
            0..=128 => KERNEL_HEAP
                .b128
                .get_mut()
                .alloc(layout)
                .unwrap_or(core::ptr::null_mut()),
            0..=512 => KERNEL_HEAP
                .b512
                .get_mut()
                .alloc(layout)
                .unwrap_or(core::ptr::null_mut()),
            _ => core::ptr::null_mut(),
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) -> () {
        match max(layout.align(), layout.size()) {
            0..=32 => KERNEL_HEAP.b32.get_mut().dealloc(ptr, layout),
            0..=128 => KERNEL_HEAP.b128.get_mut().dealloc(ptr, layout),
            0..=512 => KERNEL_HEAP.b512.get_mut().dealloc(ptr, layout),
            _ => (),
        }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        // SAFETY: the caller must ensure that the `new_size` does not overflow.
        // `layout.align()` comes from a `Layout` and is thus guaranteed to be valid.
        let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        let old_size = max(layout.size(), layout.size());
        let new_size = max(new_layout.size(), new_layout.size());
        log!("realloced {} -> {}", old_size, new_size);
        let old_p2 = old_size.next_power_of_two();
        let new_p2 = new_size.next_power_of_two();
        if (old_p2 <= 32 && new_p2 <= 32)
            || (32 < old_p2 && old_p2 <= 128 && 32 < new_p2 && new_p2 <= 128)
            || (128 < old_p2 && old_p2 <= 512 && 128 < new_p2 && new_p2 <= 512)
        {
            log!("optimized!");
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

pub unsafe fn log_allocator() {
    KERNEL_HEAP.log()
}

pub fn init() -> () {
    call_once!();
    unsafe { KERNEL_HEAP.init() }
}

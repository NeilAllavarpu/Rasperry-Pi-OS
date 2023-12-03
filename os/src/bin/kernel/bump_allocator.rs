//! A simple bump allocator. Intended primarily for use with allocating initial, global structs,
//! that are not deallocated during the program and so do not need much deallocation support.
use core::{
    alloc::{AllocError, Allocator, GlobalAlloc, Layout},
    num::NonZeroUsize,
    ptr::NonNull,
    sync::atomic::{AtomicPtr, Ordering},
};

use crate::println;

/// A bump allocator that grows upwards in a certain region of memory
#[derive(Debug)]
pub struct BumpAllocator {
    /// The current top of the bump allocator, where new allocations are serviced from
    pointer: AtomicPtr<u8>,
    /// The upper bound of the region of memory set aside for the bump allocator
    bound: NonZeroUsize,
}

impl BumpAllocator {
    pub const fn empty() -> Self {
        let ptr = NonZeroUsize::new(0x1000).unwrap();
        Self {
            pointer: AtomicPtr::new(ptr.get() as *mut u8),
            bound: ptr,
        }
    }
    /// Creates a new bump allocator using the provided region of memory
    pub fn new(start: NonNull<u8>, end: NonNull<u8>) -> Self {
        Self {
            pointer: AtomicPtr::new(start.as_ptr()),
            bound: end.addr(),
        }
    }

    /// Creates a new bump allocator using the provided region of memory
    pub fn set(&mut self, start: NonNull<u8>, end: NonNull<u8>) {
        *self = Self {
            pointer: AtomicPtr::new(start.as_ptr()),
            bound: end.addr(),
        }
    }
}

/// Aligns the given pointer to the provided alignment by rounding up if necsesary
pub fn align_to(pointer: *mut u8, alignment: usize) -> *mut u8 {
    pointer.map_addr(|address| address.next_multiple_of(alignment))
}

// SAFETY: Allocated blocks are persistent until deallocated; the allocator is safe to be moved;
// and allocated blocks can freely be passed among methods
unsafe impl Allocator for BumpAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        // Zero size allocations don't need to do anything
        if layout.size() == 0 {
            Ok(NonNull::slice_from_raw_parts(layout.dangling(), 0))
        } else {
            match self
                .pointer
                .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |pointer| {
                    let base_pointer = align_to(pointer, layout.align());
                    let final_pointer = base_pointer.wrapping_byte_add(layout.size());
                    // println!(
                    //     "final ptr {:X} bound {:X}",
                    //     final_pointer.addr(),
                    //     self.bound.get()
                    // );
                    (final_pointer.addr() <= self.bound.get()).then_some(final_pointer)
                }) {
                Ok(reserved_pointer) => Ok(NonNull::slice_from_raw_parts(
                    NonNull::new(align_to(reserved_pointer, layout.align())).ok_or(AllocError)?,
                    layout.size(),
                )),
                Err(_) => Err(AllocError),
            }
        }
    }

    unsafe fn deallocate(&self, _: NonNull<u8>, _: Layout) {
        // println!("WARNING: Deallocating from bump allocator!");
    }
}

unsafe impl GlobalAlloc for BumpAllocator {
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
}

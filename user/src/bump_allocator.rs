//! A simple bump allocator. Intended primarily for use with allocating initial, global structs,
//! that are not deallocated during the program and so do not need much deallocation support.
use core::{
    alloc::{AllocError, Allocator, GlobalAlloc, Layout},
    marker::PhantomData,
    ptr::{self, NonNull},
    sync::atomic::{AtomicPtr, Ordering},
};

/// A bump allocator that grows upwards in a certain region of memory
#[derive(Debug)]
pub struct BumpAllocator<'allocator> {
    /// The current top of the bump allocator, where new allocations are serviced from
    pointer: AtomicPtr<u8>,
    /// The upper bound of the region of memory set aside for the bump allocator
    bound: *mut u8,
    /// Represents the exclusive access for the memory region involved
    _marker: PhantomData<&'allocator mut [u8]>,
}

impl<'allocator> BumpAllocator<'allocator> {
    /// Const-creates an empty bump allocator, which allocates no space
    pub const fn empty() -> Self {
        Self {
            pointer: AtomicPtr::new(ptr::null_mut()),
            bound: ptr::null_mut(),
            _marker: PhantomData {},
        }
    }

    /// Creates a new bump allocator using the provided region of memory
    /// # Safety
    /// * Any previous allocations from this allocator must still remain valid for the lifetime of this allocator.
    /// This is trivially satisified if there are no previous allocations.
    pub const unsafe fn set<'memory>(&'allocator mut self, memory: &'memory mut [u8])
    where
        'memory: 'allocator,
    {
        let memory = memory.as_mut_ptr_range();
        *self = Self {
            pointer: AtomicPtr::new(memory.start),
            bound: memory.end,
            _marker: PhantomData {},
        }
    }
}

/// Aligns the given pointer to the provided alignment by rounding up if necsesary
pub fn align_to(pointer: *mut u8, alignment: usize) -> *mut u8 {
    pointer.map_addr(|address| address.next_multiple_of(alignment))
}

// SAFETY: Allocated blocks are persistent until deallocated; the allocator is safe to be moved;
// and allocated blocks can freely be passed among methods
unsafe impl Allocator for BumpAllocator<'_> {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        // Zero size allocations don't need to do anything
        if layout.size() == 0 {
            Ok(NonNull::slice_from_raw_parts(layout.dangling(), 0))
        } else {
            let old_pointer =
                self.pointer
                    .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |pointer| {
                        let base_pointer = align_to(pointer, layout.align());
                        let final_pointer = base_pointer.wrapping_byte_add(layout.size());
                        (final_pointer <= self.bound).then_some(final_pointer)
                    });
            match old_pointer {
                Ok(reserved_pointer) => Ok(NonNull::slice_from_raw_parts(
                    NonNull::new(align_to(reserved_pointer, layout.align())).ok_or(AllocError)?,
                    layout.size(),
                )),
                Err(_) => Err(AllocError),
            }
        }
    }

    unsafe fn deallocate(&self, _: NonNull<u8>, _: Layout) {}
}

unsafe impl GlobalAlloc for BumpAllocator<'_> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        self.allocate(layout)
            .map(NonNull::as_mut_ptr)
            .unwrap_or(ptr::null_mut())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: The caller promises to uphold the relevant safety conditions
        unsafe {
            self.deallocate(
                NonNull::new(ptr).expect("Allocated pointers were never null"),
                layout,
            );
        }
    }
}

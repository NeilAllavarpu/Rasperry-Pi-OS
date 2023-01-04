use crate::{
    kernel::heap::{FreeBlock, MIN_BLOCK_SIZE},
    sync::{BlockingLock, Mutex},
};
use core::{
    mem,
    num::NonZeroUsize,
    ptr::NonNull,
    sync::atomic::{AtomicUsize, Ordering},
};

/// A set of free memory blocks
pub struct FreeSet {
    /// The head of the linked list
    head: BlockingLock<FreeBlock>,
    /// Number of elements in the set
    len: AtomicUsize,
}

impl FreeSet {
    /// Inserts the given free block into the set. Returns true
    /// # Safety
    /// `block` must be at least 8-byte aligned, and have space for at least 64
    /// bytes of space
    pub unsafe fn insert(&self, block: NonNull<()>) -> bool {
        assert!(block.as_ptr().is_aligned_to(MIN_BLOCK_SIZE));
        let free_pointer = block.cast();
        let mut node = self.head.lock();

        while let Some(next_ptr) = { node.next } {
            // SAFETY: This node was inserted by a prior call to `insert`
            // * The pointer must be properly aligned: Guaranteed to be aligned
            // by the prior insert (minimum alignment 8 bytes)
            // * It must be "dereferenceable" in the sense defined in the module
            // documentation, a.k.a the memory range of the given size starting
            // at the pointer must all be within the bounds of a single
            // allocated object: Guaranteed by the prior insert (minimum size 64
            // bytes)
            // * The pointer must point to an initialized instance of T:
            // Guaranteed by prior insert (initialization before lock release)
            // * You must enforce Rust's aliasing rules, since the returned
            // lifetime 'a is arbitrarily chosen and does not necessarily
            // reflect the actual lifetime of the data. In particular, while this reference exists, the memory the pointer points to must not get mutated (except inside UnsafeCell): All accesses are through shared references, and
            // acquire `SpinLock`s before mutation (which is allowed)
            let next = unsafe { next_ptr.as_ref() }.lock();
            if next_ptr > free_pointer {
                break;
            }
            if next_ptr == free_pointer {
                return false;
            }

            node = next;
        }

        // SAFETY: The caller ensures that this is a safe operation
        unsafe {
            free_pointer
                .as_ptr()
                .write(BlockingLock::new(FreeBlock { next: node.next }));
        }
        node.next = Some(free_pointer);

        self.len.fetch_add(1, Ordering::Relaxed);

        true
    }

    /// Removes the given free block, if present. Returns whether or not the
    /// block was present
    pub fn remove_buddy_or_insert(&self, block: NonNull<()>, block_size: NonZeroUsize) -> bool {
        let free_pointer = block.cast();
        let buddy =
            NonNull::new((usize::from(free_pointer.addr()) ^ usize::from(block_size)) as *mut ())
                .expect("Buddy should not be null");
        let mut node = self.head.lock();

        while let Some(next_ptr) = node.next {
            // SAFETY: This node was inserted by a prior call to `insert`
            // * The pointer must be properly aligned: Guaranteed to be aligned
            // by the prior insert (minimum alignment 8 bytes)
            // * It must be "dereferenceable" in the sense defined in the module
            // documentation, a.k.a the memory range of the given size starting
            // at the pointer must all be within the bounds of a single
            // allocated object: Guaranteed by the prior insert (minimum size 64
            // bytes)
            // * The pointer must point to an initialized instance of T:
            // Guaranteed by prior insert (initialization before lock release)
            // * You must enforce Rust's aliasing rules, since the returned
            // lifetime 'a is arbitrarily chosen and does not necessarily
            // reflect the actual lifetime of the data. In particular, while this reference exists, the memory the pointer points to must not get mutated (except inside UnsafeCell): All accesses are through shared references, and
            // acquire `SpinLock`s before mutation (which is allowed)
            let next = unsafe { next_ptr.as_ref() }.lock();

            // The buddy is here, remove it and indicate so
            if next_ptr == buddy.cast() {
                node.next = next.next;
                self.len.fetch_sub(1, Ordering::Relaxed);
                return true;
            }

            // Overshot the pointer; insert it, as the buddy is not present
            if next_ptr > free_pointer {
                break;
            }

            node = next;
        }

        // SAFETY: The caller ensures that this is a safe operation
        unsafe {
            free_pointer
                .as_ptr()
                .write(BlockingLock::new(FreeBlock { next: node.next }));
        }
        node.next = Some(free_pointer);

        self.len.fetch_add(1, Ordering::Relaxed);
        false
    }

    /// Removes an arbitrary block from the set, if non-empty
    pub fn pop(&self) -> Option<NonNull<()>> {
        let mut head = self.head.lock();
        let next_ptr = head.next?;
        // SAFETY: By assumptions in `insert`, all pointers in the linked
        // list are valid to convert to references
        let next = unsafe { next_ptr.as_ref() }.lock();
        head.next = next.next;
        self.len.fetch_sub(1, Ordering::Relaxed);
        Some(next_ptr.cast())
    }

    /// Returns the number of elements in the set
    pub fn len(&self) -> usize {
        self.len.load(Ordering::Relaxed)
    }
}

impl const Default for FreeSet {
    /// Creates a new empty set
    fn default() -> Self {
        assert!(mem::size_of::<BlockingLock<FreeBlock>>() <= MIN_BLOCK_SIZE);
        Self {
            head: BlockingLock::new(FreeBlock { next: None }),
            len: AtomicUsize::new(0),
        }
    }
}

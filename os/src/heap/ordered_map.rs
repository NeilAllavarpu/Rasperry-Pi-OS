//! The underlying data structure used for the buddy allocator. It is a linked list free set, with
//! optimizations for the buddy allocator operations

use super::{ilog2_u8, prev_power_of_2};
use core::mem;
use core::num::NonZeroUsize;
use core::ptr::NonNull;

/// Const-converts the given `u8` to a `u32`
const fn to_u32(n: u8) -> u32 {
    #[expect(
        clippy::as_conversions,
        reason = "There is no other way to const convert a `u8` to `u32` currently"
    )]
    (n as u32)
}

/// Returns `2^exponent - 1`
///
/// # Safety
///
/// `exponent` must be less than `usize::BITS`
const unsafe fn exponent_minus_1(exponent: u8) -> usize {
    debug_assert!(
        to_u32(exponent) < usize::BITS,
        "Exponent must be less than the number of bits in a `usize`"
    );
    // SAFETY: The caller guarantees that this does not overflow
    let power = unsafe { 1_usize.unchecked_shl(to_u32(exponent)) };
    // SAFETY: `power >= 1`
    unsafe { power.unchecked_sub(1) }
}

/// Rounds `n`  up to the multiple of the power of 2 specified by `EXPONENT`
///
/// Returns `None` if this would overflow
const fn round_up_power_of_2<const EXPONENT: u8>(n: NonZeroUsize) -> Option<NonZeroUsize> {
    if to_u32(EXPONENT) >= usize::BITS {
        return None;
    }

    // SAFETY: From above, this cannot overflow
    let mask = unsafe { exponent_minus_1(EXPONENT) };
    if let Some(added) = n.checked_add(mask) {
        // SAFETY: `n >= 1` so `added >= power`, and thus at least one bit remains set after this
        // masking operation, leaving a nonzero value
        Some(unsafe { NonZeroUsize::new_unchecked(added.get() & !mask) })
    } else {
        None
    }
}

/// Returns true if `n` is a multiple of the power of 2 specified by `EXPONENT`
///
/// # Safety
///
/// `exponent` must be less than `usize::BITS`
const fn is_multiple_power_of_2(n: NonZeroUsize, exponent: u8) -> bool {
    debug_assert!(
        to_u32(exponent) < usize::BITS,
        "Exponent must be less than the number of bits in a `usize`"
    );
    // SAFETY: The caller guarantees that this does not overflow
    n.get() & unsafe { exponent_minus_1(exponent) } == 0
}

/// Returns the largest power of 2 that divides `n`
const fn largest_factor_power_of_2(n: NonZeroUsize) -> NonZeroUsize {
    // SAFETY: `n.trailing_zeros() < NonZeroUsize::BITS`
    let computation = unsafe { 1_usize.unchecked_shl(n.trailing_zeros()) };
    // SAFETY: `computation >= 1`
    unsafe { NonZeroUsize::new_unchecked(computation) }
}

/// Returns a pointer to the buddy of the current block
///
/// # Safety
///
/// `log_size` must be less than `usize::BITS`
unsafe fn buddy_of(block: NonNull<Block>, log_size: u8) -> *mut Block {
    debug_assert!(
        to_u32(log_size) < usize::BITS,
        "`log_size` must be less than `usize::BITS`"
    );
    // SAFETY: The caller guarantees that this shift does not overflow
    block
        .as_ptr()
        // SAFETY: The caller guarantees that this cannot overflow
        .with_addr(block.addr().get() ^ unsafe { 1_usize.unchecked_shl(log_size.into()) })
}

/// A free block
pub struct Block {
    /// The base-2 logarithm of this block's size
    log_size: u8,
    /// Pointer to the next block in the list
    next: NonNull<Block>,
}

/// A constant `Block` suitbble as the tail of the `OrderedBuddyMap`
static TAIL: Block = Block {
    next: NonNull::dangling(),
    log_size: u8::MAX,
};

// SAFETY: It is OK to have multiple shared references to a block across threads, as long as no
// mutation occurs
unsafe impl Sync for Block {}

/// The minimum supported size of a block
#[expect(
    clippy::unwrap_used,
    reason = "Any mistake here is caught at compile time"
)]
const MIN_SIZE: NonZeroUsize =
    NonZeroUsize::new(mem::size_of::<Block>().next_power_of_two()).unwrap();
/// Base 2 logarithm of the minimum supported block size
const LOG_MIN_SIZE: u8 = ilog2_u8(MIN_SIZE);

/// An ordered map suitable for storing blocks in a buddy allocator
pub(super) struct OrderedBuddyMap {
    /// The head of the internal linked list
    head: Block,
}

impl OrderedBuddyMap {
    /// Attempts to remove the buddy from the set, or inserts the node if the buddy is not in the
    /// set
    /// Returns true if the *insertion* is successful - i.e., the buddy is *not* in the set
    ///
    /// # Safety
    ///
    /// * The node must be uniquely pointed to by this function - it cannot be referenced anywhere
    /// else
    /// * The node must be valid for reads and writes
    pub(super) unsafe fn remove_buddy_or_insert_recursive(
        &mut self,
        mut node: NonNull<Block>,
        mut log_size: u8,
    ) {
        assert!(
            is_multiple_power_of_2(node.addr(), log_size),
            "Blocks should be aligned to their size"
        );
        assert!(
            log_size >= LOG_MIN_SIZE,
            "Size of a block is smaller than supported"
        );
        // SAFETY: This reference is valid for the duration of the function execution
        let node_ref = unsafe { node.as_mut() };
        // SAFETY: The caller ensures that this is a valid block size, and so cannot overflow
        let mut buddy = unsafe { buddy_of(node, log_size) };
        let mut prev = &mut self.head;

        loop {
            let mut next_ptr = prev.next;
            // SAFETY: These references are valid for the duration of the function execution
            let next = unsafe { next_ptr.as_mut() };

            if next.log_size == log_size && next_ptr.as_ptr() == buddy {
                // Found the buddy; cut it out, and move up one size
                prev.next = next.next;
                // SAFETY: This cannot overflow since the valid block size fits into a `usize`
                // which has less than 256 bits
                let mask = !unsafe { 1_usize.unchecked_shl(log_size.into()) };
                // SAFETY: Neither the node nor the buddy can be null pointers if they are valid
                // heap allocations
                node = unsafe { NonNull::new_unchecked(node.as_ptr().mask(mask)) };
                // SAFETY: See above
                buddy = unsafe { buddy_of(node.cast(), log_size) }.cast();
                // SAFETY: See above
                log_size = unsafe { log_size.unchecked_add(1) };
            } else if next.log_size > log_size
                || (next.log_size == log_size && next_ptr.as_ptr() > buddy)
            {
                // Went past the buddy; insert this block and end, since there is no more merging possible
                *node_ref = Block {
                    next: next_ptr,
                    log_size,
                };
                prev.next = node;
                break;
            } else {
                // Normal iteration
                prev = next;
            }
        }
    }

    /// Pops an arbitrary node from the set of the given size
    ///
    /// Returns None if the set is empty
    pub(super) fn pop(&mut self, mut log_size: u8) -> Option<NonNull<Block>> {
        log_size = log_size.max(LOG_MIN_SIZE);
        let mut prev = &mut self.head;

        loop {
            let mut next_ptr = prev.next;
            // SAFETY: These references are valid for the duration of the function invocation
            let next = unsafe { next_ptr.as_mut() };

            // If we hit the tail, no suitable blocks are available
            if next_ptr == (&TAIL).into() {
                return None;
            }

            if next.log_size >= log_size {
                // Found a block of suitable size
                // Since no blocks exist from `log_size` to `log_size` before this, we can split
                // the block in place in the list and still preserve sorted order.
                prev.next = next.next;
                for size in log_size..next.log_size {
                    let mut buddy_ptr = next_ptr
                        // SAFETY: Because `size` is a valid block size, this cannot overflow
                        .map_addr(|addr| addr | unsafe { 1_usize.unchecked_shl(size.into()) });
                    // SAFETY: The buddy block is exclusively owned for the duration of this
                    // function invocation
                    let buddy = unsafe { buddy_ptr.as_mut() };
                    prev.next = buddy_ptr;
                    buddy.next = next.next;
                    prev = buddy;
                }
                return Some(next_ptr);
            }

            prev = next;
        }
    }

    /// Initializes the map with the range of bytes, beginning at `start` and of length `size`, as
    /// the available region. Not all bytes are guaranteed to be used.
    ///
    /// # Safety
    ///
    /// * The specified region must be valid for reads and writes
    /// * The specified region must not be used by anything else
    pub(super) unsafe fn new(start_region: NonNull<()>, mut size: usize) -> Self {
        let start = start_region.cast::<Block>();
        assert!(
            // SAFETY: `usize::MAX > size`
            start.addr().get() <= unsafe { usize::MAX.unchecked_sub(size) },
            "The provided range should not wrap"
        );
        let mut map = Self {
            head: Block {
                log_size: u8::MIN,
                next: (&TAIL).into(),
            },
        }; /*
           if let Some(next_block_addr) = round_up_power_of_2::<LOG_MIN_SIZE>(start.addr()) {
               let mut next_block = start.with_addr(next_block_addr);
               size =
               // SAFETY: `next_block_addr >= start.addr()`
               size.saturating_sub(unsafe { next_block_addr.get().unchecked_sub(start.addr().get()) });

               while let Some(nonzero_size) = NonZeroUsize::new(size) {
                   let capacity = prev_power_of_2(nonzero_size);
                   let alignment = largest_factor_power_of_2(next_block.addr());
                   let block_size = capacity.min(alignment);
                   if block_size < MIN_SIZE {
                       break;
                   }
                   // SAFETY: We correctly formed this block from the region given to us by the caller
                   unsafe {
                       map.remove_buddy_or_insert_recursive(next_block, ilog2_u8(block_size));
                   };

                   next_block =
                       next_block.map_addr(|old| unsafe { old.unchecked_add(block_size.into()) });
                   size = unsafe { size.unchecked_sub(block_size.into()) };
               }
           }*/
        map
    }
}

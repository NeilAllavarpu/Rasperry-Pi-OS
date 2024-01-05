use core::{mem, num};

use bitvec::{
    ptr::BitPtr,
    slice::{self, BitSlice},
};

type BackingType = u64;

pub struct BitMap<'a, const MIN_BLOCK_SIZE: u8, const MAX_BLOCK_SIZE: u8> {
    num_small_blocks: usize,
    usage: &'a mut BitSlice<BackingType>,
}

#[allow(clippy::unwrap_in_result)]
impl<'a, const MIN_BLOCK_SIZE: u8, const MAX_BLOCK_SIZE: u8>
    BitMap<'a, MIN_BLOCK_SIZE, MAX_BLOCK_SIZE>
{
    /// Creates a fully-deallocated bitmap from a slice
    pub fn from_slice(slice: &'a mut [BackingType], size: usize) -> Self {
        let usage = BitSlice::from_slice_mut(slice);
        let num_small_blocks = size / (1 << MIN_BLOCK_SIZE);
        usage.fill(true);

        let mut bitmap = Self {
            num_small_blocks,
            usage,
        };

        assert!(bitmap.deallocate(0, MAX_BLOCK_SIZE));

        bitmap
    }

    fn slice_for_level(&mut self, level: u8) -> Option<&mut BitSlice<BackingType>> {
        let mut index = 0;
        let mut num_at_level = self.num_small_blocks;
        for _ in 0..level {
            index += num_at_level;
            num_at_level /= 2;
        }

        self.usage.get_mut(index..num_at_level)
    }

    /// Returns an index corresponding to an allocation suitably sized
    pub fn allocate_any(&mut self, log_size: u8) -> Option<usize> {
        if log_size > MAX_BLOCK_SIZE {
            return None;
        }
        let log_size = log_size.max(MIN_BLOCK_SIZE);
        let level = log_size - MIN_BLOCK_SIZE;

        if let Some(free) = self.slice_for_level(0).unwrap().first_zero() {
            *self.slice_for_level(0).unwrap().get_mut(free).unwrap() = true;
            Some(free)
        } else if let Some(bigger_free) = self.allocate_any(log_size + 1) {
            *self
                .slice_for_level(level)
                .unwrap()
                .get_mut(bigger_free * 2 + 1)
                .unwrap() = false;
            Some(bigger_free * 2)
        } else {
            None
        }
    }

    /// p
    pub fn deallocate(&mut self, index: usize, log_size: u8) -> bool {
        assert!(MIN_BLOCK_SIZE <= log_size && log_size <= MAX_BLOCK_SIZE);

        let level = log_size - MIN_BLOCK_SIZE;

        let bits = self.slice_for_level(level).unwrap();
        if bits[index ^ 0x1] {
            // in use
            *bits.get_mut(index).unwrap() = true;
            true
        } else {
            // not in use, percolate up
            *bits.get_mut(index ^ 0x1).unwrap() = true;
            self.deallocate(index, log_size)
        }
    }
}

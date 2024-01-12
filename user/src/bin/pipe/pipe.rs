use alloc::collections::{vec_deque::Drain, VecDeque};
use user::println;

/// Integer type representing an ID for a pipe via a message
pub type PipeId = u16;

/// The shared component of a pipe. Buffers data from writers until readers remove said data.
pub struct Pipe {
    buffer: VecDeque<u8>,
}

impl Pipe {
    /// Creates a new, empty pipe
    pub const fn new() -> Self {
        Self {
            buffer: VecDeque::new(),
        }
    }

    /// Reads up to `max_count` bytes from the pipe, or less if less are available
    pub fn read(&mut self, max_count: usize) -> Drain<u8> {
        let count = max_count.min(self.buffer.len());
        self.buffer.drain(0..count)
    }

    /// Writes all the given `bytes` into the pipe
    pub fn write(&mut self, bytes: impl Iterator<Item = u8>) {
        self.buffer.extend(bytes);
        println!("buffer is now {:X?}", self.buffer);
    }
}

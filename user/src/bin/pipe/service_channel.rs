use alloc::boxed::Box;
use core::{
    iter,
    sync::atomic::{AtomicU8, Ordering},
};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use crate::{pipe::PipeId, process::DropError};

const PAGE_SIZE: usize = 1 << 16;

#[repr(C, align(32768))]
struct Buffer([AtomicU8; PAGE_SIZE / 2]);

impl Buffer {
    fn read_byte(&self, index: usize) -> u8 {
        self.0[index % self.0.len()].load(Ordering::Relaxed)
    }

    fn write_byte(&mut self, index: usize, value: u8) {
        self.0[index % self.0.len()].store(value, Ordering::Relaxed)
    }
}

#[derive(FromPrimitive)]
pub enum MessageKind {
    None = 0,
    Read = 1,
    Write = 2,
    Fork = 3,
    Create = 4,
    DropRead = 5,
    DropWrite = 6,
}

pub struct ReadBufferStream<'a>(&'a Buffer, usize);
pub struct WriteBufferStream<'a>(&'a mut Buffer, usize);

impl ReadBufferStream<'_> {
    /// Reads a single byte from the buffer and advances the pointer by 1
    fn read_byte(&mut self) -> u8 {
        let value = self.0.read_byte(self.1);
        self.1 = self.1.wrapping_add(1);
        value
    }

    /// Reads `N` bytes from the buffer, and advances the pointer accordingly
    fn read_bytes<const N: usize>(&mut self) -> [u8; N] {
        let mut bytes = [0; N];
        bytes.fill_with(|| self.read_byte());
        bytes
    }

    /// Reads a single `u16` from the stream and advances the pointer by 2
    fn read_u16(&mut self) -> u16 {
        u16::from_ne_bytes(self.read_bytes())
    }

    fn read_pipe_id(&mut self) -> PipeId {
        PipeId::from_ne_bytes(self.read_bytes())
    }

    /// Moves the pointer back one, e.g. to undo a read of a bad value
    fn back(&mut self) {
        self.1 = self.1.wrapping_sub(1);
    }

    /// Reads a message from the incoming buffer, if any are available
    pub fn read_message(&mut self) -> Option<Request> {
        let message_kind = self.read_byte();
        match FromPrimitive::from_u8(message_kind) {
            None | Some(MessageKind::None) => {
                self.back();
                None
            }
            Some(MessageKind::Read) => {
                let pipe_id = self.read_pipe_id();
                let length = usize::from(self.read_u16());
                Some(Request::Read(pipe_id, length))
            }
            Some(MessageKind::Write) => {
                let pipe_id = self.read_pipe_id();
                let length = usize::from(self.read_u16());
                let bytes = iter::repeat_with(|| self.read_byte()).take(length);
                Some(Request::Write(pipe_id, bytes.collect()))
            }
            Some(MessageKind::Fork) => {
                let target_pid = self.read_u16();
                Some(Request::Fork(target_pid))
            }
            Some(MessageKind::Create) => Some(Request::Create),
            Some(MessageKind::DropRead) => {
                let pipe_id = self.read_pipe_id();
                Some(Request::DropRead(pipe_id))
            }
            Some(MessageKind::DropWrite) => {
                let pipe_id = self.read_pipe_id();
                Some(Request::DropWrite(pipe_id))
            }
        }
    }
}

impl WriteBufferStream<'_> {
    fn write_byte(&mut self, value: u8) {
        self.0.write_byte(self.1, value);
        self.1 = self.1.wrapping_add(1);
    }

    fn write_bytes(&mut self, value: impl Iterator<Item = u8>) {
        for byte in value {
            self.write_byte(byte);
        }
    }

    /// Moves the pointer back one, e.g. to undo a read of a bad value
    fn back(&mut self) {
        self.1 = self.1.wrapping_sub(1);
    }

    /// Writes a message to the outgoing buffer
    #[expect(clippy::as_conversions)]
    pub fn write_message<T: ExactSizeIterator + Iterator<Item = u8>>(
        &mut self,
        response: Response<T>,
    ) {
        match response {
            Response::Read(bytes) => {
                self.write_byte(MessageKind::Read as u8);
                self.write_bytes(
                    u16::try_from(bytes.size_hint().0)
                        .expect("Number of written bits should be less than 2^16")
                        .to_ne_bytes()
                        .iter()
                        .copied(),
                );
                self.write_bytes(bytes);
            }
            Response::ReadFailure(_) => todo!(),
            Response::Write => self.write_byte(MessageKind::Write as u8),
            Response::WriteFailure(_) => todo!(),
            Response::Fork => self.write_byte(MessageKind::Fork as u8),
            Response::ForkFailure => todo!(),
            Response::Create(pipe_id) => {
                self.write_byte(MessageKind::Create as u8);
                self.write_bytes(pipe_id.to_ne_bytes().iter().copied());
            }
            Response::CreateFailure => todo!(),
            Response::DropRead => self.write_byte(MessageKind::DropRead as u8),
            Response::DropReadFailure(_) => todo!(),
            Response::DropWrite => self.write_byte(MessageKind::DropWrite as u8),
            Response::DropWriteFailure(_) => todo!(),
        }
        self.write_byte(MessageKind::None as u8);
        self.back();
    }
}

pub struct Channel<'a> {
    pub incoming: ReadBufferStream<'a>,
    pub outgoing: WriteBufferStream<'a>,
    page: u64,
}

pub enum Request {
    Read(PipeId, usize),
    Write(PipeId, Box<[u8]>),
    Fork(u16),
    Create,
    DropRead(PipeId),
    DropWrite(PipeId),
}

#[derive(Clone, Copy)]
#[repr(u8)]
pub enum ReadError {
    NoSuchPipe = 0,
    InsufficientPermissions = 1,
    Locked = 2,
}

#[derive(Clone, Copy)]
#[repr(u8)]
pub enum WriteError {
    NoSuchPipe = 0,
    InsufficientPermissions = 1,
    Locked = 2,
}

pub enum Response<T: ExactSizeIterator + Iterator<Item = u8>> {
    Read(T),
    ReadFailure(ReadError),
    Write,
    WriteFailure(WriteError),
    Fork,
    ForkFailure,
    Create(u16),
    CreateFailure,
    DropRead,
    DropReadFailure(DropError),
    DropWrite,
    DropWriteFailure(DropError),
}

impl Drop for Channel<'_> {
    fn drop(&mut self) {
        todo!("dealloc page: {}", self.page);
    }
}

impl Channel<'_> {}

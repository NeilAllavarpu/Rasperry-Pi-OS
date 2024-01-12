use alloc::{sync::Arc, vec::Vec};
use user::{pid_map::U16Map, sync::SpinLock};

use crate::{
    pipe::{Pipe, PipeId},
    service_channel::Channel,
};

pub static PROCESSES: SpinLock<U16Map<ProcessState>> = SpinLock::new(U16Map::new());

#[derive(Clone)]
struct PipeInfo {
    pipe: Arc<SpinLock<Pipe>>,
    readable: bool,
    writable: bool,
}

pub struct ProcessState<'a> {
    pipes: U16Map<PipeInfo>,
    pub channel: Channel<'a>,
}

pub enum CreateError {
    MaxPipeCount,
    NoMemory,
}

pub enum DropError {
    NoPermissions,
    NoSuchPipe,
}

impl<'a> ProcessState<'a> {
    pub const fn new_with_channel(channel: Channel<'a>) -> Self {
        Self {
            pipes: U16Map::new(),
            channel,
        }
    }

    pub fn clone_with_channel(&self, channel: Channel<'a>) -> Self {
        Self {
            pipes: self.pipes.clone(),
            channel,
        }
    }

    /// Returns a reference to the given pipe if this process has permission to read from it
    pub fn get_read(&self, pipe_id: PipeId) -> Option<&Arc<SpinLock<Pipe>>> {
        self.pipes
            .get(pipe_id)
            .filter(|info| info.readable)
            .map(|info| &info.pipe)
    }

    /// Returns a reference to the given pipe if this process has permission to write to it
    pub fn get_write(&self, pipe_id: PipeId) -> Option<&Arc<SpinLock<Pipe>>> {
        self.pipes
            .get(pipe_id)
            .filter(|info| info.writable)
            .map(|info| &info.pipe)
    }

    /// Creates a new pipe that this process can read and write to
    pub fn create_pipe(&mut self) -> Result<PipeId, CreateError> {
        Arc::try_new(SpinLock::new(Pipe::new()))
            .map_err(|_err| CreateError::NoMemory)
            .and_then(|pipe| {
                self.pipes
                    .insert_lowest(PipeInfo {
                        pipe,
                        readable: true,
                        writable: true,
                    })
                    .ok_or(CreateError::MaxPipeCount)
            })
    }

    /// Drops read permissions from a pipe. Drops the pipe if it is no longer accessible
    pub fn drop_read(&mut self, pipe_id: PipeId) -> Result<(), DropError> {
        let pipe = self.pipes.get_mut(pipe_id).ok_or(DropError::NoSuchPipe)?;
        if pipe.readable {
            pipe.readable = false;
            if !pipe.writable {
                self.pipes.set(pipe_id, None).unwrap();
            }
            Ok(())
        } else {
            Err(DropError::NoPermissions)
        }
    }

    /// Drops write permissions from a pipe. Drops the pipe if it is no longer accessible
    pub fn drop_write(&mut self, pipe_id: PipeId) -> Result<(), DropError> {
        let pipe = self.pipes.get_mut(pipe_id).ok_or(DropError::NoSuchPipe)?;
        if pipe.writable {
            pipe.writable = false;
            if !pipe.readable {
                self.pipes.set(pipe_id, None).unwrap();
            }
            Ok(())
        } else {
            Err(DropError::NoPermissions)
        }
    }
}

use super::{Execution, UserContext};
use alloc::vec::Vec;

pub struct ExecutionMap(Vec<Option<Execution>>);

#[derive(Debug)]
pub enum ForkError {
    NoMem,
    NoPid,
    SrcNotValid,
}

impl ExecutionMap {
    /// Creates a new, unpopulated `ExecutionMap`
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    /// Returns an `Ok` with an unused PID with space already allocated, else returns `Err` with a pid corresponding to one past the end of the current allocation (i.e., can be reached with a single `push`)
    fn find_available_pid(&self) -> Result<u16, u16> {
        self.0
            .iter()
            .enumerate()
            .find_map(|(id, arc)| arc.is_none().then_some(id))
            .map(|pid| u16::try_from(pid).expect("PID should be at most 16 bits"))
            .ok_or_else(|| self.0.len().try_into().unwrap())
    }

    /// Creates an execution with the given information, and defaults for all other values, at the lowest available PID
    pub fn create(&mut self, tcr_el1: u64, ttbr0: u64, user_context: *const UserContext) -> u16 {
        match self.find_available_pid() {
            Ok(pid) => {
                self.0[usize::from(pid)] = Some(Execution::new(tcr_el1, ttbr0, user_context, pid));
                pid
            }
            Err(pid) => {
                self.0
                    .push(Some(Execution::new(tcr_el1, ttbr0, user_context, pid)));
                pid
            }
        }
    }

    /// Returns the execution corresponding to the given PID, if present
    pub fn get(&self, pid: u16) -> Option<&Execution> {
        self.0.get(usize::from(pid)).and_then(Option::as_ref)
    }

    /// Removes and returns the execution correspodning to the given PID, if present
    pub fn remove(&mut self, pid: u16) -> Option<Execution> {
        self.0.get_mut(usize::from(pid)).and_then(Option::take)
    }

    /// Duplicates the execution at `src_pid` into the next available pid
    pub fn fork(&mut self, src_pid: u16) -> Result<u16, ForkError> {
        let src_exec = self.get(src_pid).ok_or(ForkError::SrcNotValid)?;
        match self.find_available_pid() {
            Ok(pid) => {
                let mut new_execution = src_exec.clone();
                new_execution.pid = pid;
                self.0[usize::from(pid)] = Some(new_execution);
                Ok(pid)
            }
            Err(pid) => {
                let mut new_execution = src_exec.clone();
                new_execution.pid = pid;
                self.0.push(Some(new_execution));
                Ok(pid)
            }
        }
    }
}

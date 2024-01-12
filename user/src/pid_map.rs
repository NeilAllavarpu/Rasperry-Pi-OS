use core::{iter, mem};

use alloc::vec::Vec;

#[derive(Clone)]
pub struct U16Map<T>(Vec<Option<T>>);

impl<T> U16Map<T> {
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    pub fn get(&self, pid: u16) -> Option<&T> {
        self.0.get(usize::from(pid)).and_then(Option::as_ref)
    }

    pub fn get_mut(&mut self, pid: u16) -> Option<&mut T> {
        self.0.get_mut(usize::from(pid)).and_then(Option::as_mut)
    }

    pub fn set(&mut self, pid: u16, value: Option<T>) -> Option<T> {
        let pid = usize::from(pid);
        self.0
            .extend(iter::repeat_with(|| None).take(pid.saturating_sub(self.0.len())));
        mem::replace(
            self.0
                .get_mut(pid)
                .expect("Should be extended to the proper length"),
            value,
        )
    }

    pub fn insert_lowest(&mut self, value: T) -> Option<u16> {
        self.0
            .iter()
            .enumerate()
            .find_map(|(index, option)| {
                option.is_none().then_some(
                    u16::try_from(index).expect("u16 map should not have more than 2^16 elements"),
                )
            })
            .or_else(|| u16::try_from(self.0.len()).ok())
            .map(|pid| {
                assert!(self.set(pid, Some(value)).is_none());
                pid
            })
    }
}

use core::mem;

use alloc::{sync::Arc, vec::Vec};

use super::{Execution, UserContext};

pub struct ExecutionMap(Vec<Option<Arc<Execution>>>);

impl ExecutionMap {
    pub const fn new() -> Self {
        Self(Vec::new())
    }

    pub fn create(
        &mut self,
        tcr_el1: u64,
        ttbr0: u64,
        user_context: *const UserContext,
    ) -> Arc<Execution> {
        let pid = self
            .0
            .iter()
            .enumerate()
            .find_map(|(id, arc)| arc.is_none().then_some(id));
        if let Some(pid) = pid {
            let arc = Arc::new(Execution::new(
                tcr_el1,
                ttbr0,
                user_context,
                u16::try_from(pid).unwrap(),
            ));
            self.0[pid] = Some(Arc::clone(&arc));
            arc
        } else {
            let pid = self.0.len();
            let arc = Arc::new(Execution::new(
                tcr_el1,
                ttbr0,
                user_context,
                u16::try_from(pid).unwrap(),
            ));
            self.0.push(Some(Arc::clone(&arc)));
            arc
        }
    }

    pub fn get(&self, pid: u16) -> Option<&Arc<Execution>> {
        self.0.get(usize::from(pid)).and_then(Option::as_ref)
    }

    pub fn remove(&mut self, pid: u16) -> Option<Arc<Execution>> {
        self.0.get_mut(usize::from(pid)).and_then(Option::take)
    }
}

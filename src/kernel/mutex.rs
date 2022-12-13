/// Guarantess single-access of the enclosed data
pub trait Mutex {
    /// The type of state that is wrapped by this mutex.
    type State;

    /// Locks the mutex and grants the closure temporary mutable access to the inner state
    fn lock<'a, R>(&'a self, f: impl FnOnce(&'a mut Self::State) -> R) -> R;
}

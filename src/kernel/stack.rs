use core::sync::atomic::{AtomicPtr, Ordering};

/// Trait for items that can be put into `Stack`s or `BoxStack`s
pub trait Stackable {
    /// Sets the next pointer, when in the stack
    /// Undefined behavior if called manually
    /// # Safety
    /// Only the internal stack implementation should call this function
    unsafe fn set_next(&mut self, next: *mut Self);

    /// Reads the next pointer, when in the stack
    /// The value is only valid when in the stack
    fn read_next(&self) -> *mut Self;
}

/// A lock-free thread-safe linked-list intrusive stack
/// DOES NOT DEAL PROPERLY WITH DROPPING
pub struct Stack<T: Stackable> {
    /// The top of the stack
    top: AtomicPtr<T>,
}

impl<T: Stackable> Stack<T> {
    /// Creates a new, empty stack
    pub const fn new() -> Self {
        Self {
            top: AtomicPtr::new(core::ptr::null_mut()),
        }
    }

    /// Adds an element to the top of the stack
    pub fn push(&self, value: &mut T) {
        let mut top_ptr = self.top.load(Ordering::Relaxed);
        loop {
            // SAFETY: This is the only valid place to use this method
            unsafe { value.set_next(top_ptr) }
            let previous_top = self.top.compare_exchange_weak(
                top_ptr,
                value,
                Ordering::Release,
                Ordering::Acquire,
            );

            if let Err(next_top) = previous_top {
                top_ptr = next_top;
            } else {
                break;
            }
        }
    }

    /// Removes the first element from the top of the stack
    pub fn pop(&self) -> Option<&mut T> {
        let mut top = self.top.load(Ordering::Acquire);
        loop {
            // SAFETY: Either `top_ptr` is null, or this points to a valid T as set by `push`
            if let Some(previous_top) = unsafe { top.as_mut() } {
                let exchange_result = self.top.compare_exchange_weak(
                    top,
                    previous_top.read_next(),
                    Ordering::Relaxed,
                    Ordering::Acquire,
                );

                if let Err(next_top) = exchange_result {
                    top = next_top;
                } else {
                    return Some(previous_top);
                }
            } else {
                return None;
            }
        }
    }

    /// Computes the current depth of the the stack, for logging purposes
    /// Not thread safe, or perfectly accurate
    ///
    /// # Safety
    /// Use *only* for logging purposes
    pub unsafe fn depth(&self) -> usize {
        let mut ptr = self.top.load(Ordering::Acquire);
        let mut depth: usize = 0;
        // SAFETY: `ptr` is obtained from the existing stack list,
        // and must be valid via `push`
        while let Some(element) = unsafe { ptr.as_ref() } {
            depth += 1;
            ptr = element.read_next();
        }
        depth
    }
}

/// SAFETY: By construction, these stacks are thread-safe
unsafe impl<T: Stackable> Send for Stack<T> {}
/// SAFETY: By construction, these stacks are thread-safe
unsafe impl<T: Stackable> Sync for Stack<T> {}

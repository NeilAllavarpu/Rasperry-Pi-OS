use alloc::boxed::Box;
use core::sync::atomic::{AtomicPtr, Ordering};

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
            unsafe { value.set_next(top_ptr) }
            let previous_top = self.top.compare_exchange_weak(
                top_ptr,
                value,
                Ordering::Relaxed,
                Ordering::Relaxed,
            );

            if previous_top.is_ok() {
                break;
            }

            top_ptr = previous_top.unwrap_err();
        }
    }

    /// Removes the first element from the top of the stack
    pub fn pop(&self) -> Option<&mut T> {
        let mut top_ptr = self.top.load(Ordering::Relaxed);
        loop {
            if top_ptr.is_null() {
                return None;
            }

            let previous_top = self.top.compare_exchange_weak(
                top_ptr,
                // Assumption: Since `top_ptr` is not null,
                // this must point to a valid T
                // as pushed into the stack by `push`
                unsafe { (*top_ptr).read_next() },
                Ordering::Relaxed,
                Ordering::Relaxed,
            );

            if previous_top.is_ok() {
                return Some(unsafe { &mut *top_ptr });
            }

            top_ptr = previous_top.unwrap_err();
        }
    }

    /// Computes the current depth of the the stack, for logging purposes
    /// Not thread safe, or perfectly accurate
    ///
    /// # Safety
    /// Use *only* for logging purposes
    pub unsafe fn depth(&self) -> usize {
        let mut ptr = self.top.load(Ordering::Relaxed);
        let mut depth: usize = 0;
        while !ptr.is_null() {
            depth += 1;
            ptr = unsafe { (*ptr).read_next() }
        }
        depth
    }
}

unsafe impl<T: Stackable> Send for Stack<T> {}
unsafe impl<T: Stackable> Sync for Stack<T> {}

/// Stack which contains boxed values
pub struct BoxStack<T: Stackable> {
    stack: Stack<T>,
}

#[allow(dead_code)]
impl<T: Stackable> BoxStack<T> {
    /// Creates a new, empty stack
    pub const fn new() -> Self {
        Self {
            stack: Stack::new(),
        }
    }

    /// Adds an element to the top of the stack
    pub fn push(&self, value: Box<T>) {
        self.stack.push(unsafe { &mut *Box::into_raw(value) })
    }

    /// Removes the first element from the top of the stack
    pub fn pop(&self) -> Option<Box<T>> {
        self.stack
            .pop()
            .map(|value| unsafe { Box::from_raw(value) })
    }

    /// Computes the current depth of the the stack, for logging purposes
    /// Not thread safe, or perfectly accurate
    ///
    /// # Safety
    /// Use *only* for logging purposes
    pub unsafe fn depth(&self) -> usize {
        unsafe { self.stack.depth() }
    }
}

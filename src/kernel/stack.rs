use core::{
    marker::PhantomData,
    sync::atomic::{AtomicU128, Ordering},
};

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
///
/// DOES NOT DEAL PROPERLY WITH DROPPING
pub struct Stack<T: Stackable> {
    /// The top of the stack + a counter to address ABA problems
    top_and_counter: AtomicU128,
    /// Marker for the type
    phantom: PhantomData<T>,
}

impl<T: Stackable> Stack<T> {
    /// Extracts the top pointer and the counter separately, from a combined u128
    fn extract_parts(top_and_counter: u128) -> (*mut T, u64) {
        (
            <*mut T>::from_bits(
                (top_and_counter & ((1 << 64) - 1))
                    .try_into()
                    .expect("Top pointer should not overflow"),
            ),
            (top_and_counter >> 64)
                .try_into()
                .expect("Counter should not overflow"),
        )
    }

    /// Combines a top pointer and a counter into a u128
    fn combine_parts(top: *mut T, counter: u64) -> u128 {
        u128::try_from(top.to_bits()).expect("Top pointer should fit into 128 bits")
            | (u128::from(counter) << 64)
    }

    /// Creates a new, empty stack
    pub const fn new() -> Self {
        Self {
            top_and_counter: AtomicU128::new(0),
            phantom: PhantomData,
        }
    }

    /// Adds an element to the top of the stack
    pub fn push(&self, value: &mut T) {
        let (mut top, mut counter) =
            Self::extract_parts(self.top_and_counter.load(Ordering::Acquire));
        // SAFETY: This is the only valid place to use this method
        unsafe { value.set_next(top) }
        while let Err(next_top_and_counter) = self.top_and_counter.compare_exchange_weak(
            Self::combine_parts(top, counter),
            Self::combine_parts(value, counter + 1),
            Ordering::Release,
            Ordering::Acquire,
        ) {
            let (top_, counter_) = Self::extract_parts(next_top_and_counter);
            top = top_;
            counter = counter_;
            // SAFETY: This is the only valid place to use this method
            unsafe { value.set_next(top) }
        }
    }

    /// Removes the first element from the top of the stack
    pub fn pop(&self) -> Option<&mut T> {
        let (mut top, mut counter) =
            Self::extract_parts(self.top_and_counter.load(Ordering::Acquire));
        // SAFETY: Either `top_ptr` is null, or this points to a valid T as set by `push`
        while let Some(previous_top) = unsafe { top.as_mut() } {
            if let Err(next_top_and_counter) = self.top_and_counter.compare_exchange_weak(
                Self::combine_parts(top, counter),
                Self::combine_parts(previous_top.read_next(), counter),
                Ordering::Relaxed,
                Ordering::Acquire,
            ) {
                let (top_, counter_) = Self::extract_parts(next_top_and_counter);
                top = top_;
                counter = counter_;
            } else {
                return Some(previous_top);
            }
        }
        None
    }

    /// Computes the current depth of the the stack, for logging purposes
    /// Not thread safe, or perfectly accurate
    ///
    /// # Safety
    /// Use *only* for logging purposes
    pub unsafe fn depth(&self) -> usize {
        let (mut ptr, _) = Self::extract_parts(self.top_and_counter.load(Ordering::Acquire));
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

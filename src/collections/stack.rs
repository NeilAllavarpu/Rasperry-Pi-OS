use crate::sync::AtomicStampedPtr;
use alloc::sync::Arc;
use core::sync::atomic::Ordering;

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
#[allow(clippy::module_name_repetitions)]
pub struct UnsafeStack<T: Stackable> {
    /// The top of the stack + a stamp to address ABA problems
    top: AtomicStampedPtr<T>,
}

impl<T: Stackable> UnsafeStack<T> {
    /// Creates a new, empty stack
    pub const fn new() -> Self {
        Self {
            top: AtomicStampedPtr::default(),
        }
    }

    /// Adds an element to the top of the stack
    /// # Safety
    /// `value` must point to a pinned object that will not be deallocated
    pub unsafe fn push(&self, value: *mut T) {
        self.top
            .fetch_update_unstamped(Ordering::Release, Ordering::Acquire, |top| {
                // SAFETY: This is the only valid place to use this method
                // By assumption, `value` is valid
                unsafe { (*value).set_next(top) };
                Some(value)
            })
            .expect("Should never return `None`");
    }

    /// Removes the first element from the top of the stack
    pub fn pop(&self) -> Option<*mut T> {
        self.top
            .fetch_update_stamped(Ordering::Relaxed, Ordering::Acquire, |top, stamp| {
                // SAFETY: Either `top_ptr` is null, or this points to a valid T as set by `push`
                unsafe { top.as_ref() }.map(|top_ref| (top_ref.read_next(), stamp + 1))
            })
            .ok()
            .map(|(top, _)| top)
    }

    /// Computes the current depth of the the stack, for logging purposes
    /// Not thread safe, or perfectly accurate
    ///
    /// # Safety
    /// Use *only* for logging purposes
    pub unsafe fn depth(&self) -> usize {
        let mut ptr = self.top.load_unstamped(Ordering::Acquire);
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
unsafe impl<T: Stackable> Send for UnsafeStack<T> {}
/// SAFETY: By construction, these stacks are thread-safe
unsafe impl<T: Stackable> Sync for UnsafeStack<T> {}

/// A lock-free intrusive stack of `Arc`s
pub struct ArcStack<T: Stackable>(UnsafeStack<T>);

impl<T: Stackable> ArcStack<T> {
    pub const fn new() -> Self {
        Self(UnsafeStack::new())
    }

    /// Adds an `Arc` to the top of the stack
    pub fn push(&self, value: Arc<T>) {
        // SAFETY: `Arc`s are pinned into memory, and since this holds a strong
        // pointer the underlying allocation is not freed
        unsafe {
            self.0.push(Arc::into_raw(value).cast_mut());
        }
    }

    // Removes the first `Arc` from the top of the stack
    pub fn pop(&self) -> Option<Arc<T>> {
        // SAFETY: The `pop`ped pointer came from an `Arc::into_raw` via `push`
        self.0.pop().map(|arc| unsafe { Arc::from_raw(arc) })
    }
}

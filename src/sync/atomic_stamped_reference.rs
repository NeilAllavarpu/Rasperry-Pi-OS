use core::sync::atomic::{AtomicPtr, Ordering};

/// An `AtomicStampedPtr` maintains an object pointer along with an integer "stamp", that can be updated atomically.
pub struct AtomicStampedPtr<T>(AtomicPtr<T>);

#[cfg(target_pointer_width = "64")]
impl<T> AtomicStampedPtr<T> {
    // TCR_EL1 is configured to allow the top byte to be used for tagging
    /// The number of bits for the pointer address
    const PTR_BITS: u8 = 56;
    /// The mask to extract a pointer from a stamped pointer
    const PTR_MASK: usize = (1 << Self::PTR_BITS) - 1;
    /// The mask to extract a stamp from a stamped pointer
    const STAMP_MASK: usize = !Self::PTR_MASK;

    /// Decomposes a stamped pointer into a pointer and stamp
    fn decompose_stamped_pointer(pointer_and_stamp: *mut T) -> (*mut T, usize) {
        (
            pointer_and_stamp.mask(Self::PTR_MASK),
            pointer_and_stamp.mask(Self::STAMP_MASK).to_bits(),
        )
    }

    /// Combines a pointer and a stamp into a stamped pointer
    #[allow(clippy::as_conversions)]
    fn compose_stamped_pointer((pointer, stamp): (*mut T, usize)) -> *mut T {
        ((stamp << Self::PTR_BITS) | pointer.to_bits()) as *mut T
    }

    /// Loads a value from the pointer.
    ///
    /// `load` takes an `Ordering` argument which describes the memory ordering of this operation. Possible values are `SeqCst`, `Acquire` and `Relaxed`.
    ///
    /// # Panics
    /// Panics if `order` is `Release` or `AcqRel`.
    pub fn load_unstamped(&self, ordering: Ordering) -> *mut T {
        self.0.load(ordering).mask(Self::PTR_MASK)
    }

    /// Fetches the pointer, and applies a function to it that returns an
    /// optional new pointer. Returns a `Result` of `Ok(previous_pointer)` if
    /// the function returned `Some(_)`, else `Err(previous_pointer)`.
    ///
    /// Note: This may call the function multiple times if the pointer has been
    /// changed from other threads in the meantime, as long as the function
    /// returns `Some(_)`, but the function will have been applied only once to
    /// the stored pointer.
    ///
    /// `fetch_update` takes two `Ordering` arguments to describe the memory
    /// ordering of this operation. The first describes the required ordering
    /// for when the operation finally succeeds while the second describes the
    /// required ordering for loads. These correspond to the success and failure
    /// orderings of `Atomic_::compare_exchange` respectively.
    ///
    /// Using `Acquire` as success ordering makes the store part of this
    /// operation `Relaxed`, and using `Release` makes the final successful load
    /// `Relaxed`. The (failed) load ordering can only be `SeqCst`, `Acquire` or
    /// `Relaxed`.
    pub fn fetch_update_unstamped<F>(
        &self,
        set_order: Ordering,
        fetch_order: Ordering,
        mut f: F,
    ) -> Result<*mut T, *mut T>
    where
        F: FnMut(*mut T) -> Option<*mut T>,
    {
        self.0
            .fetch_update(set_order, fetch_order, |pointer_and_stamp| {
                let (pointer, stamp) = Self::decompose_stamped_pointer(pointer_and_stamp);
                f.call_mut((pointer.mask(Self::PTR_MASK),))
                    .map(|new_pointer| Self::compose_stamped_pointer((new_pointer, stamp)))
            })
            .map(|pointer_and_stamp| Self::decompose_stamped_pointer(pointer_and_stamp).0)
            .map_err(|pointer_and_stamp| Self::decompose_stamped_pointer(pointer_and_stamp).0)
    }

    /// Fetches the pointer and stamp, and applies a function to it that returns
    /// an optional new pointer and stamp. Returns a `Result` of
    /// `Ok(previous_pointer_and_stamp)` if the function returned `Some(_)`,
    /// else `Err(previous_pointer_and_stamp)`.
    ///
    /// Note: This may call the function multiple times if the pointer and stamp
    /// have been changed from other threads in the meantime, as long as the
    /// function returns `Some(_)`, but the function will have been applied only
    /// once to the stored pointer and stamp.
    ///
    /// `fetch_update` takes two `Ordering` arguments to describe the memory
    /// ordering of this operation. The first describes the required ordering
    /// for when the operation finally succeeds while the second describes the
    /// required ordering for loads. These correspond to the success and failure
    /// orderings of `Atomic_::compare_exchange` respectively.
    ///
    /// Using `Acquire` as success ordering makes the store part of this
    /// operation `Relaxed`, and using `Release` makes the final successful load
    /// `Relaxed`. The (failed) load ordering can only be `SeqCst`, `Acquire` or
    /// `Relaxed`.
    pub fn fetch_update_stamped<F>(
        &self,
        set_order: Ordering,
        fetch_order: Ordering,
        mut f: F,
    ) -> Result<(*mut T, usize), (*mut T, usize)>
    where
        F: FnMut(*mut T, usize) -> Option<(*mut T, usize)>,
    {
        self.0
            .fetch_update(set_order, fetch_order, |pointer_and_stamp| {
                f.call_mut(Self::decompose_stamped_pointer(pointer_and_stamp))
                    .map(Self::compose_stamped_pointer)
            })
            .map(Self::decompose_stamped_pointer)
            .map_err(Self::decompose_stamped_pointer)
    }
}

impl<T> const Default for AtomicStampedPtr<T> {
    /// Creates a new `AtomicStampedReference` initialized to a null pointer
    fn default() -> Self {
        Self(AtomicPtr::default())
    }
}

/// Memory mapped IO wrapper
pub struct Mmio<T> {
    /// Beginning address of the MMIO region
    start_addr: *mut T,
}

impl<T> Mmio<T> {
    /// Creates an MMIO wrapper at the given location
    /// # Safety
    /// `start_addr` must be correct, and should not be reused by anything else
    pub const unsafe fn new(start_addr: *mut T) -> Self {
        Self { start_addr }
    }
}

impl<T> core::ops::Deref for Mmio<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: By assumption, this dereference should be safe
        unsafe { &*self.start_addr }
    }
}

use bitfield_struct::bitfield;
use core::sync::atomic::{AtomicU64, Ordering};

#[bitfield(u64, debug = false)]
struct ExceptionPointer {
    #[bits(63)]
    vector: u64,
    is_unset: bool,
}

/// An `Execution` represents a running context for a program of one or more cores.
///
/// This includes the translation table pointer, and where to direct exceptions to.
pub struct Execution {
    /// The PC to jump to, for this `Execution` to handle various exceptions and interrupts
    exception_vector: AtomicU64,
    /// The translation table pointer for this `Execution`
    ttbr0: AtomicU64,
}

impl Execution {
    const fn new(ttbr0: u64) -> Self {
        Self {
            exception_vector: AtomicU64::new(ExceptionPointer::new().with_is_unset(true).0),
            ttbr0: AtomicU64::new(ttbr0),
        }
    }

    fn set_exception_vector(&self, exception_vector: u64) {
        self.exception_vector
            .store(exception_vector, Ordering::Relaxed);
    }

    fn get_exception_vector(&self) -> u64 {
        self.exception_vector.load(Ordering::Relaxed)
    }
}
use alloc::{collections::VecDeque, sync::Arc, vec::Vec};
use bitfield_struct::bitfield;
use core::{
    arch::asm,
    hint,
    ptr::{self, NonNull},
    sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
};
use stdos::sync::SpinLock;

use crate::{
    memory::{PhysicalPage, ReadablePage, WriteablePage},
    println,
};

#[bitfield(u64, debug = false)]
struct OptionPointer {
    #[bits(63)]
    value: usize,
    is_unset: bool,
}

struct AtomicOptionPointer(AtomicU64);

impl AtomicOptionPointer {
    const fn new() -> Self {
        Self(AtomicU64::new(u64::MAX))
    }

    fn load(&self, ordering: Ordering) -> OptionPointer {
        self.0.load(ordering).into()
    }

    fn store(&self, value: u64, ordering: Ordering) {
        self.0.store(value, ordering)
    }
}

#[derive(Clone, Copy)]
pub enum ExceptionCode {
    Preemption = 0,
    Resumption = 1,
    PageFault = 2,
}

/// An `Execution` represents a running context for a program of one or more cores.
///
/// This includes the translation table pointer, and where to direct exceptions to.
pub struct Execution {
    /// The PC to jump to, for this `Execution` to handle various exceptions and interrupts
    pub exception_vector: AtomicUsize,
    /// The translation table pointer for this `Execution`
    ttbr0: AtomicU64,
    writeable_pages: Vec<WriteablePage>,
    readable_pages: Vec<ReadablePage>,
    pub return_to: AtomicOptionPointer,
    token: AtomicBool,
}

impl Clone for Execution {
    fn clone(&self) -> Self {
        Self {
            exception_vector: AtomicUsize::new(self.exception_vector.load(Ordering::Relaxed)),
            ttbr0: AtomicU64::new(self.ttbr0.load(Ordering::Relaxed)),
            writeable_pages: self.writeable_pages.clone(),
            readable_pages: self.readable_pages.clone(),
            return_to: AtomicOptionPointer::new(),
            token: AtomicBool::new(self.token.load(Ordering::Relaxed)),
        }
    }
}

impl Execution {
    /// Creates a new execution withs the given address space
    pub const fn new(ttbr0: u64, exception_vector: usize) -> Self {
        Self {
            exception_vector: AtomicUsize::new(exception_vector),
            ttbr0: AtomicU64::new(ttbr0),
            writeable_pages: Vec::new(),
            readable_pages: Vec::new(),
            return_to: AtomicOptionPointer::new(),
            token: AtomicBool::new(false),
        }
    }

    /// Jumps into usermode by calling the exception vector with the given code
    pub fn jump_into(self: Arc<Self>, code: ExceptionCode) -> ! {
        let return_point = match code {
            ExceptionCode::Resumption => {
                let return_to = self.return_to.load(Ordering::Relaxed);
                if return_to.is_unset() {
                    self.exception_vector.load(Ordering::Relaxed)
                } else {
                    return_to.value()
                }
            }
            _ => self.exception_vector.load(Ordering::Relaxed),
        };
        let ttbr0 = self.ttbr0.load(Ordering::Relaxed);

        let current = get_tpidr_el1();
        if let Some(current) = current {
            // If TPIDR_EL1 is currently set to point to this execution, the strong count must be at least one from there, so we can drop the current `Arc` and keep the execution alive
            assert!(
                Arc::strong_count(&self) >= 2,
                "Arc count should be at least two from the local Arc and the Arc in TPIDR_EL1"
            );
            assert_eq!(current.as_ptr().cast_const(), Arc::as_ptr(&self), "When jumping into an execution, the current execution should not be a different execution");
            drop(self);
        } else {
            set_tpidr_el1(Some(
                NonNull::new(Arc::into_raw(self).cast_mut()).expect("`Arc`s should never be null"),
            ));
        }
        // SAFETY: This correctly sets up a return into user mode, after which entry into the kernel is only possible via exception/IRQ
        unsafe {
            asm! {
                "msr TTBR0_EL1, {TTBR0_EL1}",
                "msr SPSR_EL1, xzr",
                "msr ELR_EL1, {ELR_EL1}",
                "eret",
                TTBR0_EL1 = in(reg) ttbr0,
                ELR_EL1 = in(reg) return_point,
                options(noreturn, nostack),
            }
        }
        // NOTE: This is only here due to a bug in rust-analyzer that incorrectly thinks that execution can fall through the noreturn asm block
        unreachable!()
    }
}

/// Reads the value of `TPIDR_EL1`, as a pointer
fn get_tpidr_el1() -> Option<NonNull<Execution>> {
    let tpidr_el1;
    // SAFETY: This touches only a system register with no other side effects
    unsafe {
        asm! {
            "mrs {}, TPIDR_EL1",
            out(reg) tpidr_el1,
            options(nomem, nostack, preserves_flags)
        };
    }
    println!("GETTING TPIDR {:X?}", tpidr_el1);
    NonNull::new(tpidr_el1)
}

/// Writes the value of a pointer to `TPIDR_EL1`
fn set_tpidr_el1(execution: Option<NonNull<Execution>>) {
    let ptr = execution.map_or(ptr::null_mut(), NonNull::as_ptr);
    // SAFETY: This touches only a system register with no other side effects
    unsafe {
        asm! {
            "msr TPIDR_EL1, {}",
            in(reg) ptr,
            options(nomem, nostack, preserves_flags)
        };
    }
}

/// Returns an `Arc` referring to the current execution for this core, or `None` if there is no such execution
pub fn current() -> Option<Arc<Execution>> {
    get_tpidr_el1().map(|execution| {
        let execution = execution.as_ptr();
        // SAFETY: Because the pointer was in TPIDR_EL1 and nonnull, it points to an alive `Arc` that came from `Arc::into_raw`
        // Incrementing implements cloning the arc, but without needing to change TPIDR_EL1 multiple times
        unsafe { Arc::increment_strong_count(execution) }
        // SAFETY: See above
        unsafe { Arc::from_raw(execution) }
    })
}

/// Returns and removes an `Arc` referring to the current execution for this core, or `None` if there is no such execution. After doing so, any subsequent calls to `current` return `None` until set by another function
pub fn remove_current() -> Option<Arc<Execution>> {
    let old_current = get_tpidr_el1().map(|execution| {
        let execution = execution.as_ptr();
        // SAFETY: If TPIDR_EL1 is not null, it must have been set with the value of some `Arc::into_raw` call, so this is a safe operation
        unsafe { Arc::from_raw(execution) }
    });
    set_tpidr_el1(None);
    old_current
}

/// The queue for all executions that are ready to run
static RUN_QUEUE: SpinLock<VecDeque<Arc<Execution>>> = SpinLock::new(VecDeque::new());

/// Schedules an `Execution` to run
pub fn add_to_running(execution: Arc<Execution>) {
    RUN_QUEUE.lock().push_back(execution);
}

/// Sets a new `Execution` to be the running `Execution` for the core. Returns the previously running `Execution`, if any

pub fn idle_loop() -> ! {
    assert!(
        current().is_none(),
        "The currently active execution should be cleared before the idle loop"
    );
    println!("IN IDLE LOOP");
    loop {
        if let Some(execution) = RUN_QUEUE.lock().pop_front() {
            println!("PREPARE TO JUMP");
            execution.jump_into(ExceptionCode::Resumption)
        }
        hint::spin_loop();
    }
}

//! `Execution`s and related functionality
//!
//! These are the kernel's description of running user programs and their associated (physical memory) resources

use crate::{
    machine::to_physical_addr,
    memory::{ReadablePage, WriteablePage},
    println,
};
use alloc::{collections::VecDeque, sync::Arc, vec::Vec};
use bitfield_struct::bitfield;
use core::{
    arch::asm,
    hint,
    mem::transmute,
    ptr::{self, NonNull},
    sync::atomic::{AtomicBool, AtomicPtr, AtomicU64, AtomicUsize, Ordering},
};
use stdos::sync::SpinLock;

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
    writeable_pages: SpinLock<Vec<WriteablePage>>,
    readable_pages: SpinLock<Vec<ReadablePage>>,
    user_context: AtomicPtr<UserContext>,
    ttbr0: AtomicU64,
    tcr_el1: AtomicU64,
    token: AtomicBool,
}

#[repr(C)]
pub struct UserContext {
    pub exception_vector: AtomicUsize,
    pub exception_stack: AtomicPtr<u64>,
}

impl UserContext {
    pub fn pop(&self) -> u64 {
        let popped_sp = unsafe {
            self.exception_stack
                .fetch_ptr_sub(1, Ordering::Relaxed)
                .sub(1)
        };
        unsafe { UserPointer(popped_sp).read() }
    }

    /// Writes a `u64` to the memory pointed to by `exception_stack`, with user privileges, then increments the `exception_stack` pointer
    fn push(&self, val: u64) {
        let pushed_sp = self.exception_stack.fetch_ptr_add(1, Ordering::SeqCst);
        unsafe { UserPointer(pushed_sp).write(val) }
    }
}

/// A pointer whose accesses are performed under user privileges instead of kernel privileges
#[repr(transparent)]
pub struct UserPointer(*mut u64);

impl UserPointer {
    /// Reads a value from the `UserPointer`
    pub unsafe fn read(&self) -> u64 {
        let val;
        unsafe {
            asm! {
                "ldtr {}, [{}]",
                out(reg) val,
                in(reg) self.0,
                options(readonly, nostack, preserves_flags)
            };
        }
        val
    }

    /// Writes a value to the `UserPointer`
    pub unsafe fn write(&mut self, val: u64) {
        unsafe {
            asm! {
                "sttr {}, [{}]",
                in(reg) val,
                in(reg) self.0,
                options(nostack, preserves_flags)
            };
        }
    }
}

pub enum ContextError {
    MisalignedTtbr0,
    InaccessibleTtbr0,
    InvalidTcrBits,
    MisalignedUserContext,
    InaccessibleUserContext,
}

impl Execution {
    /// Creates a new execution withs the given address space
    pub const fn new(tcr_el1: u64, ttbr0: u64, user_context: *const UserContext) -> Self {
        Self {
            writeable_pages: SpinLock::new(Vec::new()),
            readable_pages: SpinLock::new(Vec::new()),
            token: AtomicBool::new(false),
            user_context: AtomicPtr::new(user_context.cast_mut()),
            ttbr0: AtomicU64::new(ttbr0),
            tcr_el1: AtomicU64::new(tcr_el1),
        }
    }

    fn page_bits(&self) -> u8 {
        16
    }

    pub fn set_context(
        &self,
        user_context: *const UserContext,
        ttbr0: u64,
        tcr_el1: u64,
    ) -> Result<(), ContextError> {
        if ttbr0 & 0x3F != 0 {
            return Err(ContextError::MisalignedTtbr0);
        }
        if !self.contains_pa(ttbr0) {
            return Err(ContextError::InaccessibleTtbr0);
        }
        if !user_context.is_aligned() {
            return Err(ContextError::MisalignedUserContext);
        }
        if (user_context.addr() >> 48) & 0xFF != 0 {
            return Err(ContextError::InaccessibleUserContext);
        }
        // self.tcr_el1.store(tcr_el1, Ordering::Relaxed);
        self.ttbr0.store(ttbr0, Ordering::Relaxed);
        self.user_context
            .store(user_context.cast_mut(), Ordering::Relaxed);
        Ok(())
    }

    fn contains_pa(&self, pa: u64) -> bool {
        self.writeable_pages
            .lock()
            .binary_search_by(|x| (x.addr() >> self.page_bits()).cmp(&(pa >> self.page_bits())))
            .is_ok()
            || self
                .readable_pages
                .lock()
                .binary_search_by(|x| (x.addr() >> self.page_bits()).cmp(&(pa >> self.page_bits())))
                .is_ok()
    }

    fn contains_pa_writeable(&self, pa: u64) -> bool {
        self.writeable_pages
            .lock()
            .binary_search_by(|x| (x.addr() >> self.page_bits()).cmp(&(pa >> self.page_bits())))
            .is_ok()
    }

    pub fn validate_user_pointer<T>(&self, ptr: *const T) -> Option<&T> {
        let pa = to_physical_addr(ptr.addr());
        println!("the pa is {:X?}", pa);
        pa.ok().and_then(|pa| {
            self.contains_pa(pa.pa())
                .then(|| unsafe { ptr.as_ref() }.unwrap())
        })
    }

    pub fn validate_user_pointer_writeable<T>(&self, ptr: *const T) -> Option<&T> {
        let pa = to_physical_addr(ptr.addr());
        println!("PA: {:X?}, VA: {:X?}", pa, ptr);
        for a in self.writeable_pages.lock().iter() {
            println!("Among:{:?}", a.addr());
        }
        pa.ok().and_then(|pa| {
            self.contains_pa_writeable(pa.pa())
                .then(|| unsafe { ptr.as_ref() }.unwrap())
        })
    }

    pub fn user_context(&self) -> &UserContext {
        let context = self.user_context.load(Ordering::Relaxed);
        assert!(context.is_aligned());
        unsafe { context.as_ref() }.unwrap()
    }

    pub fn with_autotranslate<T>(&self, f: impl Fn() -> T) -> T {
        let tcr_el1 = self.tcr_el1.load(Ordering::Relaxed);
        unsafe {
            asm! {
                "msr TCR_EL1, {TCR_EL1_MODIFIED}",
                "isb",
                TCR_EL1_MODIFIED = in(reg) tcr_el1 & !(1 << 7),
                options(readonly, nostack, preserves_flags),
            }
        }
        let result = f();
        unsafe {
            asm! {
                "msr TCR_EL1, {TCR_EL1}",
                TCR_EL1 = in(reg) tcr_el1,
                options(readonly, nostack, preserves_flags),
            }
        }
        result
    }

    /// Jumps into usermode by calling the exception vector with the given code and arguments
    pub fn jump_into(self: Arc<Self>, code: ExceptionCode, arguments: &[u64]) -> ! {
        let ttbr0 = self.ttbr0.load(Ordering::Relaxed);
        let tcr_el1 = self.tcr_el1.load(Ordering::Relaxed);

        unsafe {
            asm! {
                "msr TTBR0_EL1, {TTBR0_EL1}",
                "msr TCR_EL1, {TCR_EL1}",
                "isb",
                TTBR0_EL1 = in(reg) ttbr0,
                TCR_EL1 = in(reg) tcr_el1,
                options(readonly, nostack, preserves_flags),
            }
        }

        let current = get_tpidr_el1();
        let context = self.user_context();
        let ev_addr = context.exception_vector.as_ptr().cast();

        let context: &UserContext = unsafe { transmute(context) };

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

        for &arg in arguments {
            context.push(arg);
        }
        context.push(code as u64);

        let return_point = unsafe { UserPointer(ev_addr).read() };

        println!("Preparing to jump to {:X}", return_point);

        if let ExceptionCode::Resumption = code {
            unsafe {
                asm! {
                    "msr SP_EL0, xzr",
                    options(nomem, nostack, preserves_flags),
                }
            }
        }

        // SAFETY: This correctly sets up a return into user mode, after which entry into the kernel is only possible via exception/IRQ
        unsafe {
            asm! {
                "msr SPSR_EL1, xzr",
                "msr ELR_EL1, {ELR_EL1}",
                "eret",
                ELR_EL1 = in(reg) return_point,
                options(noreturn, nostack, preserves_flags),
            }
        }
        // NOTE: This is only here due to a bug in rust-analyzer that incorrectly thinks that execution can fall through the noreturn asm block
        unreachable!()
    }

    /// Adds a page to the write set of an `Execution`
    pub fn add_writable_page(&self, page: WriteablePage) {
        let mut pages = self.writeable_pages.lock();
        let insertion = pages
            .binary_search(&page)
            .expect_err("Should not add a duplicate page to an execution's writable set");
        pages.insert(insertion, page);
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

/// Sets a new `Execution` to be the running `Execution` for the core.
pub fn idle_loop() -> ! {
    assert!(
        current().is_none(),
        "The currently active execution should be cleared before the idle loop"
    );
    unsafe {
        asm! {
            "msr DAIFClr, 0b1111",
            options(nomem, nostack, preserves_flags)
        }
    }
    loop {
        if let Some(execution) = RUN_QUEUE.lock().pop_front() {
            unsafe {
                asm! {
                    "msr DAIFSet, 0b1111",
                    options(nomem, nostack, preserves_flags)
                }
            }
            execution.jump_into(ExceptionCode::Resumption, &[]);
        }
        hint::spin_loop();
    }
}

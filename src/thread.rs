use crate::{
    call_once,
    cell::InitCell,
    collections::{ArcStack, Stackable},
    derive_ord,
    kernel::PerCore,
    sync::RwLock,
    sync::{Mutex, SpinLock},
};
use aarch64_cpu::asm::{sev, wfe};
use alloc::{boxed::Box, collections::BinaryHeap, sync::Arc};
use core::{
    alloc::Layout,
    cell::{Cell, RefCell},
    cmp::Reverse,
    num::NonZeroU64,
    ptr::NonNull,
    sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
    time::Duration,
};

/// Architecture threading support: context switching and identifying the current thread
mod architecture;
pub use architecture::*;

/// Guards to temporarily disable preemption
mod preemption_guard;
pub use preemption_guard::PreemptionGuard;

/// Number of cores
const NUM_CORES: u8 = 4;

/// Local Tcb Data; not visible to other threads
struct TcbLocal {
    /// Whether or not this thread is preemptible
    preemptible: Cell<bool>,
    /// The time when the thread last began to run
    last_started: Cell<Duration>,
    /// Whether or not there is a pending preemption for this thread
    pending_preemption: Cell<bool>,
    /// The work this thread is running
    work: RefCell<Box<dyn FnMut()>>,
}

/// A thread its and associated context
#[repr(C)]
pub struct Tcb {
    /// The last-saved stack pointer of the thread
    /// Should not be used, except for when context switching or upon creation
    sp: NonNull<u128>,
    /// The thread's numerical ID, for logging purposes
    id: NonZeroU64,
    /// Next pointer
    next: *mut Self,
    /// The SP from allocation
    allocated_sp: NonNull<u8>,
    /// The total CPU runtime of this thread
    runtime: RwLock<Duration>,
    /// Private internal data
    local: TcbLocal,
}

impl Stackable for Tcb {
    unsafe fn set_next(&mut self, next: *mut Self) {
        self.next = next;
    }

    fn read_next(&self) -> *mut Self {
        self.next
    }
}

/// The ID of the next thread created
static NEXT_THREAD_ID: AtomicU64 = AtomicU64::new(1);
/// The number of currently running threads
static ACTIVE_THREAD_COUNT: AtomicUsize = AtomicUsize::new(0);
/// The global ready thread list
static READY_THREADS: InitCell<SpinLock<BinaryHeap<Reverse<Thread>>>> = InitCell::new();
/// The idle cores, one per core
static IDLE_THREADS: InitCell<PerCore<Thread>> = InitCell::new();
/// The static size of a stack, in bytes
/// TODO: Convert this to a dynamic size via paging
const STACK_SIZE: usize = 0x2000;
/// The layout for the stack
#[allow(clippy::undocumented_unsafe_blocks)]
const STACK_LAYOUT: Layout = unsafe { Layout::from_size_align_unchecked(STACK_SIZE, 16) };
/// Gets a prepared stack for a thread to use
fn get_stack() -> (NonNull<u8>, NonNull<u128>) {
    loop {
        #[allow(clippy::as_conversions)]
        if let Some(sp) =
            // SAFETY: Layout is correct
            NonNull::new(unsafe { alloc::alloc::alloc(STACK_LAYOUT) })
        {
            // SAFETY: The passed stack pointer is correctly computed via allocation
            return (sp, unsafe {
                architecture::set_up_stack(
                    NonNull::new(sp.as_ptr().byte_add(STACK_SIZE).cast())
                        .expect("Stack should not be null"),
                )
            });
        }
        yield_now();
    }
}

/// Spawns a new thread
pub fn spawn(f: impl FnMut() + 'static) -> Thread {
    let active_count = ACTIVE_THREAD_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
    {
        let mut threads = READY_THREADS.lock();
        let curr_capacity = threads.capacity();
        if curr_capacity < active_count {
            let len = threads.len();
            threads.reserve(curr_capacity * 2 - len);
        }
    }

    let (allocated_sp, sp) = get_stack();

    Thread(Arc::new(Tcb {
        id: NonZeroU64::new(NEXT_THREAD_ID.fetch_add(1, core::sync::atomic::Ordering::Relaxed))
            .expect("ID should not be 0"),
        runtime: RwLock::new(Duration::ZERO),
        allocated_sp,
        sp,
        next: core::ptr::null_mut(),
        local: TcbLocal {
            preemptible: Cell::new(true),
            last_started: Cell::new(Duration::default()),
            pending_preemption: Cell::new(false),
            work: RefCell::new(Box::new(f)),
        },
    }))
}

impl Tcb {
    /// Runs the current thread
    /// # Safety
    /// This should only be called once per thread, to begin its execution
    pub unsafe fn run(&self) -> ! {
        (self.local.work.borrow_mut()).call_mut(());
        stop();
    }

    /// Returns whether or not the thread is an idle thread
    fn is_idle(&self) -> bool {
        u64::from(self.id) <= NUM_CORES.into()
    }
}

impl Drop for Tcb {
    fn drop(&mut self) {
        ACTIVE_THREAD_COUNT.fetch_sub(1, Ordering::Relaxed);
        let allocated_sp = self.allocated_sp;
        // SAFETY: This is the pointer received from `alloc` and the layout given to `alloc`
        unsafe { alloc::alloc::dealloc(allocated_sp.as_ptr(), STACK_LAYOUT) }
    }
}

/// Stops the currently executing thread, and releases its resources
pub fn stop() -> ! {
    /// Pending threads to be freed
    static DEAD_THREADS: ArcStack<Tcb> = ArcStack::new();

    while let Some(dead_thread) = DEAD_THREADS.pop() {
        drop(dead_thread);
    }

    force_context_switch(|me| DEAD_THREADS.push(me));
    unreachable!()
}

/// Attempts to get a thread to run from the ready threads
fn get_thread_to_run() -> Option<Arc<Tcb>> {
    READY_THREADS.lock().pop().map(|Reverse(Thread(tcb))| tcb)
}

/// Context switches into the next ready thread, or the idle thread if none are
/// available
fn force_context_switch(callback: impl FnMut(Arc<Tcb>)) {
    if let Some(thread) = get_thread_to_run() {
        architecture::context_switch(thread, callback);
    } else {
        let _guard = PreemptionGuard::new();
        architecture::context_switch(
            {
                let idle_stored = IDLE_THREADS.current();
                Arc::clone(&idle_stored.0)
            },
            callback,
        );
    }
}

/// A handle for a thread
pub struct Thread(pub Arc<Tcb>);

impl Thread {
    /// Gets the thread’s unique identifier.
    pub fn id(&self) -> ThreadId {
        let Self(thread) = self;
        ThreadId(thread.id)
    }
}

/// A unique identifier for a running thread.
pub struct ThreadId(NonZeroU64);

impl ThreadId {
    /// This returns a numeric identifier for the thread identified by this `ThreadId`.
    pub fn as_u64(&self) -> NonZeroU64 {
        self.0
    }
}

derive_ord!(Thread);

// Sorts threads for the ready list, by runtime
impl Ord for Thread {
    fn cmp(&self, Self(other): &Self) -> core::cmp::Ordering {
        self.0.runtime.read().cmp(&other.runtime.read())
    }
}

/// The idle loop, for idle threads
pub fn idle_loop() -> ! {
    loop {
        if let Some(thread) = get_thread_to_run() {
            architecture::context_switch(thread, |_me| ());
        }
        wfe();
    }
}

/// Schedules a thread to be run
pub fn schedule(thread: Thread) {
    READY_THREADS.lock().push(Reverse(thread));
    sev();
}

/// Cooperatively yields to another thread, if another thread is waiting to run
#[allow(dead_code)]
pub fn yield_now() {
    assert!(!current().0.is_idle(), "The idle thread should never yield");
    if let Some(thread) = get_thread_to_run() {
        architecture::context_switch(thread, |current| schedule(Thread(current)));
    }
}

/// Blocks the calling thread, and executes the given callback after switching threads
pub fn block(callback: impl FnMut(Arc<Tcb>)) {
    assert!(!current().0.is_idle(), "The idle thread should never block");
    force_context_switch(callback);
}

/// Primary initialization sequence for threading
/// # Safety
/// Must only be called once, at the appropriate time
pub unsafe fn init() {
    call_once!();
    // SAFETY: This is called in the initialization sequence on a single core
    // and so no concurrent or prior accesses are possible
    unsafe {
        READY_THREADS.set(SpinLock::new(BinaryHeap::new()));
        IDLE_THREADS.set(PerCore::new(|| {
            // The `work` given does not matter, as `idle_loop` is directly
            // called to begin the idle loop
            let thread = spawn(|| {});
            thread.0.local.preemptible.set(false);
            thread
        }));
    }

    // SAFETY: This is only run once per-core
    unsafe {
        architecture::set_me(Thread(Arc::clone(
            &IDLE_THREADS.current_unprotected().borrow().0,
        )));
    };

    // Don't count the idle threads as active threads
    ACTIVE_THREAD_COUNT.store(0, Ordering::Relaxed);
}

/// Second initialization sequence for threading
/// # Safety
/// Must only be called once on each core, at the appropriate time
pub unsafe fn per_core_init() {
    /// Enforces mutual exclusion on the heap accesses so that no blocking occurs
    static BUSY: AtomicBool = AtomicBool::new(false);
    while BUSY.swap(true, Ordering::Acquire) {
        wfe();
    }
    // SAFETY: No preemption or blocking occurs here
    let cloned_arc = Arc::clone(unsafe { &IDLE_THREADS.current_unprotected().borrow().0 });
    BUSY.store(false, Ordering::Release);
    sev();

    // SAFETY: This is only run once per-core
    unsafe {
        architecture::set_me(Thread(cloned_arc));
    }
}

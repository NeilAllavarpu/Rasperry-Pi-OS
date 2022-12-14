use crate::{
    architecture::{self, SpinLock},
    call_once, call_once_per_core,
    kernel::{heap::FixedBlockHeap, Mutex, PerCore, SetOnce},
};
use aarch64_cpu::asm::{sev, wfe};
use alloc::{boxed::Box, collections::BinaryHeap, sync::Arc};
use core::{
    alloc::Layout,
    sync::atomic::{AtomicU64, AtomicUsize, Ordering},
    time::Duration,
};

/// A thread its and associated context
#[repr(C)]
pub struct Thread {
    /// The thread's numerical ID, for logging purposes
    pub id: u64,
    /// The last-saved stack pointer of the thread
    /// Should not be used, except for when context switching or upon creation
    sp: *mut u128,
    /// the SP from allocation
    allocated_sp: *mut u8,
    /// The total CPU runtime of this thread
    pub runtime: Duration,
    /// The time when the thread last began to run
    pub last_started: Duration,
    /// The work this thread is running
    pub work: Box<dyn FnMut()>,
}

/// The list of ready threads, sorted by runtime
struct ReadyThreads {
    /// The protected list of threads
    threads: SpinLock<BinaryHeap<Arc<Thread>>>,
}

/// The ID of the next thread created
static NEXT_THREAD_ID: AtomicU64 = AtomicU64::new(1);
/// The number of currently running threads
pub static ACTIVE_THREAD_COUNT: AtomicUsize = AtomicUsize::new(0);
/// The global ready thread list
static READY_THREADS: SetOnce<ReadyThreads> = SetOnce::new();
/// The idle cores, one per core
static IDLE_THREADS: SetOnce<PerCore<Arc<Thread>>> = SetOnce::new();
/// The static size of a stack, in bytes
/// TODO: Convert this to a dynamic size via paging
const STACK_SIZE: usize = 0x2000;
/// The layout for the stack
#[allow(clippy::undocumented_unsafe_blocks)]
const STACK_LAYOUT: Layout = unsafe { Layout::from_size_align_unchecked(STACK_SIZE, STACK_SIZE) };
/// The pool of thread stacks
static mut STACKS: FixedBlockHeap = FixedBlockHeap::new(STACK_SIZE);
/// Gets a prepared stack for a thread to use
fn get_stack() -> (*mut u8, *mut u128) {
    #[allow(clippy::as_conversions)]
    let sp =
    // SAFETY: yes
        unsafe { STACKS.alloc(STACK_LAYOUT) }
            .expect("Out of stacks!");
    // SAFETY: The passed stack pointer is correctly computed via allocation
    (sp, unsafe {
        architecture::thread::set_up_stack(sp.byte_add(STACK_SIZE).cast())
    })
}

/// Creates a new thread to run the given work
#[macro_export]
macro_rules! thread {
    ($work: ident) => {
        $crate::kernel::thread::Thread::new_from_function($work)
    };
    ($work: expr) => {
        $crate::kernel::thread::Thread::new(alloc::boxed::Box::new($work))
    };
}

impl Thread {
    /// Creates a new thread, with the given closure as its execution path
    pub fn new(work: Box<dyn FnMut()>) -> Arc<Self> {
        let active_count = ACTIVE_THREAD_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
        {
            let mut threads = READY_THREADS.threads.lock();
            let curr_capacity = threads.capacity();
            if curr_capacity < active_count {
                let len = threads.len();
                threads.reserve(curr_capacity * 2 - len);
            }
        }
        let (allocated_sp, sp) = get_stack();

        Arc::new(Self {
            id: NEXT_THREAD_ID.fetch_add(1, core::sync::atomic::Ordering::Relaxed),
            work: Box::new(work),
            runtime: Duration::ZERO,
            last_started: Duration::default(),
            allocated_sp,
            sp,
        })
    }

    /// Creates a new thread, with the given function as its execution path
    pub fn new_from_function(work: fn()) -> Arc<Self> {
        Self::new(Box::new(work))
    }

    /// Runs the current thread
    /// # Safety
    /// This should only be called once per thread, to begin its execution
    pub unsafe fn run(&mut self) -> ! {
        (self.work)();
        stop();
    }
}

/// Stops the currently executing thread, and releases its resources
pub fn stop() -> ! {
    architecture::thread::context_switch(
        READY_THREADS
            .get()
            .unwrap_or_else(|| IDLE_THREADS.with_current(|idle| Arc::clone(idle))),
        |me| {
            let allocated_sp = me.allocated_sp;
            ACTIVE_THREAD_COUNT.fetch_sub(1, Ordering::Relaxed);
            drop(me);
            // SAFETY: This is the pointer received from `alloc` and the layout given to `alloc`
            unsafe { STACKS.dealloc(allocated_sp, STACK_LAYOUT) }
        },
    );
    unreachable!()
}

impl PartialEq for Thread {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}

impl Eq for Thread {}

impl PartialOrd for Thread {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

// Sorts threads for the ready list
// A thread has MAX priority (MAX value) if it has been running the LEAST
// So we reverse here
impl Ord for Thread {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.runtime.cmp(&other.runtime).reverse()
    }
}

impl ReadyThreads {
    /// Adds a thread to the ready list, to be run when possible
    fn add(&self, thread: Arc<Thread>) {
        self.threads.lock().push(thread);
    }

    /// Removes a thread from the ready list, if any are available
    fn get(&self) -> Option<Arc<Thread>> {
        self.threads.lock().pop()
    }
}

/// The idle loop, for idle threads
fn idle_loop() {
    loop {
        if let Some(thread) = READY_THREADS.get() {
            architecture::thread::context_switch(thread, |_me| ());
        }
        wfe();
    }
}

/// Schedules a thread to be run
pub fn schedule(thread: Arc<Thread>) {
    READY_THREADS.add(thread);
    sev();
}

/// Cooperatively yields to another thread, if another thread is waiting to run
#[allow(dead_code)]
pub fn switch() {
    if let Some(thread) = READY_THREADS.get() {
        architecture::thread::context_switch(thread, schedule);
    }
}

/// Primary initialization sequence for threading
/// # Safety
/// Must only be called once, at the appropriate time
pub unsafe fn init() {
    call_once!();
    // SAFETY: This is called in the initialization sequence on a single core
    // and so no concurrent or prior accesses are possible
    unsafe {
        #[allow(clippy::as_conversions)]
        STACKS.init(0x40_0000 as *mut (), 0x2000_0000 - 0x40_0000);
        READY_THREADS.set(ReadyThreads {
            threads: SpinLock::new(BinaryHeap::new()),
        });
        IDLE_THREADS.set(PerCore::new_from_array([
            thread!(idle_loop),
            thread!(idle_loop),
            thread!(idle_loop),
            thread!(idle_loop),
        ]));
    }
    // Don't count the idle threads as active threads
    ACTIVE_THREAD_COUNT.store(0, Ordering::Relaxed);
}

/// Second initialization sequence for threading
/// # Safety
/// Must only be called once on each core, at the appropriate time
pub unsafe fn per_core_init() {
    call_once_per_core!();
    IDLE_THREADS.with_current(|idle|
        // SAFETY: This is only run once per-core
        unsafe { architecture::thread::set_me(Arc::clone(idle)) });
}

use crate::{
    architecture::{self, SpinLock},
    call_once, call_once_per_core,
    kernel::{Mutex, PerCore, SetOnce},
};
use aarch64_cpu::asm::{sev, wfe};
use alloc::{boxed::Box, collections::BinaryHeap, sync::Arc};
use core::{
    sync::atomic::{AtomicPtr, AtomicU64, AtomicUsize, Ordering},
    time::Duration,
};

#[repr(C)]
pub struct Thread {
    pub id: u64,
    sp: *mut u128,
    pub runtime: Duration,
    pub last_started: Duration,
    pub work: Box<dyn FnMut()>,
}
struct ReadyThreads {
    threads: SpinLock<BinaryHeap<Arc<Thread>>>,
}

static NEXT_THREAD_ID: AtomicU64 = AtomicU64::new(1);
static ACTIVE_THREAD_COUNT: AtomicUsize = AtomicUsize::new(0);
static READY_THREADS: SetOnce<ReadyThreads> = SetOnce::new();
static IDLE_THREADS: SetOnce<PerCore<Arc<Thread>>> = SetOnce::new();

fn get_stack() -> *mut u128 {
    static STACK_NEXT: AtomicPtr<u128> = AtomicPtr::new(0x400000 as *mut u128);
    const STACK_SIZE: usize = 0x2000;
    let sp = STACK_NEXT.fetch_byte_add(STACK_SIZE, core::sync::atomic::Ordering::Relaxed);
    unsafe { architecture::thread::set_up_stack(sp.byte_add(STACK_SIZE)) }
}

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
        READY_THREADS.get().threads.lock(|threads| {
            let curr_len = threads.len();
            if curr_len < active_count {
                threads.reserve(active_count - curr_len)
            }
        });
        Arc::new(Self {
            id: NEXT_THREAD_ID.fetch_add(1, core::sync::atomic::Ordering::Relaxed),
            work: Box::new(work),
            runtime: Duration::ZERO,
            last_started: Duration::default(),
            sp: get_stack(),
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
    if ACTIVE_THREAD_COUNT.fetch_sub(1, Ordering::Relaxed) == 1 {
        architecture::shutdown(0);
    }
    architecture::thread::context_switch(
        READY_THREADS
            .get()
            .get()
            .unwrap_or_else(|| IDLE_THREADS.get().with_current(|idle| idle.clone())),
        drop,
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
    fn new() -> Self {
        Self {
            threads: SpinLock::new(BinaryHeap::new()),
        }
    }

    fn add(&self, thread: Arc<Thread>) {
        self.threads.lock(|ready| ready.push(thread))
    }

    fn get(&self) -> Option<Arc<Thread>> {
        self.threads.lock(|ready| ready.pop())
    }
}

pub fn idle_loop() {
    loop {
        if let Some(thread) = READY_THREADS.get().get() {
            architecture::thread::context_switch(thread, |_me| ());
        }
        wfe()
    }
}

/// Schedules a thread to be run
pub fn schedule(thread: Arc<Thread>) {
    READY_THREADS.get().add(thread);
    sev();
}

/// Cooperatively yields to another thread, if another thread is waiting to run
#[allow(dead_code)]
pub fn switch() {
    if let Some(thread) = READY_THREADS.get().get() {
        architecture::thread::context_switch(thread, schedule);
    }
}

pub fn init() {
    call_once!();
    READY_THREADS.set(ReadyThreads::new());
    IDLE_THREADS.set(PerCore::new_from_array([
        thread!(idle_loop),
        thread!(idle_loop),
        thread!(idle_loop),
        thread!(idle_loop),
    ]));
    // Don't count the idle threads as active threads
    ACTIVE_THREAD_COUNT.store(0, Ordering::Relaxed);
}

pub fn per_core_init() {
    call_once_per_core!();
    IDLE_THREADS
        .get()
        .with_current(|idle| unsafe { architecture::thread::set_me(idle.clone()) })
}

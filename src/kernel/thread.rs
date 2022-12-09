use crate::{
    architecture::{self, Spinlock},
    kernel::{Mutex, PerCore, SetOnce},
};
use aarch64_cpu::asm::{sev, wfe};
use alloc::{boxed::Box, collections::BinaryHeap};
use core::{
    sync::atomic::{AtomicPtr, AtomicU64, Ordering},
    time::Duration,
};

#[repr(C)]
pub struct TCB {
    pub id: u64,
    sp: *mut u128,
    runtime: Duration,
    pub work: fn() -> (),
}
struct ReadyThreads(Spinlock<BinaryHeap<*mut TCB>>);

static NEXT_THREAD_ID: AtomicU64 = AtomicU64::new(1);
static ACTIVE_THREAD_COUNT: AtomicU64 = AtomicU64::new(0);
static READY_THREADS: SetOnce<ReadyThreads> = SetOnce::new();
static IDLE_THREADS: SetOnce<PerCore<*mut TCB>> = SetOnce::new();

fn get_stack() -> *mut u128 {
    static STACK_NEXT: AtomicPtr<u128> = AtomicPtr::new(0x400000 as *mut u128);
    const STACK_SIZE: usize = 0x2000;
    let sp = STACK_NEXT.fetch_byte_add(STACK_SIZE, core::sync::atomic::Ordering::Relaxed);
    unsafe { architecture::thread::set_up_stack(sp.byte_add(STACK_SIZE)) }
}

impl TCB {
    pub fn new(work: fn() -> ()) -> *mut Self {
        ACTIVE_THREAD_COUNT.fetch_add(1, Ordering::Relaxed);
        Box::into_raw(Box::new(Self {
            id: NEXT_THREAD_ID.fetch_add(1, core::sync::atomic::Ordering::Relaxed),
            work,
            runtime: Duration::ZERO,
            sp: get_stack(),
        }))
    }

    pub fn run(&self) -> ! {
        (self.work)();
        self.stop();
    }

    pub fn stop(&self) -> ! {
        if ACTIVE_THREAD_COUNT.fetch_sub(1, Ordering::Relaxed) == 1 {
            architecture::shutdown(0);
        }
        architecture::thread::context_switch(
            READY_THREADS.get().get().unwrap_or(
                unsafe { (*(IDLE_THREADS.get().with_current(|idle| idle))).as_mut() }.unwrap(),
            ),
            |me: *mut Self| unsafe {
                drop(Box::from_raw(me));
            },
        );
        unreachable!()
    }
}

impl PartialEq for TCB {
    fn eq(&self, other: &Self) -> bool {
        self.runtime == other.runtime
    }
}

impl Eq for TCB {}

impl PartialOrd for TCB {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.runtime.partial_cmp(&other.runtime)
    }
}

// Sorts TCBs for the ready list
// A TCB has MAX priority (MAX value) if it has been running the LEAST
// So we reverse here
impl Ord for TCB {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.partial_cmp(other).unwrap()
    }
}

impl ReadyThreads {
    fn new() -> Self {
        Self {
            0: Spinlock::new(BinaryHeap::new()),
        }
    }

    fn add(&self, thread: *mut TCB) -> () {
        self.0.lock(|ready| ready.push(thread))
    }

    fn get(&self) -> Option<*mut TCB> {
        self.0.lock(|ready| ready.pop())
    }
}

pub fn idle_loop() -> () {
    loop {
        match READY_THREADS.get().get() {
            Some(thread) => {
                architecture::thread::context_switch(thread, |_me| ());
            }
            None => (),
        }
        wfe()
    }
}

pub fn schedule(thread: *mut TCB) -> () {
    READY_THREADS.get().add(thread);
    sev();
}

pub fn switch() {
    match READY_THREADS.get().get() {
        Some(thread) => {
            architecture::thread::context_switch(thread, |me: *mut TCB| {
                schedule(me);
            });
        }
        None => (),
    }
}

pub fn init() -> () {
    READY_THREADS.set(ReadyThreads::new());
    IDLE_THREADS.set(PerCore::new_from_array([
        (TCB::new(idle_loop)),
        (TCB::new(idle_loop)),
        (TCB::new(idle_loop)),
        (TCB::new(idle_loop)),
    ]));
    // Don't count the idle threads as active threads
    ACTIVE_THREAD_COUNT.store(0, Ordering::Relaxed);
}

pub fn per_core_init() -> () {
    IDLE_THREADS
        .get()
        .with_current(|idle| unsafe { architecture::thread::set_me(*idle) })
}

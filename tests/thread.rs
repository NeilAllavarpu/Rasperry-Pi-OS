#![feature(custom_test_frameworks)]
#![no_main]
#![no_std]
#![reexport_test_harness_main = "test_main"]
#![test_runner(libkernel::test_runner)]
#![feature(default_alloc_error_handler)]

extern crate alloc;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicU64, Ordering};
use libkernel::{add_test, kernel, thread};

#[no_mangle]
fn kernel_main() {
    test_main()
}

add_test!(runs_basic_threading, {
    const NUM_THREADS: u64 = 1 << 16;
    const MAX_ACTIVE: u64 = 1 << 8;
    let counter: Arc<AtomicU64> = Arc::new(AtomicU64::new(0));

    for n in 0..NUM_THREADS {
        let counter_ = counter.clone();
        kernel::thread::schedule(thread!(move || {
            assert!(counter_.fetch_add(1, Ordering::Acquire) < NUM_THREADS);
        }));
        if n - counter.load(Ordering::Acquire) > MAX_ACTIVE {
            kernel::thread::switch()
        }
    }

    while counter.load(Ordering::Acquire) < NUM_THREADS {
        kernel::thread::switch();
    }

    assert!(counter.load(Ordering::Acquire) == NUM_THREADS);
});

#![feature(custom_test_frameworks)]
#![no_main]
#![no_std]
#![reexport_test_harness_main = "test_main"]
#![test_runner(libkernel::test_runner)]
#![feature(default_alloc_error_handler)]

use core::sync::atomic::{AtomicU64, Ordering};
use libkernel::kernel;
use test_macros::kernel_test;

#[no_mangle]
fn kernel_main() {
    test_main()
}

#[kernel_test]
fn runs_basic_threads() {
    const NUM_THREADS: u64 = 16;
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    COUNTER.store(0, Ordering::Release);
    for _ in 0..NUM_THREADS {
        kernel::thread::schedule(kernel::thread::TCB::new(|| {
            assert!(COUNTER.fetch_add(1, Ordering::Acquire) < NUM_THREADS);
        }));
    }

    while COUNTER.load(Ordering::Acquire) < NUM_THREADS {
        kernel::thread::switch();
    }
    assert!(COUNTER.load(Ordering::Acquire) == NUM_THREADS);
}

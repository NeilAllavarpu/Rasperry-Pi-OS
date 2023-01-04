#![feature(custom_test_frameworks)]
#![no_main]
#![no_std]
#![reexport_test_harness_main = "test_main"]
#![test_runner(libkernel::test_runner)]
#![feature(default_alloc_error_handler)]

extern crate alloc;
use alloc::sync::Arc;
use core::sync::atomic::{AtomicU64, Ordering};
use libkernel::{add_test, thread};

#[no_mangle]
fn kernel_main() {
    test_main()
}

add_test!(threading, {
    const NUM_THREADS: u64 = 1 << 12;
    const MAX_ACTIVE: u64 = 1 << 8;
    let counter: Arc<AtomicU64> = Arc::new(AtomicU64::new(0));

    for n in 0..NUM_THREADS {
        let counter_ = counter.clone();
        thread::schedule(thread::spawn(move || {
            assert!(counter_.fetch_add(1, Ordering::Relaxed) < NUM_THREADS);
        }));
        while n + 1 - counter.load(Ordering::Relaxed) > MAX_ACTIVE {
            thread::yield_now()
        }
    }

    while counter.load(Ordering::Relaxed) < NUM_THREADS {
        thread::yield_now();
    }

    assert!(counter.load(Ordering::Relaxed) == NUM_THREADS);
});

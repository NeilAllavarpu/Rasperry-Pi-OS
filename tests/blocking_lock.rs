#![feature(custom_test_frameworks)]
#![no_main]
#![no_std]
#![reexport_test_harness_main = "test_main"]
#![test_runner(libkernel::test_runner)]
#![feature(default_alloc_error_handler)]

extern crate alloc;
use alloc::sync::Arc;
use libkernel::{
    add_test,
    sync::{BlockingLock, Mutex},
    thread,
};

#[no_mangle]
fn kernel_main() {
    test_main()
}

add_test!(blocking_lock, {
    const NUM_THREADS: usize = 1 << 12;
    let outer = Arc::new(BlockingLock::new(0));
    let inner = Arc::new(BlockingLock::new(0));

    for _ in 0..NUM_THREADS {
        let _outer = Arc::clone(&outer);
        let _inner = Arc::clone(&inner);

        thread::schedule(thread::spawn(move || {
            let mut outer = _outer.lock();
            *_inner.lock() += 2;
            thread::yield_now();
            *outer += 1;
        }));
    }

    while *inner.lock() != NUM_THREADS * 2 {
        thread::yield_now()
    }
    while *outer.lock() != NUM_THREADS {
        thread::yield_now()
    }

    assert_eq!(*inner.lock(), NUM_THREADS * 2);
    assert_eq!(*outer.lock(), NUM_THREADS);
});

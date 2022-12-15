//! The initialization sequences

#![no_main]
#![no_std]
#![feature(custom_test_frameworks)]
#![feature(default_alloc_error_handler)]
#![feature(integer_atomics)]
#![feature(fn_traits)]
#![feature(format_args_nl)]
#![feature(once_cell)]
#![feature(panic_info_message)]
#![feature(pointer_byte_offsets)]
#![feature(ptr_metadata)]
#![feature(ptr_to_from_bits)]
#![reexport_test_harness_main = "test_main"]
#![test_runner(test_runner)]

extern crate alloc;

use alloc::sync::Arc;
use core::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
pub mod architecture;
pub mod board;
pub mod collections;
pub mod kernel;

/// The default runner for unit tests.
pub fn test_runner(tests: &[&TestCase]) -> ! {
    use crate::kernel::time::now;

    const DEFAULT_LOOPS: u64 = 16;
    let num_loops: u64 = option_env!("LOOP")
        .and_then(|v| str::parse(v).ok())
        .unwrap_or(DEFAULT_LOOPS);

    for test in tests {
        let running = Arc::new(AtomicU64::new(1));
        for i in 1..=num_loops {
            let running_ = Arc::clone(&running);
            let i_ = i;
            let timeout = test.timeout;

            // Timeout thread
            kernel::thread::schedule(thread!(move || {
                let start = now();
                while running_.load(Ordering::Relaxed) == i_ {
                    assert!(now() - start < timeout, "Test timed out");
                    kernel::thread::switch();
                }
            }));

            println!("[{}/{}] {}:", i, num_loops, test.name);

            let start = now();
            (test.test)();
            let end = now();
            running.fetch_add(1, Ordering::Relaxed);

            println!(".... PASSED: {:#?}", end - start);
        }
    }

    architecture::shutdown(0);
}

/// Registers a test to the given name
#[macro_export]
macro_rules! add_test {
    ($name: ident, $test: block) => {
        #[test_case]
        const $name: $crate::TestCase = $crate::TestCase {
            name: stringify!($name),
            test: || $test,
            timeout: core::time::Duration::from_secs(1),
        };
    };
    ($name: ident, $test: block, $timeout: expr) => {
        #[test_case]
        const $name: $crate::TestCase = $crate::TestCase {
            name: stringify!($name),
            test: || $test,
            timeout: $timeout,
        };
    };
}

/// Represents a test to run
pub struct TestCase {
    /// Name of the test.
    pub name: &'static str,

    /// Function pointer to the test.
    pub test: fn(),

    /// Timeout for the test, defaults to 1 second
    pub timeout: Duration,
}

#[cfg(test)]
#[no_mangle]
fn kernel_main() {
    test_main();
}

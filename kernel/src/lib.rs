//! The initialization sequences

#![no_main]
#![no_std]
#![feature(format_args_nl)]
#![feature(panic_info_message)]
#![feature(const_option)]
#![feature(once_cell)]
#![feature(strict_provenance_atomic_ptr)]
#![feature(result_option_inspect)]
#![feature(alloc_error_handler)]
#![feature(fn_traits)]
#![feature(ptr_to_from_bits)]
#![feature(linkage)]
#![feature(ptr_metadata)]
#![feature(custom_test_frameworks)]
#![feature(default_alloc_error_handler)]
#![feature(pointer_byte_offsets)]
#![forbid(unsafe_op_in_unsafe_fn)]
// etc
#![reexport_test_harness_main = "test_main"]
#![test_runner(crate::test_runner)]

extern crate alloc;

pub mod architecture;
pub mod board;
pub mod kernel;

/// The default runner for unit tests.
pub fn test_runner(tests: &[&test_types::UnitTest]) -> ! {
    use core::time::Duration;

    const NUM_LOOPS: u64 = 10;

    // Timeout thread
    use crate::kernel::timer::now;
    kernel::thread::schedule(kernel::thread::TCB::new(|| {
        let start = now();
        let timeout: Duration = Duration::from_secs(NUM_LOOPS * 5);

        loop {
            assert!(now() - start < timeout, "Test timed out");
        }
    }));

    // This line will be printed as the test headers
    println!("Running {} tests", tests.len());
    // println!()

    for test in tests {
        for i in 1..=NUM_LOOPS {
            println!("[{}/{}] {}:", i, NUM_LOOPS, test.name);

            // Run the actual test.
            (test.test_func)();

            println!(".... PASSED")
        }
    }

    architecture::shutdown(0);
}

#[cfg(test)]
#[no_mangle]
fn kernel_main() -> () {
    test_main();
}

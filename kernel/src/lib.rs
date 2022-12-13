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

    let num_loops: u64 = option_env!("LOOP")
        .and_then(|v| str::parse(v).ok())
        .unwrap_or(10);
    // Timeout thread
    kernel::thread::schedule(thread!(move || {
        use crate::kernel::timer::now;
        let start = now();
        let timeout: Duration = Duration::from_secs(num_loops);

        loop {
            assert!(now() - start < timeout, "Test timed out");
        }
    }));

    // This line will be printed as the test headers
    println!("Running {} tests", tests.len());
    // println!()

    for test in tests {
        for i in 1..=num_loops {
            println!("[{}/{}] {}:", i, num_loops, test.name);

            // Run the actual test.
            (test.test_func)();

            println!(".... PASSED")
        }
    }

    architecture::shutdown(0);
}

#[cfg(test)]
#[no_mangle]
fn kernel_main() {
    test_main();
}

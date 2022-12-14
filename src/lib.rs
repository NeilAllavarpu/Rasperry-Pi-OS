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
#![test_runner(test_runner)]

extern crate alloc;

pub mod architecture;
pub mod board;
pub mod kernel;

/// The default runner for unit tests.
pub fn test_runner(tests: &[&TestCase]) -> ! {
    use core::time::Duration;

    let num_loops: u64 = option_env!("LOOP")
        .and_then(|v| str::parse(v).ok())
        .unwrap_or(10);
    // Timeout thread
    kernel::thread::schedule(thread!(move || {
        use crate::kernel::time::now;
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
            (test.test)();

            println!(".... PASSED")
        }
    }

    architecture::shutdown(0);
}

/// Registers a test to the given name
#[macro_export]
macro_rules! add_test {
    ($name: ident, $test: block) => {
        #[test_case]
        #[allow(incorrect_ident_case)]
        const $name: $crate::TestCase = $crate::TestCase {
            name: stringify!($name),
            test: || $test,
        };
    };
}

/// Represents a test to run
pub struct TestCase {
    /// Name of the test.
    pub name: &'static str,

    /// Function pointer to the test.
    pub test: fn(),
}

#[cfg(test)]
#[no_mangle]
fn kernel_main() {
    test_main();
}

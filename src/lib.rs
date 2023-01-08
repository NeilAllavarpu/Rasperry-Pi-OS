#![no_main]
#![no_std]
#![feature(const_cmp)]
#![feature(const_convert)]
#![feature(const_default_impls)]
#![feature(const_nonnull_new)]
#![feature(const_mut_refs)]
#![feature(const_num_from_num)]
#![feature(const_option)]
#![feature(const_slice_index)]
#![feature(generic_arg_infer)]
#![feature(const_option_ext)]
#![feature(const_refs_to_cell)]
#![feature(const_result_drop)]
#![feature(const_trait_impl)]
#![feature(custom_test_frameworks)]
#![feature(default_alloc_error_handler)]
#![feature(duration_constants)]
#![feature(fn_traits)]
#![feature(format_args_nl)]
#![feature(inline_const)]
#![feature(let_chains)]
#![feature(once_cell)]
#![feature(panic_info_message)]
#![feature(pointer_byte_offsets)]
#![feature(pointer_is_aligned)]
#![feature(ptr_mask)]
#![feature(ptr_metadata)]
#![feature(ptr_to_from_bits)]
#![feature(strict_provenance)]
#![reexport_test_harness_main = "test_main"]
#![test_runner(test_runner)]

extern crate alloc;
pub mod architecture;
pub mod board;
pub mod cell;
pub mod collections;
pub mod kernel;
pub mod macros;
pub mod memory;
pub mod sync;
pub mod thread;

/// The default runner for unit tests.
pub fn test_runner(tests: &[&TestCase]) -> ! {
    const DEFAULT_LOOPS: u64 = 16;
    let num_loops: u64 = option_env!("LOOP")
        .and_then(|v| str::parse(v).ok())
        .unwrap_or(DEFAULT_LOOPS);

    for test in tests {
        for i in 1..=num_loops {
            use crate::architecture::time::now;

            println!("[{}/{}] {}:", i, num_loops, test.name);

            let start = now();
            (test.test)();
            let end = now();

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
    pub timeout: core::time::Duration,
}

#[cfg(test)]
#[no_mangle]
fn kernel_main() {
    test_main();
}

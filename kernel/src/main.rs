//! The initialization sequences

#![no_main]
#![no_std]
#![feature(format_args_nl)]
#![feature(panic_info_message)]
#![feature(const_option)]
#![feature(once_cell)]
#![feature(strict_provenance_atomic_ptr)]
#![feature(result_option_inspect)]
#![feature(fn_traits)]
#![feature(ptr_to_from_bits)]
#![feature(ptr_metadata)]
#![feature(alloc_error_handler)]
#![feature(extend_one)]
#![feature(default_alloc_error_handler)]
#![feature(pointer_byte_offsets)]
#![forbid(unsafe_op_in_unsafe_fn)]
#![feature(pointer_is_aligned)]
#![reexport_test_harness_main = "test_main"]
#![feature(custom_test_frameworks)]
#![warn(clippy::correctness)]
#![warn(clippy::pedantic)]
#![warn(clippy::suspicious)]
#![warn(clippy::complexity)]
#![warn(clippy::perf)]
#![warn(clippy::style)]

extern crate alloc;

mod architecture;
mod board;
mod kernel;

#[no_mangle]
pub fn kernel_main() {}

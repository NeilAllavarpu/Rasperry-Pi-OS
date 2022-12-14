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
#![test_runner(_test_runner)]
#![feature(custom_test_frameworks)]
#![warn(clippy::correctness)]
#![feature(stmt_expr_attributes)]
#![warn(clippy::pedantic)]
#![warn(clippy::suspicious)]
#![warn(clippy::complexity)]
#![warn(clippy::perf)]
#![warn(clippy::style)]
#![warn(clippy::restriction)]
#![allow(clippy::arithmetic_side_effects)]
#![allow(clippy::implicit_return)]
#![allow(clippy::integer_arithmetic)]
#![allow(clippy::panic)]
#![allow(clippy::unreachable)]
#![allow(clippy::expect_used)]
#![allow(clippy::separated_literal_suffix)]
#![allow(clippy::missing_trait_methods)]
#![allow(clippy::integer_division)]
#![allow(clippy::single_char_lifetime_names)]
#![allow(clippy::partial_pub_fields)]
#![allow(clippy::pub_use)]
#![allow(clippy::self_named_module_files)]
#![allow(clippy::default_numeric_fallback)]

extern crate alloc;

/// Dummy function for rust-analyzer issues
fn _test_runner(_: &[&()]) {}

/// Architecture-specific implementations
mod architecture;
/// Board-specific implementaitons
mod board;
/// Generic implementations
mod kernel;

#[no_mangle]
/// The default main sequence
pub fn kernel_main() {}

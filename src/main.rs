//! The initialization sequences

#![no_main]
#![no_std]
#![feature(custom_test_frameworks)]
#![feature(default_alloc_error_handler)]
#![feature(fn_traits)]
#![feature(format_args_nl)]
#![feature(integer_atomics)]
#![feature(once_cell)]
#![feature(panic_info_message)]
#![feature(pointer_byte_offsets)]
#![feature(ptr_metadata)]
#![feature(ptr_to_from_bits)]
#![feature(stmt_expr_attributes)]
#![reexport_test_harness_main = "test_main"]
#![test_runner(_test_runner)]
#![forbid(unsafe_op_in_unsafe_fn)]
#![warn(clippy::correctness)]
#![warn(clippy::pedantic)]
#![warn(clippy::suspicious)]
#![warn(clippy::complexity)]
#![warn(clippy::perf)]
#![warn(clippy::style)]
#![allow(clippy::blanket_clippy_restriction_lints)]
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
#![allow(clippy::new_without_default)]

extern crate alloc;

/// Dummy function for rust-analyzer issues
fn _test_runner(_: &[&()]) {}

/// Architecture-specific implementations
mod architecture;
/// Board-specific implementaitons
mod board;
/// Collections
mod collections;
/// Generic implementations
mod kernel;

#[no_mangle]
/// The default main sequence
pub fn kernel_main() {}

/// Dummy macro for kernel tests, replaced by `lib.rs` when tests
#[macro_export]
macro_rules! add_test {
    ($name: ident, $test: block) => {};
}

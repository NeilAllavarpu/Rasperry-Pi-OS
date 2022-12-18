//! A Raspberry Pi Operating System
#![no_main]
#![no_std]
#![feature(const_default_impls)]
#![feature(const_refs_to_cell)]
#![feature(const_trait_impl)]
#![feature(custom_test_frameworks)]
#![feature(default_alloc_error_handler)]
#![feature(fn_traits)]
#![feature(format_args_nl)]
#![feature(integer_atomics)]
#![feature(let_chains)]
#![feature(maybe_uninit_uninit_array)]
#![feature(once_cell)]
#![feature(panic_info_message)]
#![feature(pointer_byte_offsets)]
#![feature(ptr_metadata)]
#![feature(ptr_to_from_bits)]
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

use aarch64_cpu::asm::wfi;

extern crate alloc;

/// Dummy function for rust-analyzer issues
fn _test_runner(_: &[&()]) {}

/// Architecture-specific implementations
mod architecture;
/// Board-specific implementaitons
mod board;
/// Additional cells
pub mod cell;
/// Collections
mod collections;
/// Generic implementations
mod kernel;
/// Useful macros
mod macros;
/// Synchronization primitives
mod sync;

#[no_mangle]
/// The default main sequence
pub fn kernel_main() {
    log!("Kernel main running");
    loop {
        wfi();
    }
}

/// Dummy macro for kernel tests, replaced by `lib.rs` when tests
#[macro_export]
macro_rules! add_test {
    ($name: ident, $test: block) => {};
}

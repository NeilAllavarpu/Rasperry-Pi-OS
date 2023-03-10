//! A Raspberry Pi Operating System
#![no_main]
#![no_std]
#![feature(const_cmp)]
#![feature(const_convert)]
#![feature(const_default_impls)]
#![feature(const_nonnull_new)]
#![feature(const_mut_refs)]
#![feature(const_num_from_num)]
#![feature(const_option)]
#![feature(const_option_ext)]
#![feature(const_slice_index)]
#![feature(const_refs_to_cell)]
#![feature(const_result_drop)]
#![feature(const_trait_impl)]
#![feature(custom_test_frameworks)]
#![feature(default_alloc_error_handler)]
#![feature(duration_constants)]
#![feature(fn_traits)]
#![feature(format_args_nl)]
#![feature(inline_const)]
#![feature(generic_arg_infer)]
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
#![allow(clippy::module_name_repetitions)]

use aarch64_cpu::asm::wfi;

extern crate alloc;

/// Dummy function for rust-analyzer issues
fn _test_runner(_: &[&()]) {}

/// Architecture-specific implementations
mod architecture;
/// Board-specific implementaitons
mod board;
/// Additional cells
mod cell;
/// Collections
mod collections;
/// Generic implementations
mod kernel;
/// Useful macros
mod macros;
/// MMU + Virtual Memory
mod memory;
/// Synchronization primitives
mod sync;
/// Native threads
mod thread;

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

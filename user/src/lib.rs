//! Usermode library OS to interface with the kernel and other user programs, and provide the basic abstractions of a standard monolithic kernel

#![no_main]
#![no_std]
#![warn(clippy::complexity)]
#![deny(clippy::correctness)]
#![warn(clippy::nursery)]
#![warn(clippy::pedantic)]
#![deny(clippy::perf)]
#![warn(clippy::restriction)]
#![warn(clippy::style)]
#![deny(clippy::suspicious)]
#![deny(unsafe_op_in_unsafe_fn)]
#![feature(used_with_arg)]
#![expect(
    clippy::allow_attributes,
    reason = "Unable to disable this just for some macros"
)]
#![expect(
    clippy::allow_attributes_without_reason,
    reason = "Issue with linting irrelevant statements"
)]
#![expect(
    clippy::bad_bit_mask,
    reason = "Unable to disable this just for some macros"
)]
#![expect(
    clippy::blanket_clippy_restriction_lints,
    reason = "This is intentionally enabled"
)]
#![expect(clippy::implicit_return, reason = "This is the desired format")]
#![expect(
    clippy::inline_asm_x86_intel_syntax,
    reason = "This is not targeted at x86"
)]
#![expect(
    clippy::integer_division,
    reason = "This is used with acceptable or intended rounding"
)]
#![expect(clippy::mod_module_files, reason = "This is the desired format")]
#![expect(clippy::question_mark_used, reason = "This is the desired format")]
#![expect(clippy::semicolon_inside_block, reason = "This is the desired format")]
#![expect(
    clippy::separated_literal_suffix,
    reason = "This is the desired format"
)]
#![expect(clippy::pub_with_shorthand)]
#![expect(clippy::single_call_fn)]
#![expect(clippy::shadow_same)]
#![expect(clippy::shadow_reuse)]
#![expect(clippy::unreachable)]
#![feature(allocator_api)]
#![feature(asm_const)]
#![feature(const_mut_refs)]
#![feature(const_ptr_as_ref)]
#![feature(const_option)]
#![feature(fn_traits)]
#![feature(format_args_nl)]
#![feature(generic_arg_infer)]
#![feature(generic_const_exprs)]
#![feature(let_chains)]
#![feature(lint_reasons)]
#![feature(linkage)]
#![feature(inline_const)]
#![feature(int_roundings)]
#![feature(naked_functions)]
#![feature(nonzero_ops)]
#![feature(panic_info_message)]
#![feature(pointer_is_aligned)]
#![feature(slice_ptr_get)]
#![feature(stdsimd)]
#![feature(stmt_expr_attributes)]
#![feature(strict_provenance)]
#![feature(sync_unsafe_cell)]
#![feature(ptr_mask)]
#![feature(unchecked_math)]
#![feature(never_type)]
#![feature(unchecked_shifts)]
#![feature(c_size_t)]
#![feature(alloc_layout_extra)]
#![feature(strict_provenance_atomic_ptr)]
#![feature(non_null_convenience)]
#![feature(unnamed_fields)]
use core::{
    ffi::c_int,
    fmt::{Error, Write},
    panic::PanicInfo,
    sync::atomic::AtomicU32,
};

extern crate alloc;
use bump_allocator::BumpAllocator;
use os::syscalls;

const EOF: c_int = -1;
pub type Result<T> = core::result::Result<T, errno::Error>;
pub mod bump_allocator;
pub mod cell;
pub mod errno;
pub mod os;
pub mod pid_map;
pub mod runtime;
pub mod signal;
pub mod stdio;
pub mod sync;
pub mod sys;
pub mod unistd;

/// The global heap allocator for the kernel
#[global_allocator]
static mut KERNEL_ALLOCATOR: BumpAllocator = BumpAllocator::empty();

pub struct Stdout;
impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        syscalls::write(s.as_bytes()).then_some(()).ok_or(Error)
    }
}

#[macro_export]
macro_rules! println {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        writeln!(&mut $crate::Stdout {}, $($arg)*).unwrap();
    }};
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {{
        use core::fmt::Write;
        write!(&mut $crate::Stdout{}, $($arg)*).unwrap();
    }};
}

/// PANIC HANDLER
#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    match (info.location(), info.message()) {
        (None, None) => println!("thread 'main' panicked"),
        (None, Some(message)) => println!("thread 'main' panicked:\n{message}"),
        (Some(location), None) => println!("thread 'main' panicked at {location}"),
        (Some(location), Some(message)) => {
            println!("thread 'main' panicked at {location}:\n{message}");
        }
    }
    syscalls::exit()
}

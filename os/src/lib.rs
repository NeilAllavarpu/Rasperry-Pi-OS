//! The libary operating system portion of the OS, residing in userspace. This implements any
//! functionality safely permissible within a process' own execution, or calls another process or
//! the kernel if unable to do so.

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

use core::{
    fmt::{Error, Write},
    ptr::NonNull,
};

pub mod cell;
// pub mod heap;
pub mod os;
pub mod sync;

#[inline]
pub fn write(bytes: &[u8]) -> bool {
    let status: u64;
    unsafe {
        core::arch::asm! {
            "svc 0x1000",
            inout("x0") bytes.as_ptr() => status,
            in("x1") bytes.len(),
            options(nostack, readonly),
            clobber_abi("C"),
        }
    }
    match status {
        0 => true,
        1 => false,
        _ => unreachable!("Write syscall returned an invalid success/failure value"),
    }
}
pub struct Stdout;
impl Write for Stdout {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        write(s.as_bytes()).then_some(()).ok_or(Error)
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

//! The initialization sequences

#![no_main]
#![no_std]
#![feature(format_args_nl)]
#![feature(panic_info_message)]
#![feature(const_option)]
#![feature(once_cell)]
#![feature(result_option_inspect)]
#![feature(default_alloc_error_handler)]

extern crate alloc;

mod architecture;
mod board;
mod kernel;

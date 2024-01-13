//! PIPES!

#![no_std]
#![no_main]
#![feature(naked_functions)]
#![feature(asm_const)]
#![feature(inline_const)]
#![feature(generic_arg_infer)]
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
#![feature(lint_reasons)]
#![feature(allocator_api)]
#![expect(
    clippy::allow_attributes,
    reason = "Unable to disable this just for some macros"
)]
#![expect(clippy::shadow_reuse)]
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
#![feature(maybe_uninit_slice)]

use crate::{
    process::{CreateError, PROCESSES},
    service_channel::{ReadError, Request, Response, WriteError},
};
use alloc::{collections::vec_deque::Drain, sync::Arc};
use user::{os::syscalls, println};

extern crate alloc;
mod pipe;
mod process;
mod service_channel;

#[no_mangle]
extern "C" fn main() -> ! {
    // simple_signal(handle_message)
    let mut count = 0;
    println!d("Pipin time");
    loop {
        syscalls::block();
        println!("Simple signal: {count}!");
        count += 1;
    }
}

/// Handler when a message is delivered to this process by some
extern "C" fn handle_message(request_pid: u16) {
    let mut processes = PROCESSES.lock();
    let Some(process) = processes.get_mut(request_pid) else {
        println!("Unknown PID {request_pid}");
        return;
    };
    while let Some(message) = process.channel.incoming.read_message() {
        println!("message received!");
        let response: Response<Drain<u8>> = match message {
            Request::Read(pipe_id, count) => match process.get_read(pipe_id) {
                Some(pipe) => {
                    let pipe = Arc::clone(pipe);
                    let mut pipe = pipe.lock();
                    let bytes = pipe.read(count);
                    drop(message);
                    process
                        .channel
                        .outgoing
                        .write_message(Response::Read(bytes));
                    continue;
                }
                None => Response::ReadFailure(ReadError::NoSuchPipe),
            },
            Request::Write(pipe_id, bytes) => process.get_write(pipe_id).map_or(
                Response::WriteFailure(WriteError::NoSuchPipe),
                |pipe| {
                    let pipe = Arc::clone(pipe);
                    let mut pipe = pipe.lock();
                    pipe.write(bytes.iter().copied());
                    Response::Write
                },
            ),
            Request::Fork(target_pid) => todo!(),
            Request::Create => match process.create_pipe() {
                Ok(pid) => Response::Create(pid),
                Err(CreateError::MaxPipeCount) => Response::CreateFailure,
                Err(CreateError::NoMemory) => todo!(),
            },
            Request::DropRead(pipe_id) => match process.drop_read(pipe_id) {
                Ok(()) => Response::DropRead,
                Err(err) => Response::DropReadFailure(err),
            },
            Request::DropWrite(pipe_id) => match process.drop_write(pipe_id) {
                Ok(()) => Response::DropWrite,
                Err(err) => Response::DropWriteFailure(err),
            },
        };
        process.channel.outgoing.write_message(response);
    }
}

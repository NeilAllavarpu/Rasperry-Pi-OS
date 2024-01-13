use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use crate::main;
use core::{
    arch,
    ffi::{c_int, c_void},
    hint,
    ops::Deref,
    ptr::{self, addr_of_mut, NonNull},
    sync::atomic::{AtomicPtr, Ordering},
};

/// User context, compatible with the kernel's view of this struct
#[repr(C)]
pub struct UserContext {
    /// The code that is invoked when the kernel delivers an exception to this program
    exception_vector: unsafe extern "C" fn(u64),
    /// The exception stack used for the kernel to store arguments and other exception-related information
    pub(crate) exception_stack: AtomicPtr<u64>,
}

/// The context set for this program when initially loaded
pub(crate) static CONTEXT: UserContext = UserContext {
    exception_vector: _exception_handler,
    exception_stack: AtomicPtr::new(unsafe { addr_of_mut!(EXCEPTION_STACK[1]) }.cast()),
};

impl UserContext {
    fn pop_value(&self) -> u64 {
        let mut val = 0;
        self.exception_stack
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| {
                let new = unsafe { v.sub(1) };
                val = unsafe { new.read() };
                Some(new)
            })
            .unwrap();
        val
    }
}

/// Memory for the exception stack to save context
static mut EXCEPTION_STACK: [unsafe extern "C" fn(*mut u64, u16, usize) -> !; 8] =
    [main, main, main, main, main, main, main, main];

/// Save region for the stack pointer
pub static mut SP: u64 = 0x1_0000 - 0x100;

/// The handlers to which to dispatch to; index determines which code is used
static HANDLER_TABLE: [AtomicPtr<()>; 4] = [
    AtomicPtr::new(preemption as *mut ()),
    AtomicPtr::new(resumption as *mut ()),
    AtomicPtr::new(resumption as *mut ()),
    AtomicPtr::new(resumption as *mut ()),
];

/// The exception handler invoked when this program receives any sort of exception.
/// Dispatches to other exception handlers based on the read exception code.
/// # Safety
/// Must only be called by the kernel's exception mechanism:
/// * The exception stack must contain an exception code, return address if applicable, and any arguments if applicable
/// * The stack pointer must be zeroed only if a stack pointer was properly saved prior (e.g. by the preemption handler); it must be valid otherwise
#[naked]
unsafe extern "C" fn _exception_handler(exception_code: u64) {
    // SAFETY: The caller upholds safety guarantees
    unsafe {
        arch::asm! {
            "cbz x0, {preemption}",
            "tbz x0, 0, 0f",
            "ldr x2, {sp}",
            "mov sp, x2",
            "cmp x0, 1",
            "b.eq {resumption}",
            "0:",
            "stp x20,  lr, [sp, -0xA0]!",
            "stp x2,  x3,  [sp, 0x10]",
            "stp x4,  x5,  [sp, 0x20]",
            "stp x6,  x7,  [sp, 0x30]",
            "stp x8,  x9,  [sp, 0x40]",
            "stp x10, x11, [sp, 0x50]",
            "stp x12, x13, [sp, 0x60]",
            "stp x14, x15, [sp, 0x70]",
            "stp x16, x17, [sp, 0x80]",
            "mrs x17, NZCV",
            "stp x18, x17, [sp, 0x90]",
            "bl {handler}",
            "msr NZCV, x17",
            "ldp x16, x17, [sp, 0x80]",
            "ldp x14, x15, [sp, 0x70]",
            "ldp x12, x13, [sp, 0x60]",
            "ldp x10, x11, [sp, 0x50]",
            "ldp x8,  x9,  [sp, 0x40]",
            "ldp x6,  x7,  [sp, 0x30]",
            "ldp x4,  x5,  [sp, 0x20]",
            "ldp x2,  x3,  [sp, 0x10]",
            "ldp x20,  lr,  [sp], 0xA0",
            "svc 0x0",
            preemption = sym preemption,
            resumption = sym resumption,
            handler = sym general_handler,
            sp = sym SP,
            options(noreturn)
        }
    }
}

/// Handler invoked when a program is being preempted
///
/// # Safety
/// This function must only be called when context is properly set up:
/// * registers `x0`/`x1` must be properly saved on the stack
unsafe extern "C" fn preemption() {
    // SAFETY: The caller upholds safety guarantees
    unsafe {
        arch::asm! {
            // LOAD x0 FROM EXCEPTION STACK!!!!!!
            "stp x0,  x1,  [sp, -0x100]!",
            "stp x2,  x3,  [sp, 0x10]",
            "stp x4,  x5,  [sp, 0x20]",
            "stp x6,  x7,  [sp, 0x30]",
            "stp x8,  x9,  [sp, 0x40]",
            "stp x10, x11, [sp, 0x50]",
            "stp x12, x13, [sp, 0x60]",
            "stp x14, x15, [sp, 0x70]",
            "stp x16, x17, [sp, 0x80]",
            "stp x18, x19, [sp, 0x90]",
            "stp x20, x21, [sp, 0xA0]",
            "stp x22, x23, [sp, 0xB0]",
            "stp x24, x25, [sp, 0xC0]",
            "stp x26, x27, [sp, 0xD0]",
            "mrs x27, NZCV",
            "stp x28, x27, [sp, 0xE0]",
            "stp x29, x30, [sp, 0xF0]",
            "mov x0, sp",
            "adr x1, {sp}",
            "str x0, [x1]",
            "svc 0x9999", // TODO: EOI SVC?
            sp = sym SP,
            options(noreturn)
        }
    }
}

/// Handler invoked when resuming a program after being suspended
/// # Safety
/// This function must only be called when context is properly set up:
/// * the exception stack must contain a valid return address
/// * register state must be saved properly on the stack
#[naked]
unsafe extern "C" fn resumption() {
    // SAFETY: The caller upholds safety guarantees
    unsafe {
        arch::asm! {
            "ldp x2,  x3,  [sp, 0x10]",
            "ldp x4,  x5,  [sp, 0x20]",
            "ldp x6,  x7,  [sp, 0x30]",
            "ldp x8,  x9,  [sp, 0x40]",
            "ldp x10, x11, [sp, 0x50]",
            "ldp x12, x13, [sp, 0x60]",
            "ldp x14, x15, [sp, 0x70]",
            "ldp x16, x17, [sp, 0x80]",
            "ldp x18, x19, [sp, 0x90]",
            "ldp x20, x21, [sp, 0xA0]",
            "ldp x22, x23, [sp, 0xB0]",
            "ldp x24, x25, [sp, 0xC0]",
            "ldp x26, x27, [sp, 0xD0]",
            "ldp x28, x29, [sp, 0xE0]",
            "msr  NZCV, x29",
            "ldp x29, x30, [sp, 0xF0]",
            "ldp x0,  x1,  [sp], 0x100",
            "svc 0x0",
            options(noreturn)
        }
    }
}

#[derive(FromPrimitive)]
enum ExceptionCode {
    Preemption = 0,
    Resumption = 1,
    PageFault = 2,
    UserSignal = 3,
}

#[repr(C)]
struct ReturnRegs {
    x0: u64,
    x1: u64,
}

/// Rust handler invoked when any exception occurs
extern "C" fn general_handler(exception_code: u64, arg0: u64) -> ReturnRegs {
    match FromPrimitive::from_u64(exception_code) {
        Some(ExceptionCode::Preemption) => {
            unreachable!("Preemption should not reach the general handler")
        }
        Some(ExceptionCode::Resumption) => {
            unreachable!("Resumption should not reach the general handler")
        }
        Some(ExceptionCode::PageFault) => {
            handle_page_fault(arg0);
            let x1 = CONTEXT.pop_value();
            let x0 = CONTEXT.pop_value();
            ReturnRegs { x0, x1 }
        }
        Some(ExceptionCode::UserSignal) => {
            handle_user_signal(u16::try_from(arg0).expect("PID should be valid"));
            ReturnRegs {
                x0: exception_code,
                x1: arg0,
            }
        }
        None => {
            unreachable!("Unexpected signal code: {exception_code}")
        }
    }
}

/// Handler when the kernel delivers a page fault to this process. Resolves abstractions such as `mmap` before dispatching to the user handler, if necessary
extern "C" fn handle_page_fault(faulting_info: u64) {
    panic!("Page fault occured! Faulting information: {faulting_info:X}");
}

/// Handler when a signal is delivered from another process
extern "C" fn handle_user_signal(sender_pid: u16) {
    panic!("User signal occured! Sender: {sender_pid}");
}

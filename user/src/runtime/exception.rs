use alloc::boxed::Box;
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;

use crate::{
    println,
    signal::ffi::{SigInfo, SigVal},
    sys::types::ffi::pid_t,
};

use super::init::_start;
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
static mut EXCEPTION_STACK: [unsafe extern "C" fn(); 8] = [
    _start, _start, _start, _start, _start, _start, _start, _start,
];

/// Save region for the stack pointer
pub static mut SP: u64 = 0;

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
            "cmp x0, 1",
            "b.eq {resumption}",
            "tbz x0, 1, 0f",
            "ldr x2, {sp}",
            "mov sp, x2",
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
            "ldp x0,  x1,  [sp], 0x10",
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
            "ldp x0,  x1,  [sp, 0x10]",
            "ldp x2,  x3,  [sp, 0x20]",
            "ldp x4,  x5,  [sp, 0x30]",
            "ldp x6,  x7,  [sp, 0x40]",
            "ldp x8,  x9,  [sp, 0x50]",
            "ldp x10, x11, [sp, 0x60]",
            "ldp x12, x13, [sp, 0x70]",
            "ldp x14, x15, [sp, 0x80]",
            "ldp x16, x17, [sp, 0x90]",
            "ldp x18, x19, [sp, 0xA0]",
            "ldp x20, x21, [sp, 0xB0]",
            "ldp x22, x23, [sp, 0xC0]",
            "ldp x24, x25, [sp, 0xD0]",
            "ldp x26, x27, [sp, 0xE0]",
            "ldp x28, x29, [sp, 0xF0]",
            "msr  NZCV, x29",
            "ldp x29, x30, [sp, 0x100]",
            "add  sp,  sp,   0x110",
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

pub enum HandlerType {
    Signal(unsafe extern "C" fn(c_int)),
    SigAction(unsafe extern "C" fn(c_int, *mut SigInfo, *mut c_void)),
}
pub(crate) struct SignalInfo {
    handler: HandlerType,
    switch_stack: bool,
}

pub struct AtomicBox<T>(AtomicPtr<T>);

struct TaggedPointer<T>(*mut T);

impl<T> TaggedPointer<T> {
    fn ptr(self) -> *mut T {
        self.0
    }

    fn tag(self) -> u8 {
        u8::try_from(self.0.addr() >> 56).unwrap()
    }
}

impl<T> Clone for TaggedPointer<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for TaggedPointer<T> {}

impl<T> AtomicBox<T> {
    const unsafe fn new() -> Self {
        Self(AtomicPtr::new(ptr::null_mut()))
    }

    /// reeeeplace
    pub fn replace(&self, info: Box<T>) -> Option<Box<T>> {
        let raw_ptr = Box::into_raw(info);
        let prev_ptr = loop {
            let prev_ptr = TaggedPointer(self.0.load(Ordering::Relaxed));
            if prev_ptr.tag() == 0
                && let Ok(previous_ptr) = self.0.compare_exchange(
                    prev_ptr.0,
                    raw_ptr,
                    Ordering::AcqRel,
                    Ordering::Relaxed,
                )
            {
                break previous_ptr;
            }
            hint::spin_loop();
        };
        NonNull::new(prev_ptr).map(|ptr| unsafe { Box::from_raw(ptr.as_ptr()) })
    }

    ///reaads
    #[allow(clippy::unwrap_in_result)]
    pub fn read(&self) -> Option<AtomicBoxGuard<T>> {
        let raw_ptr = self
            .0
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |ptr| {
                if ptr.is_null() {
                    None
                } else {
                    Some(ptr.map_addr(|addr| {
                        let addr_portion = addr & 0xFF_FFFF_FFFF_FFFF;
                        let top_bits = u8::try_from(addr >> 56).unwrap();
                        let new_count = top_bits.checked_add(1).unwrap();
                        addr_portion & (usize::from(new_count) << 56)
                    }))
                }
            });
        match raw_ptr {
            Ok(raw_ptr) => Some(AtomicBoxGuard {
                container: self,
                reference: unsafe { raw_ptr.as_ref() }.unwrap(),
            }),
            Err(p) => {
                assert!(p.is_null());
                None
            }
        }
    }

    unsafe fn decrement_readers(&self) {
        self.0
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |ptr| {
                assert!(!ptr.is_null());
                Some(ptr.map_addr(|addr| {
                    let addr_portion = addr & 0xFF_FFFF_FFFF_FFFF;
                    let top_bits = u8::try_from(addr >> 56).unwrap();
                    let new_count = top_bits.checked_sub(1).unwrap();
                    addr_portion & (usize::from(new_count) << 56)
                }))
            })
            .unwrap();
    }
}

pub struct AtomicBoxGuard<'reference, 'data, T>
where
    'data: 'reference,
{
    container: &'data AtomicBox<T>,
    reference: &'reference T,
}

impl<'reference, 'data, T> Drop for AtomicBoxGuard<'reference, 'data, T> {
    fn drop(&mut self) {
        unsafe { self.container.decrement_readers() };
    }
}

impl<'reference, 'data, T> Deref for AtomicBoxGuard<'reference, 'data, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.reference
    }
}

/// Signal handlers for
pub(crate) static SIGNAL_HANDLERS: [AtomicBox<SignalInfo>; 4] =
    [const { unsafe { AtomicBox::new() } }; 4];

/// Rust handler invoked when any exception occurs
extern "C" fn general_handler(exception_code: u64, arg0: u64, sp: usize) -> ReturnRegs {
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
            handle_user_signal(pid_t::try_from(arg0).expect("PID should be valid"));
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
    println!("User signal occured! Sender: {sender_pid}");
    if let Some(handler_info) = SIGNAL_HANDLERS[ExceptionCode::UserSignal as usize].read() {
        match handler_info.handler {
            HandlerType::Signal(handler) => unsafe { handler(ExceptionCode::UserSignal as _) },
            HandlerType::SigAction(handler) => {
                let siginfo = SigInfo {
                    si_addr: ptr::null_mut(),
                    si_band: 0,
                    // what exactly is this value?
                    si_value: SigVal {
                        sival_int: ExceptionCode::UserSignal as _,
                    },
                    si_signo: ExceptionCode::UserSignal as _,
                    si_code: todo!("SI_USER"),
                    si_errno: 0, // check?
                    si_status: 0,
                    si_pid: sender_pid,
                    si_uid: 0,
                };
                unsafe {
                    handler(
                        ExceptionCode::UserSignal as _,
                        addr_of_mut!(siginfo),
                        ptr::null_mut(),
                    )
                }
            }
        }
    }
}

#![no_main]
#![no_std]
#![feature(naked_functions)]
#![feature(asm_const)]
#![feature(inline_const)]
#![feature(generic_arg_infer)]

use core::sync::atomic::{AtomicPtr, AtomicU64};

#[repr(C)]
struct UserContext {
    exception_vector: extern "C" fn() -> !,
    exception_stack: AtomicPtr<u64>,
    exception_stack_mem: &'static [AtomicU64],
}

static CONTEXT: UserContext = UserContext {
    exception_vector: _start,
    exception_stack: AtomicPtr::new(EXCEPTION_STACK[0].as_ptr()),
    exception_stack_mem: &EXCEPTION_STACK,
};

static EXCEPTION_STACK: [AtomicU64; 8] = [const { AtomicU64::new(0) }; _];

#[no_mangle]
#[link_section = ".init"]
#[naked]
extern "C" fn _start() -> ! {
    unsafe {
        core::arch::asm! {
            "adr x0, {CONTEXT}",
            "adr x0, 0f",
            "mov x1, 10",
            "svc #0x1000",
            "svc #0x2000",
            "0: .string \"Hello from UART\"",
            CONTEXT = sym CONTEXT,
            // in ("x0") bytes.as_ptr(),
            // in ("x1") bytes.len(),
            options(noreturn)
        }
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    loop {}
}

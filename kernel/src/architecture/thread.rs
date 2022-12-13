use crate::kernel::{self, thread::Thread};
use aarch64_cpu::registers::TPIDR_EL1;
use alloc::sync::Arc;
use core::{
    arch::global_asm,
    ptr::{self, Pointee},
};
use tock_registers::interfaces::{Readable, Writeable};

/// Runs the given closure with the current thread as a parameter
pub fn me<T, Work: FnOnce(&mut Thread) -> T>(work: Work) -> T {
    work.call_once(
        (unsafe { &mut *<*mut Thread>::from_bits(TPIDR_EL1.get().try_into().unwrap()) },),
    )
}

/// Sets the currently running thread
/// # Safety
/// Only the initialization sequence should call this, using the idle threads
pub unsafe fn set_me(thread: Arc<Thread>) {
    // thread.
    TPIDR_EL1.set(Arc::into_raw(thread).to_bits().try_into().unwrap());
}

#[no_mangle]
extern "C" fn thread_trampoline() -> ! {
    unsafe { me(|me| me.run()) }
}

/// Creates a stack appropriate to trampoline start a thread
/// # Safety
/// `stack_top` must be a valid pointer to the top of a newly allocated stack
pub unsafe fn set_up_stack(stack_top: *mut u128) -> *mut u128 {
    unsafe {
        let desired_top = stack_top.byte_sub(0x70);
        // The upper 64 bits of the entry store the LR to return to
        // The lower 64 bits of the entry store the FP,
        // zeroed here to indicate the end of the call chain
        desired_top
            .write(u128::try_from((thread_trampoline as *const fn()).to_bits()).unwrap() << 64);
        desired_top
    }
}

global_asm!(include_str!("context_switch.s"));
extern "C" {
    #[allow(improper_ctypes)]
    fn _context_switch(
        data: *mut (),
        metadata: *const (),
        new_thread: *mut Thread,
        callback: extern "C" fn(data_address: *mut (), metadata: *const (), thread: *mut Thread),
    );
}

extern "C" fn invoke_callback<Callback>(
    data_address: *mut (),
    metadata: *const <Callback as Pointee>::Metadata,
    previous_thread: *mut Thread,
) where
    Callback: FnMut(Arc<Thread>),
{
    unsafe { ptr::from_raw_parts_mut::<Callback>(data_address, *metadata).as_mut() }
        .unwrap()
        .call_once((unsafe { Arc::from_raw(previous_thread) },))
}

/// Context switches into the given thread, and invokes the callback after switching threads
pub fn context_switch<Callback>(new_thread: Arc<Thread>, mut data: Callback)
where
    Callback: FnMut(Arc<Thread>),
{
    me(|me| {
        me.runtime += kernel::timer::now() - me.last_started;
        let (data, metadata): (*mut (), <Callback as Pointee>::Metadata) =
            ptr::addr_of_mut!(data).to_raw_parts();
        unsafe {
            _context_switch(
                data,
                &metadata,
                Arc::into_raw(new_thread) as *mut Thread,
                invoke_callback::<Callback>,
            )
        }
        me.last_started = kernel::timer::now()
    });
}

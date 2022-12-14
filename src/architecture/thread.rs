use crate::{
    architecture,
    kernel::{self, thread::Thread},
};
use aarch64_cpu::registers::TPIDR_EL1;
use alloc::sync::Arc;
use core::{
    arch::global_asm,
    ptr::{self, Pointee},
};
use tock_registers::interfaces::{Readable, Writeable};

/// Runs the given closure with the current thread as a parameter
pub fn me<T, Work: FnOnce(&mut Thread) -> T>(work: Work) -> T {
    work.call_once((
        // SAFETY: `TPIDR_EL1` is set up during the initialization seqeunce
        // and during context switches so that it always contains a valid pointer
        // to the current thread
        unsafe { &mut *<*mut Thread>::from_bits(architecture::u64_to_usize(TPIDR_EL1.get())) },
    ))
}

/// Sets the currently running thread
/// # Safety
/// Only the initialization sequence should call this, using the idle threads
pub unsafe fn set_me(thread: Arc<Thread>) {
    // thread.
    TPIDR_EL1.set(architecture::usize_to_u64(Arc::into_raw(thread).to_bits()));
}

/// The first path of execution for a newly created thread
/// Jumps directly to the run method for the thread
#[no_mangle]
extern "C" fn thread_trampoline() -> ! {
    me(|me|
        // SAFETY: This is the only valid time to `run` a newly created thread
        unsafe { me.run() })
}

/// Creates a stack appropriate to trampoline start a thread
/// # Safety
/// `stack_top` must be a valid pointer to the top of a newly allocated stack
pub unsafe fn set_up_stack(stack_top: *mut u128) -> *mut u128 {
    // SAFETY: By assumption, `stack_top` is valid, and the following pointer
    // math is in accordance with the setup found in `context_switch.s`
    unsafe {
        let desired_top = stack_top.byte_sub(0x70);
        // The upper 64 bits of the entry store the LR to return to
        // The lower 64 bits of the entry store the FP,
        // zeroed here to indicate the end of the call chain
        desired_top.write(
            u128::from(architecture::usize_to_u64(
                #[allow(clippy::fn_to_numeric_cast_any)]
                #[allow(clippy::as_conversions)]
                (thread_trampoline as *const fn()).to_bits(),
            )) << 64_u8,
        );
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

/// Invokes the given callback, from its pointer parts
/// and passes the previously executing thread as a parameter
extern "C" fn invoke_callback<Callback>(
    data_address: *mut (),
    metadata: *const <Callback as Pointee>::Metadata,
    previous_thread: *mut Thread,
) where
    Callback: FnMut(Arc<Thread>),
{
    #[allow(clippy::unit_arg)]
    // SAFETY: the given parameters were correctly passed through a context switch
    // and reconstructed from the values in `context_switch`
    unsafe { ptr::from_raw_parts_mut::<Callback>(data_address, *metadata).as_mut() }
        .expect("Pointers passed through context switch should be valid")
        .call_once((
            // SAFETY: The previous thread was correctly read from `TPIDR_EL1`
            // and reconstructed from the values in `context_switch`
            unsafe { Arc::from_raw(previous_thread) },
        ));
}

/// Context switches into the given thread, and invokes the callback after switching threads
pub fn context_switch<Callback>(new_thread: Arc<Thread>, mut callback: Callback)
where
    Callback: FnMut(Arc<Thread>),
{
    me(|me| {
        me.runtime += kernel::time::now() - me.last_started;
        let (data, metadata): (*mut (), <Callback as Pointee>::Metadata) =
            ptr::addr_of_mut!(callback).to_raw_parts();
        // # SAFETY: The parameters are correctly set up and passed to context switching
        unsafe {
            _context_switch(
                data,
                &metadata,
                Arc::into_raw(new_thread).cast_mut(),
                invoke_callback::<Callback>,
            );
        }
        me.last_started = kernel::time::now();
    });
}

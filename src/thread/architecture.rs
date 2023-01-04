use crate::{
    architecture::{self, exception},
    thread::{self, Tcb},
};
use aarch64_cpu::registers::TPIDR_EL1;
use alloc::sync::Arc;
use core::{
    arch::global_asm,
    ptr::{self, addr_of, NonNull, Pointee},
};
use tock_registers::interfaces::{Readable, Writeable};

use super::Thread;

/// Returns a raw pointer to the currently running thread
/// # Safety
/// The thread must be kept alive while this pointer is active (e.g. the thread
/// should not be freed)
unsafe fn current_unsafe() -> NonNull<Tcb> {
    NonNull::new(<*mut Tcb>::from_bits(architecture::u64_to_usize(
        TPIDR_EL1.get(),
    )))
    .expect("Current thread should not be null")
}

/// Gets a handle to the thread that invokes it.
pub fn current() -> Thread {
    // SAFETY: `TPIDR_EL1` is set up during the initialization seqeunce
    // and during context switches so that it always contains a valid pointer
    // to the current thread, which is kept alive and then the strong counter
    // is incremented to persist the thread
    unsafe {
        let pointer = current_unsafe().as_ptr();
        Arc::increment_strong_count(pointer);
        Thread(Arc::from_raw(pointer))
    }
}

/// Sets the currently running thread
/// # Safety
/// Only the initialization sequence should call this, using the idle threads
pub unsafe fn set_me(Thread(thread): Thread) {
    TPIDR_EL1.set(architecture::usize_to_u64(Arc::into_raw(thread).to_bits()));
}

/// The first path of execution for a newly created thread
/// Jumps directly to the run method for the thread
#[no_mangle]
extern "C" fn thread_trampoline() -> ! {
    assert!(!exception::are_disabled());
    // SAFETY:
    // The current thread is guaranteed to be active for the duration of this
    // code block
    // The returned pointer is properly set by `context_switch`
    let current = unsafe { current_unsafe().as_ref() };
    current.local.last_started.set(architecture::time::now());
    // SAFETY:  This is the only valid time to `run` a newly created thread
    unsafe { current.run() }
}

/// Creates a stack appropriate to trampoline start a thread
/// # Safety
/// `stack_top` must be a valid pointer to the top of a newly allocated stack
pub unsafe fn set_up_stack(stack_top: NonNull<u128>) -> NonNull<u128> {
    // SAFETY: By assumption, `stack_top` is valid, and the following pointer
    // math is in accordance with the setup found in `context_switch.s`
    unsafe {
        let desired_top =
            NonNull::new(stack_top.as_ptr().byte_sub(0x70)).expect("Stack range should be valid");
        // The upper 64 bits of the entry store the LR to return to
        // The lower 64 bits of the entry store the FP,
        // zeroed here to indicate the end of the call chain
        desired_top.as_ptr().write(
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
        metadata: usize,
        new_thread: *mut Tcb,
        callback: extern "C" fn(data_address: *mut (), metadata: *const (), thread: *mut Tcb),
    );
}

/// Invokes the given callback, from its pointer parts
/// and passes the previously executing thread as a parameter
extern "C" fn invoke_callback<Callback>(
    data_address: *mut (),
    metadata: *const <Callback as Pointee>::Metadata,
    previous_thread: *mut Tcb,
) where
    Callback: FnMut(Arc<Tcb>),
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
pub(super) fn context_switch<Callback>(new_thread: Arc<Tcb>, mut callback: Callback)
where
    Callback: FnMut(Arc<Tcb>),
{
    {
        let Thread(current) = current();
        let last_started = current.local.last_started.get();
        let now = architecture::time::now();
        *current.runtime.write() += now - last_started;
    }

    // We should never attempt to context switch while interrupts are disabled
    // Doing so is likely an error
    assert!(!architecture::exception::are_disabled());
    let (data, metadata): (*mut (), <Callback as Pointee>::Metadata) =
        ptr::addr_of_mut!(callback).to_raw_parts();
    // SAFETY: The parameters are correctly set up and passed to context switching
    unsafe {
        #[allow(clippy::as_conversions)]
        _context_switch(
            data,
            addr_of!(metadata).to_bits(),
            Arc::into_raw(new_thread).cast_mut(),
            invoke_callback::<Callback>,
        );
    }
    {
        let Thread(current) = current();
        current.local.last_started.set(architecture::time::now());
    }
}

/// Preempts a thread, if preemption is not disabled
pub fn preempt() {
    let Thread(current) = current();
    if current.local.preemptible.get() {
        assert!(!current.is_idle());
        thread::yield_now();
    } else {
        current.local.pending_preemption.set(true);
    }
}

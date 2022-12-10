use crate::kernel::{self, thread::TCB};
use aarch64_cpu::registers::TPIDR_EL1;
use core::{
    arch::global_asm,
    ptr::{self, Pointee},
};
use tock_registers::interfaces::{Readable, Writeable};

pub fn me<'a>() -> &'a mut TCB {
    unsafe { &mut *<*mut TCB>::from_bits(TPIDR_EL1.get().try_into().unwrap()) }
}

pub unsafe fn set_me(thread: *mut TCB) -> () {
    TPIDR_EL1.set(thread.to_bits().try_into().unwrap())
}

#[no_mangle]
extern "C" fn thread_trampoline() -> ! {
    me().run();
}

/// **SAFETY**: `stack_top` must be a valid pointer to the top of a newly allocated stack
pub unsafe fn set_up_stack(stack_top: *mut u128) -> *mut u128 {
    unsafe {
        let desired_top = stack_top.byte_sub(0x70);
        // The upper 64 bits of the entry store the LR to return to
        // The lower 64 bits of the entry store the FP,
        // zeroed here to indicate the end of the call chain
        desired_top.write(
            u128::try_from((thread_trampoline as *const fn() -> ()).to_bits()).unwrap() << 64,
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
        new_thread: *mut TCB,
        callback: extern "C" fn(data_address: *mut (), metadata: *const (), thread: *mut TCB) -> (),
    ) -> ();
}

extern "C" fn invoke_callback<Callback>(
    data_address: *mut (),
    metadata: *const <Callback as Pointee>::Metadata,
    previous_thread: *mut TCB,
) -> ()
where
    Callback: FnMut(*mut TCB) -> (),
{
    unsafe { ptr::from_raw_parts_mut::<Callback>(data_address, *metadata).as_mut() }
        .unwrap()
        .call_once((previous_thread,))
}

pub fn context_switch<Callback>(new_thread: *mut TCB, mut data: Callback) -> ()
where
    Callback: FnMut(*mut TCB) -> (),
{
    let me = me();
    me.runtime += kernel::timer::now() - me.last_started;
    let (data, metadata): (*mut (), <Callback as Pointee>::Metadata) =
        ptr::addr_of_mut!(data).to_raw_parts();
    unsafe { _context_switch(data, &metadata, new_thread, invoke_callback::<Callback>) }
    me.last_started = kernel::timer::now()
}

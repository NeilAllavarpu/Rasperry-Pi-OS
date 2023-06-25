use core::{arch::asm, mem::MaybeUninit};

pub struct ExceptionVector {
    switch_in: unsafe extern "C" fn() -> !,
    switch_out: unsafe extern "C" fn(pc: usize) -> !,
    translation_fault: unsafe extern "C" fn() -> !,
}

#[no_mangle]
pub static EXCEPTION_VECTOR: ExceptionVector = ExceptionVector {
    switch_in: _handle_switch_in,
    switch_out: _handle_switch_out,
    translation_fault: _handle_translation_fault,
};

#[repr(C, align(16))]
struct SaveArea {
    sp: MaybeUninit<usize>,
}

/// Save area for registers during context switches
static mut SAVE_AREA: SaveArea = SaveArea {
    sp: MaybeUninit::uninit(),
};

#[naked]
unsafe extern "C" fn _handle_switch_in() -> ! {
    unsafe {
        asm!(
            "adr x0, {SAVE_AREA}",
            "ldr x0, [x0]",
            "mov sp, x0",
            "ldp x2, x3, [sp, #0x10]",
            "ldp x4, x5, [sp, #0x20]",
            "ldp x6, x7, [sp, #0x30]",
            "ldp x8, x9, [sp, #0x40]",
            "ldp x10, x11, [sp, #0x50]",
            "ldp x12, x13, [sp, #0x60]",
            "ldp x14, x15, [sp, #0x70]",
            "ldp x16, x17, [sp, #0x80]",
            "ldp x18, x19, [sp, #0x90]",
            "ldp x20, x21, [sp, #0xA0]",
            "ldp x22, x23, [sp, #0xB0]",
            "ldp x24, x25, [sp, #0xC0]",
            "ldp x26, x27, [sp, #0xD0]",
            "ldp x28, x29, [sp, #0xE0]",
            "ldr x30, [sp, #0xF0]",
            "ldp x0, x1, [sp], #0x100",
            "br x18", // Tell the kernel that we are done saving context
            SAVE_AREA = sym SAVE_AREA,
            options(noreturn)
        )
    }
}

#[naked]
unsafe extern "C" fn _handle_switch_out(pc: usize) -> ! {
    #[cfg(target_arch = "aarch64")]
    unsafe {
        asm!(
            "stp x0, x1, [sp, #-0x100]",
            "stp x2, x3, [sp, #0x10]",
            "stp x4, x5, [sp, #0x20]",
            "stp x6, x7, [sp, #0x30]",
            "stp x8, x9, [sp, #0x40]",
            "stp x10, x11, [sp, #0x50]",
            "stp x12, x13, [sp, #0x60]",
            "stp x14, x15, [sp, #0x70]",
            "stp x16, x17, [sp, #0x80]",
            "stp x18, x19, [sp, #0x90]",
            "stp x20, x21, [sp, #0xA0]",
            "stp x22, x23, [sp, #0xB0]",
            "stp x24, x25, [sp, #0xC0]",
            "stp x26, x27, [sp, #0xD0]",
            "stp x28, x29, [sp, #0xE0]",
            "str x30, [sp, #0xF0]",
            "adr x0, {SAVE_AREA}",
            "mov x1, sp",
            "str x1, [x0]",
            "svc #0", // Tell the kernel that we are done saving context
            SAVE_AREA = sym SAVE_AREA,
            options(noreturn)
        )
    }
}

#[naked]
unsafe extern "C" fn _handle_translation_fault() -> ! {
    unsafe {
        asm!(
            "stp x0, x1, [sp, #-0x90]!",
            "stp x2, x3, [sp, #0x10]",
            "stp x4, x5, [sp, #0x20]",
            "stp x6, x7, [sp, #0x30]",
            "stp x8, x9, [sp, #0x40]",
            "stp x10, x11, [sp, #0x50]",
            "stp x12, x13, [sp, #0x60]",
            "stp x14, x15, [sp, #0x70]",
            "stp x16, x17, [sp, #0x80]",
            "str x18, [sp, #0x90]",
            "br x0", // BRANCH TO HANDLER
            options(noreturn)
        )
    }
}

// The boot sequence

use core::arch::asm;

pub fn core_id() -> u8 {
    let mut id: u8;
    unsafe {
        asm!(
            "mrs {0:x}, mpidr_el1",
            "and {0:w}, {0:w}, #0b11",
            lateout(reg) id,
            options(nomem, nostack, preserves_flags),
        );
    }
    id
}

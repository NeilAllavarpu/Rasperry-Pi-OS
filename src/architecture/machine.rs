// The boot sequence

use aarch64_cpu::registers::{MPIDR_EL1, TPIDR_EL1};
use tock_registers::interfaces::{Readable, Writeable};

pub fn core_id() -> u8 {
    (MPIDR_EL1.get() & 0b11) as u8
}

pub fn thread_id() -> u64 {
    TPIDR_EL1.get()
}

pub fn set_thread_id(id: u64) -> () {
    TPIDR_EL1.set(id)
}

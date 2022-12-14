use aarch64_cpu::registers::MPIDR_EL1;
use tock_registers::interfaces::Readable;

/// Returns the core number of the currently executing core
pub fn core_id() -> u8 {
    // SAFETY: After the mask, this should always be a valid `u8`
    unsafe { (MPIDR_EL1.get() & 0b11).try_into().unwrap_unchecked() }
}

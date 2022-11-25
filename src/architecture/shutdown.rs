pub fn shutdown(exit_code: u32) -> ! {
    use crate::{architecture, log};
    use qemu_exit::QEMUExit;
    log!("Core {}: shutdown ({})", architecture::machine::core_id(), exit_code);
    qemu_exit::AArch64::new().exit(exit_code);
}

pub fn shutdown(exit_code: u32) -> ! {
    use qemu_exit::QEMUExit;
    crate::log!("Core {}: shutdown ({})", super::core_id(), exit_code);
    qemu_exit::AArch64::new().exit(exit_code);
}

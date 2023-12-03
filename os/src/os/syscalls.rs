pub fn alloc_page(_page_size: u64) -> Result<u64, ()> {
    Ok(0x80_0000)
}

pub fn write(bytes: &[u8]) {
    unsafe {
        core::arch::asm! {
            "svc #0x1000",
            in ("x0") bytes.as_ptr(),
            in ("x1") bytes.len(),
            options(nostack, readonly, preserves_flags)
        }
    }
}

pub fn exit() -> ! {
    unsafe {
        core::arch::asm! {
            "0: svc #0x2000",
            "b 0b",
            options(noreturn, nostack, readonly),
        }
    }
}

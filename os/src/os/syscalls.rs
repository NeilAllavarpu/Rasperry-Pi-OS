/// Executes a system call and coerces the return value into a `Result`
macro_rules! svc {
    ($code:expr, $success: ty, $fail: ty) => {{
        let success: usize;
        let value: usize;
        unsafe {
            core::arch::asm! {
                concat!("svc ", $code),
                options(nostack),
                out("x0") success,
                out("x1") value,
                clobber_abi("C"),
            }
        }
        match success {
            0 => Err(value),
            1 => Ok(value),
            _ => panic!(
                "Syscall {} returned an invalid success/failure value",
                $code
            ),
        }
    }};
}

pub fn alloc_page(_page_size: u64) -> Result<u64, ()> {
    Ok(0x80_0000)
}

#[inline]
pub fn write(bytes: &[u8]) -> bool {
    let status: usize;
    unsafe {
        core::arch::asm! {
            "svc 0x1000",
            inout("x0") bytes.as_ptr() => status,
            in("x1") bytes.len(),
            options(nostack, readonly),
            clobber_abi("C"),
        }
    }
    match status {
        0 => true,
        1 => false,
        _ => unreachable!("Write syscall returned an invalid success/failure value"),
    }
}

#[inline]
pub fn exit() -> ! {
    unsafe {
        core::arch::asm! {
            "svc 0x1000",
            options(nostack, readonly),
            clobber_abi("C"),
        }
    }
    // Should never reach here
    loop {
        core::hint::spin_loop()
    }
}

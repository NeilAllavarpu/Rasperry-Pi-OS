use crate::println;

#[inline]
#[must_use]
pub fn alloc_page() -> Option<u64> {
    let page: u64;
    let status: u64;
    unsafe {
        core::arch::asm! {
            "svc 0x3000",
            out("x0") status,
            out("x1") page,
            options(nomem, nostack),
            clobber_abi("C"),
        }
    };
    match status {
        0 => Some(page),
        1 => None,
        _ => unreachable!("Allocate page syscall returned an invalid success/failure value"),
    }
}

#[inline]
pub fn write(bytes: &[u8]) -> bool {
    let status: u64;
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
    loop {
        unsafe {
            core::arch::asm! {
                "svc 0x2000",
                options(nostack, readonly),
                clobber_abi("C"),
            }
        }
    }
}

#[inline]
pub fn exec(context: *mut (), ttbr0: u64, tcr_el1: u64, sp: usize) -> Result<!, ()> {
    let status: u64;
    println!("LEt the sp be {sp:X}");
    unsafe {
        core::arch::asm! {
            "mov sp, x20",
            // "isb",
            "svc 0x4000",
            "mov x20, sp",
            inlateout("x0") context => status,
            in("x1") ttbr0,
            in("x2") tcr_el1,
            in("x20") sp,
            options(nostack, readonly),
            clobber_abi("C"),
        }
    };
    match status {
        0 => unreachable!("Exec should never return if successful"),
        1 => Err(()),
        _ => unreachable!("Exec syscall returned an invalid success/failure value"),
    }
}

#[inline]
pub fn eret() -> ! {
    unsafe {
        core::arch::asm! {
            "svc 0x0",
            options(nostack, readonly),
            clobber_abi("C"),
        }
    }
    unreachable!("`eret` should not fall through")
}

#[inline]
pub fn unblock(pid: u16) -> Result<(), ()> {
    let status: u64;
    unsafe {
        core::arch::asm! {
            "svc 0x5000",
            in("x0") pid,
            lateout("x0") status,
            options(nostack, readonly),
            clobber_abi("C"),
        }
    };
    match status {
        0 => Ok(()),
        1 => Err(()),
        _ => unreachable!("Unblock syscall returned an invalid success/failure value"),
    }
}

use core::sync::atomic::Ordering;

use common::println;

use crate::exception::CONTEXT;

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
    unsafe {
        core::arch::asm! {
            "svc 0x4000",
            inlateout("x0") context => status,
            in("x1") ttbr0,
            in("x2") tcr_el1,
            in("x3") sp,
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
pub fn getpid() -> u16 {
    let pid: u64;
    unsafe {
        core::arch::asm! {
            "mrs {}, TPIDRRO_EL0",
            out(reg) pid,
            options(nomem, nostack, preserves_flags)
        }
    }
    pid.try_into().unwrap()
}

#[inline]
pub fn fork() -> Option<u16> {
    let pid = getpid();
    let status: u64;
    let new_pid: u64;
    let ra_location = CONTEXT.exception_stack.fetch_ptr_add(1, Ordering::Relaxed);
    unsafe {
        core::arch::asm! {
            "stp x19, x29, [sp, -16]!",
            "sub x1, sp, 0x100",
            "adr x2, {saved_sp}",
            "str x1, [x2]",
            "adr x2, 0f",
            "str x2, [x0]",
            "svc 0x6000",
            "0: ldp x19, x29, [sp], 16",
            saved_sp = sym crate::exception::SP,
            inlateout("x0") ra_location => status,
            lateout("x1") new_pid,
            lateout("x2") _,
            lateout("x3") _,
            lateout("x4") _,
            lateout("x5") _,
            lateout("x6") _,
            lateout("x7") _,
            lateout("x8") _,
            lateout("x9") _,
            lateout("x10") _,
            lateout("x11") _,
            lateout("x12") _,
            lateout("x13") _,
            lateout("x14") _,
            lateout("x15") _,
            lateout("x16") _,
            lateout("x17") _,
            lateout("x18") _,
            lateout("x20") _,
            lateout("x21") _,
            lateout("x22") _,
            lateout("x23") _,
            lateout("x24") _,
            lateout("x25") _,
            lateout("x26") _,
            lateout("x27") _,
            lateout("x28") _,
            lateout("x30") _,
            options(readonly),
            clobber_abi("C"),
        }
    };
    let mypid = getpid();
    println!("my pid {mypid} returned pid {new_pid} old pid {pid}");
    if pid == mypid {
        Some(new_pid.try_into().unwrap())
    } else {
        None
    }
}

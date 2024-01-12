use crate::runtime::exception;
use crate::runtime::exception::{UserContext, CONTEXT};
use core::sync::atomic::Ordering;

/// Allocates a physical page from the kernel.
/// Returns `Some(page)` if successful.
/// Returns `None` if there is no memory available.
#[inline]
#[must_use]
pub fn alloc_page() -> Option<u64> {
    let page: u64;
    let status: u64;
    // SAFETY: This correctly invokes and specifies the outputs for a page allocation syscall
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

#[must_use]
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

/// Exits the current program, cleaning up all its resources
#[inline]
pub fn exit() -> ! {
    loop {
        // SAFETY: This correctly invokes an `exit` syscall
        unsafe {
            core::arch::asm! {
                "svc 0x2000",
                options(nostack, readonly),
                clobber_abi("C"),
            }
        }
    }
}

#[expect(clippy::exhaustive_enums)]
pub enum ExecErrorKind {
    TTBR0 = 0b01,
    TCR = 0b10,
    Context = 0b11,
}

/// Error arising from an `exec` call
#[expect(clippy::exhaustive_structs)]
pub struct ExecError {
    /// Cause of the error
    pub kind: ExecErrorKind,
    /// Whether the error was due to an alignment error
    pub alignment_caused: bool,
}

/// Switches the address space and context of the current program to that of the provided arguments.
///
/// # Safety
/// * `context` must be correctly configured to operate in the *new* address space; the resumption handler will be immediately invoked after this
/// * `ttbr0` and `tcr_el1` must be correctly configured to operate the new address space
/// * `sp` must be valid for the newly running program to use as it expects
///
/// # Errors
/// Note that errors are not necessarily caught and returned to this program.
/// Safety can still be violated, as documented above, in these cases if not caught.
/// * If any of the values are misaligned, an error is returned indicating an alignment error for that value
/// * If the `context` is not properly accessible from usermode, a `Context` error is returned
/// * If the physical page for `ttbr0` is not owned by the program, a `TTBR0` error is returned
/// * If `tcr` sets invalid/privileged bits, a `TCR` error is returned.
#[inline]
pub unsafe fn exec(
    context: *mut UserContext,
    ttbr0: u64,
    tcr_el1: u64,
    sp: usize,
) -> Result<!, ExecError> {
    if !context.is_aligned() {
        return Err(ExecError {
            kind: ExecErrorKind::Context,
            alignment_caused: true,
        });
    }
    let status: u64;
    // SAFETY: The caller promises safety of the arguments, and this ASM block properly invokes `exec`
    unsafe {
        core::arch::asm! {
            "mov x3, sp",
            "adr x4, {sp_saved}",
            "str x3, [x4]",
            "mov sp, {sp}",
            "svc 0x4000", // <-- TODO: If preemption occurs here, SP is corrupt!
            "ldr x3, {sp_saved}",
            "mov sp, x3",
            sp = in(reg) sp,
            sp_saved = sym exception::SP,
            inlateout("x0") context => status,
            in("x1") ttbr0,
            in("x2") tcr_el1,
            out("x3") _,
            out("x4") _,
            options(nostack, readonly),
            clobber_abi("C"),
        }
    };

    Err(ExecError {
        kind: match status >> 1_u8 {
            0b00 => unreachable!("Exec should never return if successful"),
            0b01 => ExecErrorKind::TTBR0,
            0b10 => ExecErrorKind::TCR,
            0b11 => ExecErrorKind::Context,
            _ => unreachable!("Exec syscall returned an invalid success/failure value: {status}"),
        },
        alignment_caused: status & 0b1 == 1,
    })
}

/// Unblocks the targeted process by supplying its blocking token.
/// Returns true on success.
/// Returns false if the specified process does not exist
#[inline]
#[must_use]
pub fn unblock(pid: u16) -> bool {
    let status: u64;
    // SAFETY: This correctly specifies an `unblock` syscall
    unsafe {
        core::arch::asm! {
            "svc 0x5000",
            in("x0") pid,
            lateout("x0") status,
            options(nomem, nostack),
            clobber_abi("C"),
        }
    };
    match status {
        0 => true,
        1 => false,
        status => {
            unreachable!("Unblock syscall returned an invalid success/failure value: {status}")
        }
    }
}

#[inline]
pub fn block() {
    let ra_location = CONTEXT.exception_stack.fetch_ptr_add(1, Ordering::Relaxed);
    // SAFETY: This correctly marks all registers as clobbered and preserves the stack pointer
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
            inlateout("x0") ra_location => _,
            lateout("x1") _,
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
            saved_sp = sym exception::SP,
            clobber_abi("C"),
        }
    };
}

#[inline]
#[must_use]
pub fn send_signal(target_pid: u16) -> bool {
    let status: u64;
    unsafe {
        core::arch::asm! {
            "svc 0x7000",
            in("x0") target_pid,
            lateout("x0") status,
            options(nomem, nostack),
            clobber_abi("C"),
        }
    };
    match status {
        0 => true,
        1 => false,
        status => {
            unreachable!("Unblock syscall returned an invalid success/failure value: {status}")
        }
    }
}

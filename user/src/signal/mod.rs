pub enum SignalKind {}

pub struct SigInfo {}

#[allow(clippy::missing_docs_in_private_items)]
#[allow(clippy::struct_field_names)]
/// C interface, POSIX-specified functions
pub mod ffi {
    use core::ffi::{c_int, c_long, c_void};

    use bitfield_struct::bitfield;

    use crate::sys::types::ffi::{pid_t, uid_t};

    #[repr(C)]
    pub union SigVal {
        pub sival_int: c_int,
        pub sival_ptr: *mut c_void,
    }

    #[repr(C)]
    pub struct SigInfo {
        pub si_addr: *mut c_void,
        pub si_band: c_long,
        pub si_value: SigVal,
        pub si_signo: c_int,
        pub si_code: c_int,
        pub si_errno: c_int,
        pub si_status: c_int,
        pub si_pid: pid_t,
        pub si_uid: uid_t,
    }

    #[repr(C)]
    struct SigAction {
        sa_handler: unsafe extern "C" fn(c_int),
        sa_sigaction: unsafe extern "C" fn(c_int, *mut SigInfo, *mut c_void),
    }

    #[bitfield(u32)]
    struct SigFlags {
        nocldstop: bool,
        onstack: bool,
        resethand: bool,
        restart: bool,
        siginfo: bool,
        nocldwait: bool,
        nodefer: bool,
        #[bits(25)]
        __: u32,
    }

    #[no_mangle]
    unsafe extern "C" fn sigaction(
        sig: c_int,
        act: Option<&SigAction>,
        oact: Option<&mut SigAction>,
    ) -> c_int {
        // if act.is_some() {}
        0
    }
}

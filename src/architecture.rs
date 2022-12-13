// Architecture-specific (ARM) code
mod boot;
mod config;
pub mod exception;
mod exception_handlers;
pub mod machine;
mod shutdown;
mod spinlock;
pub mod thread;
pub mod timer;

pub use config::CONFIG;
pub use shutdown::shutdown;
pub use spinlock::SpinLock;

pub fn init() {
    crate::call_once!();
    exception::init();
    config::init();
}

pub fn per_core_init() {
    crate::call_once_per_core!();
    exception::per_core_init();
}

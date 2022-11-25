// Architecture-specific (ARM) code
mod boot;
mod config;
pub mod exception;
pub mod machine;
mod shutdown;
mod spinlock;
pub mod timer;

pub use config::CONFIG;
pub use shutdown::shutdown;
pub use spinlock::Spinlock;

pub fn init() {
    crate::call_once!();
    exception::init();
    config::init();
}

pub fn per_core_init() {
    crate::call_once_per_core!();
    exception::per_core_init();
}

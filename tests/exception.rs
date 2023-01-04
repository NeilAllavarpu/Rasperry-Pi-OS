#![feature(custom_test_frameworks)]
#![no_main]
#![no_std]
#![reexport_test_harness_main = "test_main"]
#![test_runner(libkernel::test_runner)]
#![feature(default_alloc_error_handler)]

extern crate alloc;
use libkernel::{
    add_test,
    architecture::exception::{self, Guard},
};

#[no_mangle]
fn kernel_main() {
    test_main()
}

add_test!(guard_preserves_interrupt_state, {
    assert!(
        !exception::are_disabled(),
        "Interrupts should be enabled when a thread runs, by default"
    );
    let guard = Guard::new();
    assert!(
        exception::are_disabled(),
        "Interrupts should be disabled while a guard is active"
    );
    drop(guard);
    assert!(
        !exception::are_disabled(),
        "Dropping all guards should re-enable interrupts"
    );
    let guard1 = Guard::new();
    assert!(
        exception::are_disabled(),
        "Interrupts should be disabled while a guard is active"
    );
    let guard2 = Guard::new();
    assert!(
        exception::are_disabled(),
        "Interrupts should be disabled while a guard is active"
    );
    drop(guard2);
    assert!(
        exception::are_disabled(),
        "Interrupts should remain disabled while a guard is active, even if another guard is dropped"
    );
    drop(guard1);
    assert!(
        !exception::are_disabled(),
        "Dropping all guards should re-enable interrupts"
    );
});

use crate::{architecture, cell::InitCell, derive_ord, kernel::PerCore, thread};
use aarch64_cpu::registers::{CNTP_CTL_EL0, CNTP_CVAL_EL0, ELR_EL1, SPSR_EL1};
use alloc::collections::BinaryHeap;
use core::{cmp::Reverse, time::Duration};
use tock_registers::interfaces::{ReadWriteable, Readable, Writeable};

/// Wrapper class for raw ticks
mod tick;
use tick::Tick;
/// Timer IRQ disabling guard. Enables safe mutual exclusion for a `PerCore`
/// object here, when the `PerCore` is accessed inside the IRQ handler
mod timer_irq_lock;
pub use timer_irq_lock::TimerIrqGuard;

/// Returns the current time
pub fn now() -> Duration {
    Tick::current_tick().into()
}

/// Timer scheduling ///

/// The global queue of all scheduled events
static SCHEDULED_EVENTS: InitCell<PerCore<BinaryHeap<Reverse<Event>>>> = InitCell::new();
/// Initializes timer events/callbacks
pub fn init() {
    tick::init();
    // SAFETY: This is the init seqeunce, and so is safe
    unsafe {
        SCHEDULED_EVENTS.set(PerCore::new(BinaryHeap::new));
        PREEMPTION_PERIOD.set(
            Duration::MILLISECOND
                .try_into()
                .expect("Preemption period should not overflow"),
        );
    };
}

/// Period between consecutive preemption events
static PREEMPTION_PERIOD: InitCell<Tick> = InitCell::new();

/// Enables preemption
pub fn per_core_init() {
    SCHEDULED_EVENTS.current().push(Reverse(Event {
        when: *PREEMPTION_PERIOD,
        operation: Operation::Preemption,
    }));
}

/// An operation to be run when an event is scheduled
enum Operation {
    /// A callback that indicates preemption
    Preemption,
}

/// A key to identify an event
struct Event {
    /// When this event is scheduled to fire
    when: Tick,
    /// The operation to run when scheduled
    operation: Operation,
}

derive_ord!(Event);

impl Ord for Event {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.when.cmp(&other.when)
    }
}

/// Sets the timer to trigger an interrupt at time `when`
fn enable_next_timer_irq(when: Tick) {
    CNTP_CVAL_EL0.set(when.into());
    CNTP_CTL_EL0.modify(CNTP_CTL_EL0::ENABLE::SET);
}

/// Handles a timer IRQ
pub fn handle_irq() {
    // Preserve ELR_EL1 and SPSR_EL1, in case an interrupt occurs in the following code
    let elr = ELR_EL1.extract();
    let spsr = SPSR_EL1.extract();

    // Mark the IRQ as handled and turn on interrupts, as the following code does not need to be protected
    CNTP_CTL_EL0.modify(CNTP_CTL_EL0::ENABLE::CLEAR);
    // SAFETY: Because this is an IRQ handler, we know that all we need to do is
    // to restore interrupts to disabled at the end
    unsafe {
        architecture::exception::enable();
    }

    let mut should_preempt = false;

    {
        /// Error message if the pending events queue is erroneously empty
        const EMPTY_EVENTS_MESSAGE: &str =
            "There should always be at least one scheduled event (preemption)";
        let _irq_guard = TimerIrqGuard::new();
        let mut events = SCHEDULED_EVENTS.current();

        while let Reverse(event_) = events.peek().expect(EMPTY_EVENTS_MESSAGE) && event_.when < Tick::current_tick_unsync() {
                let Reverse(event) = events.pop().expect(EMPTY_EVENTS_MESSAGE);
                match event.operation {
                    Operation::Preemption => {
                        // Schedule next preemption event
                        events.push(Reverse(Event {
                            when: event.when + *PREEMPTION_PERIOD,
                            operation: Operation::Preemption,
                        }));
                        should_preempt = true;
                    }
                }
            }

        enable_next_timer_irq(
            events
                .peek()
                .map(|Reverse(event)| event.when)
                .expect(EMPTY_EVENTS_MESSAGE),
        );
    }

    if should_preempt {
        thread::preempt();
    }

    // SAFETY: `eret` will re-enable exceptions. We need to disable them briefly
    // so that `ELR_EL1` and `SPSR_EL1` are not overwritten between now and the
    // final `eret`
    unsafe { architecture::exception::disable() }
    // Restore the exception registers
    ELR_EL1.set(elr.get());
    SPSR_EL1.set(spsr.get());
}

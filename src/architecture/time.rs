use crate::{
    architecture::{self},
    cell::InitCell,
    derive_ord,
    kernel::PerCore,
};
use aarch64_cpu::registers::{CNTP_CTL_EL0, CNTP_CVAL_EL0, ELR_EL1, SPSR_EL1};
use alloc::{
    boxed::Box,
    collections::BinaryHeap,
    sync::{Arc, Weak},
};
use core::{
    cmp::Reverse,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
use smallvec::SmallVec;
use tock_registers::interfaces::{ReadWriteable, Readable, Writeable};

/// Wrapper class for raw ticks
mod tick;
use tick::Tick;
/// Timer IRQ disabling guard. Enables safe mutual exclusion for a `PerCore`
/// object here, when the `PerCore` is accessed inside the IRQ handler
mod timer_irq_lock;
use timer_irq_lock::TimerIrqGuard;

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
            Duration::SECOND
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
    /// A callback to run once, at a certain point
    Callback(Box<dyn FnOnce()>, Arc<AtomicBool>),
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

impl Event {
    /// Creates a new `EventKey` with a unique ID, along with a handle
    fn new(when: Tick, callback: Box<dyn FnOnce()>) -> (Self, EventHandle) {
        let active = Arc::new(AtomicBool::new(true));
        let handle = EventHandle(Arc::downgrade(&active));
        let event = Self {
            when,
            operation: Operation::Callback(callback, active),
        };
        (event, handle)
    }
}

derive_ord!(Event);

impl Ord for Event {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.when.cmp(&other.when)
    }
}

/// A handle for a scheduled callback. Can be used to cancel the event
pub struct EventHandle(Weak<AtomicBool>);

impl EventHandle {
    /// Aborts the scheduled callback
    #[allow(dead_code)]
    pub fn abort(self) {
        if let Some(active) = Weak::upgrade(&self.0) {
            active.store(false, Ordering::Relaxed);
        }
    }
}

/// Sets the timer to trigger an interrupt at time `when`
fn enable_next_timer_irq(when: Tick) {
    CNTP_CVAL_EL0.set(when.into());
    CNTP_CTL_EL0.modify(CNTP_CTL_EL0::ENABLE::SET);
}

/// Schedules a one-time callback to be run after the given delay has passed
#[allow(dead_code)]
pub fn schedule_callback(
    delay: Duration,
    callback: Box<dyn FnOnce()>,
) -> Result<EventHandle, <Tick as TryFrom<Duration>>::Error> {
    let when = (now() + delay).try_into()?;
    let (event, handle) = Event::new(when, callback);
    let _irq_guard = TimerIrqGuard::new();
    let mut events = SCHEDULED_EVENTS.current();
    events.push(Reverse(event));
    if let Some(Reverse(min_event)) = events.peek() {
        enable_next_timer_irq(min_event.when);
    }
    Ok(handle)
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

    {
        let mut callbacks: SmallVec<[Box<dyn FnOnce()>; 4]> = SmallVec::new_const();
        let mut should_preempt: bool = false;
        // Copy any pending events to a local state, so that we don't have to
        // hold the scheduler lock while running events
        {
            let _irq_guard = TimerIrqGuard::new();
            let mut events = SCHEDULED_EVENTS.current();

            while let Some(Reverse(event_)) = events.peek() && event_.when < Tick::current_tick_unsync() {
                let Reverse(event) = events.pop().expect("Should not fail");
                match event.operation {
                    Operation::Callback(callback,  active) => {
                        // Only handle the event if still active; otherwise, drop the event
                        if active.load(Ordering::Relaxed) {
                            // Time condition has been met, prepare run the event
                            callbacks.push(callback);
                        }
                    }
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
            if let Some(next_event_tick) = events.peek().map(|Reverse(event)| event.when) {
                enable_next_timer_irq(next_event_tick);
            }
        }

        for callback in callbacks {
            callback.call_once(());
        }

        if should_preempt {
            architecture::thread::preempt();
        }
    }

    // SAFETY: `eret` will re-enable exceptions. We need to disable them briefly
    // so that `ELR_EL1` and `SPSR_EL1` are not overwritten between now and the
    // final `eret`
    unsafe { architecture::exception::disable() }
    // Restore the exception registers
    ELR_EL1.set(elr.get());
    SPSR_EL1.set(spsr.get());
}

use crate::{
    architecture::{self, SpinLock},
    cell::InitCell,
    derive_ord,
    kernel::{Mutex, PerCore},
};
use aarch64_cpu::registers::{CNTP_CTL_EL0, CNTP_CVAL_EL0, ELR_EL1, SPSR_EL1};
use alloc::{
    boxed::Box,
    collections::BinaryHeap,
    sync::{Arc, Weak},
};
use core::{
    cmp::Reverse,
    mem::MaybeUninit,
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};
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

/// Initializes timer events/callbacks
pub fn init() {
    tick::init();
    // SAFETY: This is the init seqeunce, and so is safe
    unsafe {
        SCHEDULED_EVENTS.set(PerCore::new(BinaryHeap::new));
    };
}

/// Timer scheduling ///
/// The global queue of all scheduled events
static SCHEDULED_EVENTS: InitCell<PerCore<BinaryHeap<Reverse<Event>>>> = InitCell::new();

/// An operation to be run when an event is scheduled
enum Operation {
    /// A callback to run once, at a certain point
    Callback(Box<dyn FnOnce()>),
    /// A callback that may reoccur indefinitely
    PeriodicCallback(Arc<SpinLock<dyn FnMut()>>, Tick),
}

/// A key to identify an event
struct Event {
    /// When this event is scheduled to fire
    when: Tick,
    /// Whether or not this event is still active
    active: Arc<AtomicBool>,
    /// The operation to run when scheduled
    operation: Operation,
}

impl Event {
    /// Creates a new `EventKey` with a unique ID
    fn new(when: Tick, operation: Operation) -> Self {
        Self {
            when,
            active: Arc::new(AtomicBool::new(true)),
            operation,
        }
    }

    /// Creates a handle for this event
    fn create_handle(&self) -> EventHandle {
        EventHandle(Arc::downgrade(&self.active))
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
    let tick = (now() + delay).try_into()?;
    Ok(add_event(tick, Operation::Callback(callback)))
}

/// Schedules a periodic time callback to be run once per period
#[allow(dead_code)]
pub fn schedule_periodic_callback(
    period: Duration,
    callback: Arc<SpinLock<dyn FnMut()>>,
) -> Result<EventHandle, <Tick as TryFrom<Duration>>::Error> {
    let tick = period.try_into()?;
    Ok(add_event(
        tick,
        Operation::PeriodicCallback(
            callback,
            period.try_into().expect("Period should not overflow"),
        ),
    ))
}

/// Adds an event to the scheduling list
fn add_event(when: Tick, operation: Operation) -> EventHandle {
    let event = Event::new(when, operation);
    let handle = event.create_handle();
    {
        CNTP_CTL_EL0.modify(CNTP_CTL_EL0::IMASK::SET);

        let _irq_guard = TimerIrqGuard::new();
        let mut events = SCHEDULED_EVENTS.current();
        events.push(Reverse(event));
        if let Some(Reverse(min_event)) = events.peek() {
            enable_next_timer_irq(min_event.when);
        }
        CNTP_CTL_EL0.modify(CNTP_CTL_EL0::IMASK::CLEAR);
    }
    handle
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
        // Only one core shall handle general timer interrutps
        // if architecture::machine::core_id() == 0 {
        /// TODO: Ideally, we would be able to `drain_filter` all matching events
        /// into a (variably sized) stack-allocated slice/array. However for some
        /// reason I can't seem to get this to work, so here's a temporary hack to
        /// make it work. Will panic if there are too many scheduled events to run
        const MAX_ELEMS: usize = 64;

        let mut to_run: [MaybeUninit<Event>; MAX_ELEMS] = MaybeUninit::uninit_array();
        let mut num_elems = 0;
        // Copy any pending events to a local state, so that we don't have to
        // hold the scheduler lock while running events
        {
            let _irq_guard = TimerIrqGuard::new();
            let mut events = SCHEDULED_EVENTS.current();

            while let Some(Reverse(event)) = events.pop() {
                // Only handle the event if still active; otherwise, drop the event
                if event.active.load(Ordering::Relaxed) {
                    if event.when < Tick::current_tick_unsync() {
                        // Time condition has been met, schedule the event
                        *to_run
                            .get_mut(num_elems)
                            .expect("Index should be in bounds") = MaybeUninit::new(event);
                        num_elems += 1;
                    } else {
                        // Time condition not met, re-add the element and break out
                        events.push(Reverse(event));
                        break;
                    }
                }
            }
            if let Some(next_event_tick) = events.peek().map(|Reverse(event)| event.when) {
                enable_next_timer_irq(next_event_tick);
            }
        }
        for elem in to_run {
            if num_elems == 0 {
                break;
            }
            num_elems -= 1;
            // SAFETY: We manually construct the array so that the first `num_elem`
            // elements are initialized
            let event = unsafe { elem.assume_init() };
            match event.operation {
                Operation::Callback(callback) => {
                    callback.call_once(());
                }
                Operation::PeriodicCallback(callback, period) => {
                    let next_event = Event::new(
                        event.when + period,
                        Operation::PeriodicCallback(Arc::clone(&callback), period),
                    );
                    {
                        let _irq_guard = TimerIrqGuard::new();
                        SCHEDULED_EVENTS.current().push(Reverse(next_event));
                    }
                    callback.lock().call_mut(());
                }
            }
        }
    }

    // SAFETY: `eret` will re-enable exceptions. We need to disable them briefly
    // so that `ELR_EL1` is not overwritten between now and the final `eret`
    unsafe { architecture::exception::disable() }
    // Restore the exception registers
    ELR_EL1.set(elr.get());
    SPSR_EL1.set(spsr.get());
}

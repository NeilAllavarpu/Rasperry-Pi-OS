use crate::{
    architecture::{self, SpinLock},
    cell::InitCell,
    derive_ord,
    kernel::Mutex,
};
use aarch64_cpu::registers::{CNTP_CTL_EL0, CNTP_CVAL_EL0, ELR_EL1, SPSR_EL1};
use alloc::{
    boxed::Box,
    collections::BTreeMap,
    sync::{Arc, Weak},
};
use core::{
    cmp::min,
    mem::MaybeUninit,
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};
use tock_registers::interfaces::{ReadWriteable, Readable, Writeable};

/// Wrapper class for raw ticks
mod tick;
use tick::Tick;

/// Returns the current time
pub fn now() -> Duration {
    Tick::current_tick().into()
}

/// Initializes timer events/callbacks
pub fn init() {
    tick::init();
    // SAFETY: This is the init seqeunce, and so is safe
    unsafe {
        SCHEDULED_EVENTS.set(SpinLock::new(BTreeMap::new()));
    };
}

/// Timer scheduling ///

/// Sets the timer to trigger an interrupt at time `when`
fn enable_next_timer_irq(when: Tick) {
    CNTP_CVAL_EL0.set(when.into());
    CNTP_CTL_EL0.modify(CNTP_CTL_EL0::ENABLE::SET);
}

/// The ID of the next created event
static NEXT_EVENT_ID: AtomicU64 = AtomicU64::default();

/// A key to identify an event
pub struct EventKey {
    /// The time at which to trigger this event
    time: AtomicU64,
    /// The ID of the event
    id: u64,
}

impl EventKey {
    /// Creates a new `EventKey` with a unique ID
    fn new(tick: Tick) -> Self {
        Self {
            time: AtomicU64::new(tick.into()),
            id: NEXT_EVENT_ID.fetch_add(1, Ordering::Relaxed),
        }
    }

    /// Advances the key's `time` by the given amount
    fn advance_by(&self, amount: Tick) {
        self.time.fetch_add(amount.into(), Ordering::Relaxed);
    }

    /// Returns the `Tick` at which this event will occur
    fn tick(&self) -> Tick {
        self.time.load(Ordering::Relaxed).into()
    }
}

derive_ord!(EventKey);

impl Ord for EventKey {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        let comparison = self.tick().cmp(&other.tick());
        if comparison.is_eq() {
            self.id.cmp(&other.id)
        } else {
            comparison
        }
    }
}

/// An operation to be run when an event is scheduled
enum Operation {
    /// A callback to run once, at a certain point
    Callback(Box<dyn FnOnce()>),
    /// A callback that may reoccur indefinitely
    PeriodicCallback(Arc<SpinLock<dyn FnMut()>>, Tick),
}

/// The global queue of all scheduled events
static SCHEDULED_EVENTS: InitCell<SpinLock<BTreeMap<Arc<EventKey>, Operation>>> = InitCell::new();

/// Schedules a one-time callback to be run after the given delay has passed
#[allow(dead_code)]
pub fn schedule_callback(delay: Duration, callback: Box<dyn FnOnce()>) -> Weak<EventKey> {
    add_event(
        EventKey::new(
            (now() + delay)
                .try_into()
                .expect("Delay should not overflow"),
        ),
        Operation::Callback(callback),
    )
}

/// Schedules a periodic time callback to be run once per period
#[allow(dead_code)]
pub fn schedule_periodic_callback(
    period: Duration,
    callback: Arc<SpinLock<dyn FnMut()>>,
) -> Option<Weak<EventKey>> {
    period.try_into().ok().map(|tick| {
        add_event(
            EventKey::new(tick),
            Operation::PeriodicCallback(
                callback,
                period.try_into().expect("Period should not overflow"),
            ),
        )
    })
}

/// Adds an event to the scheduling list
fn add_event(key: EventKey, operation: Operation) -> Weak<EventKey> {
    let strong_key = Arc::new(key);
    let weak_key = Arc::downgrade(&strong_key);
    let mut events = SCHEDULED_EVENTS.lock();
    let new_timer = if let Some((prev_min, _)) = events.first_key_value() {
        min(prev_min.tick(), strong_key.tick())
    } else {
        strong_key.tick()
    };
    events.insert(strong_key, operation);
    drop(events);
    enable_next_timer_irq(new_timer);
    weak_key
}

/// Aborts the callback corresponding to the given ID
#[allow(dead_code)]
pub fn abort_callback(key: Weak<EventKey>) {
    if let Some(strong_key) = Weak::upgrade(&key) {
        SCHEDULED_EVENTS.lock().remove(&strong_key);
    }
    // Explicitly drop the key, because it is no longer valid:
    // either the event was deregistered, or the event was no longer valid anyways
    drop(key);
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

    // Only one core shall handle general timer interrutps
    if architecture::machine::core_id() == 0 {
        /// TODO: Ideally, we would be able to `drain_filter` all matching events
        /// into a (variably sized) stack-allocated slice/array. However for some
        /// reason I can't seem to get this to work, so here's a temporary hack to
        /// make it work. Will panic if there are too many scheduled events to run
        const MAX_ELEMS: usize = 64;

        let mut to_run: [MaybeUninit<(Arc<EventKey>, Operation)>; MAX_ELEMS] =
            MaybeUninit::uninit_array();
        let mut num_elems = 0;
        // Copy any pending events to a local state, so that we don't have to
        // hold the scheduler lock while running events
        {
            let mut events = SCHEDULED_EVENTS.lock();

            for entry in events.drain_filter(|key, _| key.tick() <= Tick::current_tick_unsync()) {
                *to_run
                    .get_mut(num_elems)
                    .expect("Index should be in bounds") = MaybeUninit::new(entry);
                num_elems += 1;
            }
            if let Some(next_event_tick) = events.first_key_value().map(|(key, _)| key.tick()) {
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
            let (key, operation) = unsafe { elem.assume_init() };
            match operation {
                Operation::Callback(callback) => {
                    callback.call_once(());
                }
                Operation::PeriodicCallback(callback, period) => {
                    key.advance_by(period);
                    {
                        SCHEDULED_EVENTS.lock().insert(
                            key,
                            Operation::PeriodicCallback(Arc::clone(&callback), period),
                        );
                    }
                    callback.lock().call_mut(());
                }
            }
        }
    } else {
        // For all other cores, a timer interrupt indicates preemption
        architecture::thread::preempt();
    }

    // SAFETY: `eret` will re-enable exceptions. We need to disable them briefly
    // so that `ELR_EL1` is not overwritten between now and the final `eret`
    unsafe { architecture::exception::disable() }
    // Restore the exception registers
    ELR_EL1.set(elr.get());
    SPSR_EL1.set(spsr.get());
}

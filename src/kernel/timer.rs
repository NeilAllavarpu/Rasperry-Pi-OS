use crate::architecture;
/// Timer support
use core::{hint, num::NonZeroU32, ops::Add, time::Duration};

const NANOSEC_PER_SEC: NonZeroU32 = NonZeroU32::new(1000000000).unwrap();

/// Encloses a timer given tick value
#[derive(PartialEq, PartialOrd)]
pub struct TimerValue(u64);

impl TimerValue {
    const MAX: Self = TimerValue(u64::MAX);
}

impl Add for TimerValue {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        TimerValue(self.0 + other.0)
    }
}

impl From<TimerValue> for Duration {
    fn from(timer_value: TimerValue) -> Self {
        let nanoseconds: u128 = (timer_value.0 as u128) * (NANOSEC_PER_SEC.get() as u128)
            / (architecture::timer_frequency().get() as u128);

        Duration::new(
            (nanoseconds / (NANOSEC_PER_SEC.get() as u128))
                .try_into()
                .expect("Number of seconds should fit into a u64"),
            (nanoseconds % (NANOSEC_PER_SEC.get() as u128))
                .try_into()
                .expect("Number of nanoseconds should fit into a u64"),
        )
    }
}

impl TryFrom<Duration> for TimerValue {
    type Error = &'static str;

    fn try_from(duration: Duration) -> Result<Self, Self::Error> {
        if duration > Duration::from(TimerValue::MAX) {
            return Err("Duration is too large to represent with the given timer");
        }

        let counter_value: u128 = duration.as_nanos()
            * (architecture::timer_frequency().get() as u128)
            / (NANOSEC_PER_SEC.get() as u128);

        Ok(TimerValue(counter_value as u64))
    }
}

/// Returns the current timestamp
pub fn now() -> TimerValue {
    TimerValue(architecture::current_tick())
}

/// Pauses execution for at least the given duration, up to rounding errors
pub fn wait_at_least(duration: Duration) -> () {
    let counter_value_target: TimerValue = now() + duration.try_into().unwrap();

    // Spin until the desired time is reached
    while now() < counter_value_target {
        hint::spin_loop();
    }
}

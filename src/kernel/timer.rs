/// Timer support
use crate::architecture::timer;
use core::{hint, num::NonZeroU32, ops::Add, time::Duration};

const NANOSEC_PER_SEC: NonZeroU32 = NonZeroU32::new(1000000000).unwrap();

/// Encloses a timer given tick value
pub struct TimerValue(u64);

impl TimerValue {
    const MAX: Self = TimerValue(u64::MAX);

    pub const fn new(ticks: u64) -> Self {
        Self(ticks)
    }

    pub const fn ticks(&self) -> u64 {
        self.0
    }
}

impl Add for TimerValue {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self(self.ticks() + other.ticks())
    }
}

impl From<TimerValue> for Duration {
    fn from(timer_value: TimerValue) -> Self {
        let nanoseconds: u128 = (timer_value.ticks() as u128) * (NANOSEC_PER_SEC.get() as u128)
            / (timer::timer_frequency().get() as u128);

        Self::new(
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

        let counter_value: u128 = duration.as_nanos() * (timer::timer_frequency().get() as u128)
            / (NANOSEC_PER_SEC.get() as u128);

        Ok(Self(counter_value as u64))
    }
}

/// Returns the current timestamp
pub fn now() -> Duration {
    Duration::from(timer::current_tick())
}

/// Pauses execution for at least the given duration, up to rounding errors
pub fn wait_at_least(duration: Duration) -> () {
    let target_time: Duration = now() + duration;

    // Spin until the desired time is reached
    while now() < target_time {
        hint::spin_loop();
    }
}

/// Timer support
use crate::architecture;
use core::{
    hint,
    num::{NonZeroU128, NonZeroU32},
    ops::Add,
    time::Duration,
};

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
        let nanoseconds: u128 = u128::from(timer_value.ticks()) * u128::from(NANOSEC_PER_SEC.get())
            / NonZeroU128::from(architecture::timer::timer_frequency());

        Self::new(
            (nanoseconds / NonZeroU128::from(NANOSEC_PER_SEC))
                .try_into()
                .unwrap(),
            (nanoseconds % NonZeroU128::from(NANOSEC_PER_SEC))
                .try_into()
                .unwrap(),
        )
    }
}

impl TryFrom<Duration> for TimerValue {
    type Error = &'static str;

    fn try_from(duration: Duration) -> Result<Self, Self::Error> {
        if duration > Duration::from(TimerValue::MAX) {
            return Err("Duration is too large to represent with the given timer");
        }

        Ok(Self(
            (duration.as_nanos()
                * u128::from(NonZeroU128::from(architecture::timer::timer_frequency()))
                / NonZeroU128::from(NANOSEC_PER_SEC))
            .try_into()
            .unwrap(),
        ))
    }
}

/// Returns the current timestamp
pub fn now() -> Duration {
    Duration::from(architecture::timer::current_tick())
}

/// Pauses execution for at least the given duration, up to rounding errors
#[allow(dead_code)]
pub fn wait_at_least(duration: Duration) -> () {
    let target_time: Duration = now() + duration;

    // Spin until the desired time is reached
    while now() < target_time {
        hint::spin_loop();
    }
}

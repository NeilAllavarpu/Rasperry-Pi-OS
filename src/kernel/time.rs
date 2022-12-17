/// Timer support
use crate::architecture;
use core::{
    hint,
    num::{NonZeroU128, NonZeroU32},
    ops::Add,
    time::Duration,
};

/// The number of nanoseconds per second
#[allow(clippy::undocumented_unsafe_blocks)]
const NANOSEC_PER_SEC: NonZeroU32 = unsafe { NonZeroU32::new_unchecked(1_000_000_000) };

/// Encloses a clock tick value
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct Tick {
    /// The tick value
    pub tick: u64,
}

impl Tick {
    /// The maximum representable amount of ticks
    const MAX: Self = Tick::new(u64::MAX);

    /// Creates a new `Tick` enclosing the given tick
    pub const fn new(tick: u64) -> Self {
        Self { tick }
    }
}

impl Add for Tick {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self {
            tick: self.tick + other.tick,
        }
    }
}

impl From<Tick> for Duration {
    fn from(tick: Tick) -> Self {
        let nanoseconds: u128 = u128::from(tick.tick) * u128::from(NANOSEC_PER_SEC.get())
            / NonZeroU128::from(architecture::time::frequency());

        Self::new(
            (nanoseconds / NonZeroU128::from(NANOSEC_PER_SEC))
                .try_into()
                .expect("The number of seconds for a tick should not overflow"),
            (nanoseconds % NonZeroU128::from(NANOSEC_PER_SEC))
                .try_into()
                .expect("The number of nanoseconds for a tick should not overflow"),
        )
    }
}

impl TryFrom<Duration> for Tick {
    type Error = &'static str;

    fn try_from(duration: Duration) -> Result<Self, Self::Error> {
        if duration > Duration::from(Tick::MAX) {
            return Err("Duration is too large to represent with the given timer");
        }

        Ok(Self {
            tick: (duration.as_nanos()
                * u128::from(NonZeroU128::from(architecture::time::frequency()))
                / NonZeroU128::from(NANOSEC_PER_SEC))
            .try_into()
            .map_err(|_err| {
                "Computing the ticks from a small enough duration should not overflow"
            })?,
        })
    }
}

/// Returns the current timestamp
pub fn now() -> Duration {
    Duration::from(architecture::time::current_tick())
}

/// Pauses execution for at least the given duration, up to rounding errors
#[allow(dead_code)]
pub fn wait_at_least(duration: Duration) {
    let target_time: Duration = now() + duration;

    // Spin until the desired time is reached
    while now() < target_time {
        hint::spin_loop();
    }
}

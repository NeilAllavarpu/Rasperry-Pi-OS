use aarch64_cpu::{
    asm::barrier,
    registers::{CNTFRQ_EL0, CNTPCT_EL0},
};
use tock_registers::interfaces::Readable;

/// Timer support
use crate::kernel::SetOnce;
use core::{
    num::{NonZeroU128, NonZeroU32},
    time::Duration,
};

/// The number of nanoseconds per second
#[allow(clippy::undocumented_unsafe_blocks)]
const NANOSEC_PER_SEC: NonZeroU32 = unsafe { NonZeroU32::new_unchecked(1_000_000_000) };

/// Encloses a clock tick value
#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy)]
pub struct Tick {
    /// The tick value
    tick: u64,
}

/// The frequency of the system clock, in Hz
static FREQUENCY: SetOnce<NonZeroU32> = SetOnce::new();

impl Tick {
    /// Returns the current value of the system timer, but does not necessarily
    /// Does not execute an ISB, so the timer may be read ahead of time
    pub fn current_tick_unsync() -> Self {
        Self {
            tick: CNTPCT_EL0.get(),
        }
    }

    /// Returns the current value of the system timer
    pub fn current_tick() -> Tick {
        // Prevent that the counter is read ahead of time due to out-of-order execution.
        barrier::isb(barrier::SY);
        Self::current_tick_unsync()
    }
}

/// Initializes the frequency and associated constants for `Tick`s
pub fn init() {
    unsafe {
        FREQUENCY.set(
            u32::try_from(CNTFRQ_EL0.get())
                .expect("The clock frequency should fit into 32 bits")
                .try_into()
                .expect("The clock frequency should not be 0"),
        );
    }
}

impl From<Tick> for Duration {
    fn from(tick: Tick) -> Self {
        let nanoseconds: u128 = u128::from(tick.tick) * u128::from(NANOSEC_PER_SEC.get())
            / NonZeroU128::from(*FREQUENCY);

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
        u64::try_from(
            duration.as_nanos() * u128::from(NonZeroU128::from(*FREQUENCY))
                / NonZeroU128::from(NANOSEC_PER_SEC),
        )
        .map_or(
            Err("Duration is too large to represent with the given timer"),
            |tick| Ok(Tick { tick }),
        )
    }
}

impl const From<Tick> for u64 {
    fn from(tick: Tick) -> Self {
        tick.tick
    }
}

impl const From<u64> for Tick {
    fn from(tick: u64) -> Self {
        Self { tick }
    }
}

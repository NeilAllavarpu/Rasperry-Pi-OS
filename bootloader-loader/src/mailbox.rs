//! Mailbox driver, specifically for the property interface
//!
//! See <https://github.com/raspberrypi/firmware/wiki/Mailbox-property-interface> for more
//! information
use bitfield_struct::bitfield;
use core::arch::aarch64::OSHST;
use core::mem;
use core::mem::MaybeUninit;
use core::num::NonZeroU32;
use core::num::NonZeroUsize;
use core::ptr::{self, NonNull};
use tock_registers::interfaces::Readable;
use tock_registers::interfaces::Writeable;
use tock_registers::registers::ReadWrite;
use tock_registers::{register_bitfields, register_structs};

register_bitfields! {
    u32,
    DATA [
        DATA OFFSET(4) NUMBITS(28) [],
        CHANNEL OFFSET(0) NUMBITS(4) [
            Power = 0,
            FrameBuffer = 1,
            VirtualUart = 2,
            Vchiq = 3,
            Led = 4,
            Button = 5,
            Touchscreen = 6,
            PropertyTagsToVc = 8,
            PropertyTagsToArm = 9,
        ]
    ],
    STATUS [
        FULL OFFSET(31) NUMBITS(1) [
            NotFull = 0b0,
            Full = 0b1,
        ],
        EMPTY OFFSET(30) NUMBITS(1) [
            NotEmpty = 0b0,
            Empty = 0b1,
        ]
    ]
}

register_structs! {
    Registers {
        (0x00 => data: ReadWrite<u32, DATA::Register>),
        (0x04 => _unused),
        (0x10 => peek: ReadWrite<u32>),
        (0x14 => sender: ReadWrite<u32>),
        (0x18 => status: ReadWrite<u32, STATUS::Register>),
        (0x1C => config: ReadWrite<u32>),
        (0x20 => write: ReadWrite<u32, DATA::Register>),
        (0x24 => _uu),
        (0x38 => status1:ReadWrite<u32, STATUS::Register>),
        (0x3C => @END),
    }
}

/// The possible statuses of a buffer sent via the property mailbox
#[repr(u32)]
#[allow(dead_code)]
enum BufferStatus {
    /// The buffer is an inflight message
    Request = 0,
    /// The buffer contains a successful response
    Success = 0x8000_0000,
    /// The buffer contains an errored response (e.g. parsing)
    Error = 0x8000_0001,
}

/// Clocks that can be used with the mailbox
#[repr(u32)]
#[allow(dead_code)]
pub enum Clock {
    /// External mass-media controller (SD card)
    Emmc = 1,
    /// UART
    Uart = 2,
    /// The ARM cores
    Arm = 3,
    Core = 4,
    V3d = 5,
    H264 = 6,
    Isp = 7,
    Sdram = 8,
    Pixel = 9,
    Pwm = 10,
    Hevc = 11,
    Emmc2 = 13,
    PixelBvb = 14,
}

/// IDs for the various possible property tags
#[repr(u32)]
#[expect(
    clippy::enum_variant_names,
    reason = "Other mailbox tags are not yet implemented"
)]
enum Tag {
    /// Get the maximum clock rate of a peripheral
    GetMaxClockRate = 0x3_0004,
    /// Get the current clock rate of a peripheral
    GetClockRate = 0x3_0047,
    /// Sets the clock rate of a peripheral. May be clamped to supported ranges
    SetClockRate = 0x3_8002,
}

/// Status of a tag in a message
#[bitfield(u32)]
struct TagStatus {
    /// For responses: the desired length of the response
    #[bits(31)]
    length: u32,
    /// Whether or not this represents a request or a response
    is_response: bool,
}

/// Counts the number of arguments to this macro
// Source: https://danielkeep.github.io/tlborm/book/blk-counting.html
macro_rules! count_tts {
    () => {0};
    ($_head:tt $($tail:tt)*) => {1 + count_tts!($($tail)*)};
}

/// Defines a buffer to use with mailbox messages
macro_rules! buffer {
    ($name: ident, $tag: expr, $($field: ident: $type:ty,)+) => {
        // The buffer itself is 16-byte aligned as only the upper 28 bits of the address can be
        // passed via the mailbox.
        // All u64/u32/u16 values are in host CPU endian order.
        #[repr(C, align(16))]
        struct $name {
            /// Buffer size in bytes (including the header values, the end tag and padding)
            size: u32,
            /// Buffer request/response code
            status: BufferStatus,
            /// Tag ID
            tag: Tag,
            /// Value buffer size, in bytes
            value_size: u32,
            /// Status of this tag
            tag_status: TagStatus,
            $($field: $type,)+
            /// End tag
            end: u32
        }

        impl $name {
            const fn new($($field: $type,)+) -> Self {
                #[expect(
                    clippy::as_conversions,
                    reason = "No way to const-convert a `usize` to `u32` currently"
                )]
                Self {
                    size: mem::size_of::<Self>() as u32,
                    status: BufferStatus::Request,
                    tag: $tag,
                    tag_status: TagStatus::new().with_is_response(false),
                    value_size: 4 * count_tts!($($type )+),
                    $($field,)+
                    end: 0,
                }
            }
        }
    };
}

buffer! {
    GetClockRateBuffer,
    Tag::GetClockRate,
    clock: Clock,
    rate: MaybeUninit<Option<NonZeroU32>>,
}

buffer! {
    GetMaxClockRateBuffer,
    Tag::GetMaxClockRate,
    clock: Clock,
    rate: MaybeUninit<Option<NonZeroU32>>,
}

buffer! {
    SetClockRateBuffer,
    Tag::SetClockRate,
    clock: Clock,
    rate: Option<NonZeroU32>,
    skip_setting_turbo: u32,
}

/// A property mailbox driver
pub struct Mailbox<'mailbox> {
    /// The memory-mapped registers that operate this mailbox
    registers: &'mailbox mut Registers,
}

#[allow(dead_code)]
impl Mailbox<'_> {
    /// Creates a wrapper for a memory-mapped mailbox interface at the given base register address.
    ///
    /// Returns `None` if the pointer is not suitably aligned
    ///
    /// # Safety
    /// * The address must point to a valid memory-mapped mailbox register set
    /// * The mailbox registers must be valid for at least as long as this wrapper exists
    /// * The mailbox registers must not be accessed in any other way while this wrapper exists
    pub unsafe fn new(base_address: NonZeroUsize) -> Option<Self> {
        let mut registers: NonNull<Registers> =
            NonNull::new(ptr::from_exposed_addr_mut(base_address.get()))?;

        if !registers.as_ptr().is_aligned() {
            return None;
        }

        Some(Self {
            // SAFETY: The caller upholds the conditions necessary for exclusivity and accessing,
            // and we have verified alignment
            registers: unsafe { registers.as_mut() },
        })
    }

    /// Sends the buffer to the mailbox, and waits for a response. Returns whether or not the
    /// communication was successful
    fn send<T>(&mut self, buffer: &mut T) -> bool {
        // Verify that the buffer's address fits into a `u32` and is aligned
        let Ok(buffer_addr) = u32::try_from(ptr::from_mut(buffer).addr()) else {
            return false
        };
        if buffer_addr % 16 != 0 {
            return false;
        }

        // Make sure the buffer writes are fully complete
        // SAFETY: This is defined on the Raspberry Pi
        unsafe { core::arch::aarch64::__dmb(OSHST) }

        // Wait for the mailbox to be available
        while self.registers.status.matches_any(STATUS::FULL::Full) {
            core::hint::spin_loop();
        }

        // Send the buffer's address to the mailbox
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "This does not have any side effects"
        )]
        self.registers
            .write
            .write(DATA::DATA.val(buffer_addr >> 4) + DATA::CHANNEL::PropertyTagsToVc);

        // Wait for a responnse to be ready
        while self.registers.status.matches_any(STATUS::EMPTY::Empty) {
            core::hint::spin_loop();
        }

        let data = self.registers.data.extract();
        // Since we only use one channel, we expect the channel of the response to always match
        // Since we only use synchronous responses, we expect the returned buffer to always match
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "This does not have any side effects"
        )]
        data.matches_all(DATA::DATA.val(buffer_addr >> 4) + DATA::CHANNEL::PropertyTagsToVc)
    }

    /// Returns the maximum supported clock rate for the given clock. Clocks should not be set
    /// higher than this.
    ///
    /// Returns `None` if any errors occur
    pub fn get_clock_rate(&mut self, clock: Clock) -> Option<NonZeroU32> {
        let mut buffer = GetClockRateBuffer::new(clock, MaybeUninit::uninit());
        if self.send(&mut buffer) {
            // SAFETY: The pointer is appropriately constructed from the buffer
            let clock_rate = unsafe { ptr::addr_of!(buffer.rate).read_volatile() };
            // SAFETY: The mailbox response initializes this field
            unsafe { clock_rate.assume_init() }
        } else {
            None
        }
    }

    /// Returns the maximum supported clock rate for the given clock. Clocks should not be set
    /// higher than this.
    ///
    /// Returns `None` if any errors occur
    pub fn get_max_clock_rate(&mut self, clock: Clock) -> Option<NonZeroU32> {
        let mut buffer = GetMaxClockRateBuffer::new(clock, MaybeUninit::uninit());
        if self.send(&mut buffer) {
            // SAFETY: The pointer is appropriately constructed from the buffer
            let clock_rate = unsafe { ptr::addr_of!(buffer.rate).read_volatile() };
            // SAFETY: The mailbox response initializes this field
            unsafe { clock_rate.assume_init() }
        } else {
            None
        }
    }

    /// Sets the clock rate for the given clock. May be clamped to supported ranges.
    ///
    /// Returns `None` if any errors occur, or `Some(rate)` for the returned rate
    pub fn set_clock_rate(&mut self, clock: Clock, rate: NonZeroU32) -> Option<NonZeroU32> {
        let mut buffer = SetClockRateBuffer::new(clock, Some(rate), 0);
        if self.send(&mut buffer) {
            // SAFETY: The pointer is appropriately constructed from the buffer
            unsafe { ptr::addr_of!(buffer.rate).read_volatile() }
        } else {
            None
        }
    }
}

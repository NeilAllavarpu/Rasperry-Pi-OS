//! Driver for the Raspberry Pi's GPIO pins. See items for more information

use core::{
    num::{NonZeroU8, NonZeroUsize},
    ptr::{self, NonNull},
};

/// Selects functionality for a GPIO pin
#[allow(dead_code)]
pub enum FunctionSelect {
    Input = 0b000,
    Output = 0b001,
    Alt0 = 0b100,
    Alt1 = 0b101,
    Alt2 = 0b110,
    Alt3 = 0b111,
    Alt4 = 0b011,
    Alt5 = 0b010,
}

/// Sets the pull of the internal resistors for a GPIO pin
#[allow(dead_code)]
pub enum Pull {
    Off = 0b00,
    Up = 0b01,
    Down = 0b10,
}

/// A driver to control GPIO pin functionality
pub struct Gpio {
    /// Base address of the GPIO registers
    base_address: NonNull<u32>,
}

impl Gpio {
    /// Number of GPIO pins
    const NUM_PINS: u8 = 58;

    /// Creates a wrapper for a memory-mapped GPIO interface at the given base register address.
    ///
    /// Returns `None` if the pointer is not suitably aligned
    ///
    /// # Safety
    /// * The address must point to a valid memory-mapped GPIO register set
    /// * The GPIO registers must be valid for at least as long as this wrapper exists
    /// * The GPIO registers must not be accessed in any other way while this wrapper exists
    #[inline]
    #[expect(clippy::unwrap_in_result, reason = "This conversion can never fail")]
    pub unsafe fn new(base_address: NonZeroUsize) -> Option<Self> {
        #[expect(clippy::unwrap_used, reason = "This conversion can never fail")]
        let address = NonNull::new(ptr::from_exposed_addr_mut::<u32>(base_address.get())).unwrap();
        if !address.as_ptr().is_aligned() {
            return None;
        }
        Some(Self {
            base_address: address,
        })
    }

    /// Sets a field of a GPIO register to the specified value. Will yield incorrect results if the
    ///  width of `value` is larget than the specified `field_width`
    ///
    /// # Safety
    ///
    /// `register_offset` must be a valid offset to a register, and `field_width` must also be
    /// computed appropriately
    ///
    /// # Panics
    ///
    /// Panics if `pin` is out of bounds
    /// May panic if `field_width` is invalid (i.e. greater than 32)
    #[inline]
    unsafe fn set_field(&mut self, base_offset: usize, pin: u8, field_width: NonZeroU8, value: u8) {
        debug_assert!(
            u32::from(field_width.get()) <= u32::BITS,
            "Field width should be at most 32 bits"
        );
        debug_assert!(
            value.ilog2() <= field_width.get().into(),
            "Width of values should be at most the width of a field"
        );
        assert!(pin < Self::NUM_PINS, "Pin should be in bounds");
        #[expect(clippy::unwrap_used, reason = "This conversion can never fail")]
        let fields_per_u32 =
            NonZeroU8::new(u8::try_from(u32::BITS).unwrap() / field_width).unwrap();

        let register_index = pin / fields_per_u32;
        #[expect(clippy::unwrap_used, reason = "This multiplication can never fail")]
        let register_offset = (pin % fields_per_u32)
            .checked_mul(field_width.get())
            .unwrap();

        // SAFETY: This address is valid by assertion that `pin` is valid, the offset is valid, and
        // the rest of the computation computes an in-bounds target
        let register_addr = unsafe {
            self.base_address.as_ptr().add(
                #[expect(clippy::unwrap_used, reason = "This conversion can never fail")]
                base_offset.checked_add(register_index.into()).unwrap(),
            )
        };
        // SAFETY: see above
        let mut val = unsafe { register_addr.read_volatile() };
        // Clear the old setting and apply the new one
        val &= !((#[expect(clippy::unwrap_used, reason = "This shift can never fail")]
        1_u32.checked_shl(field_width.get().into()).unwrap()
            - 1)
            << register_offset);
        val |= u32::from(value) << register_offset;
        // SAFETY: see above
        unsafe { register_addr.write_volatile(val) }
    }

    /// Selects the function for the given pin
    ///
    /// # Panics
    ///
    /// Panics if the pin is out of bounds
    #[inline]
    pub fn select_function(&mut self, pin: u8, function: FunctionSelect) {
        // SAFETY: the appropriate registers are defined at offset 0 with 3 bits per field
        unsafe {
            #[expect(clippy::unwrap_used, reason = "This conversion can never fail")]
            #[expect(
                clippy::as_conversions,
                reason = "No better way to convert enums to primitve without external crates"
            )]
            self.set_field(0, pin, NonZeroU8::new(3).unwrap(), function as u8);
        }
    }

    /// Selects the pull on the given pin
    ///
    /// # Panics
    ///
    /// Panics if the pin is out of bounds
    #[inline]
    pub fn select_pull(&mut self, pin: u8, pull: Pull) {
        // SAFETY: the appropriate registers are defined at offset 0xE4 with 2 bits per field
        unsafe {
            #[expect(clippy::unwrap_used, reason = "This conversion can never fail")]
            #[expect(
                clippy::as_conversions,
                reason = "No better way to convert enums to primitve without external crates"
            )]
            self.set_field(0xE4, pin, NonZeroU8::new(2).unwrap(), pull as u8);
        }
    }
}

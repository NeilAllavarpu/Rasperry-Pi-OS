//! Driver for the Raspberry Pi's UART. See items for more information

use core::arch::aarch64::{self, OSH};
use core::fmt::{self, Write};
use core::hint;
use core::num::NonZeroUsize;
use core::ptr::{self, NonNull};
use tock_registers::interfaces::{Readable, Writeable};
use tock_registers::registers::{Aliased, ReadOnly};
use tock_registers::{register_bitfields, register_structs};

/// IO errors associated with UART
#[derive(Debug)]
pub enum IoError {
    /// A break error occurred on the connection
    Break,
    /// A frame error occurred on the connection
    Frame,
    /// The receive FIFO was overrun
    Overrun,
    /// A parity error occured on received data
    Parity,
}

/// A driver to operate a UART's reads and writes
pub struct Uart<'uart> {
    /// The memory-mapped registers corresponding to this UART
    registers: &'uart mut UartRegisters,
}

register_bitfields! {
    u32,
    /// The data register
    ///
    /// If the FIFOs are enabled, the data byte and the 4-bit status (break, frame, parity, and
    /// overrun) is pushed onto the 12-bit wide receive FIFO
    ///
    /// If the FIFOs are not enabled, the data byte and status are stored in the receiving holding
    /// register (the bottom word of the receive FIFO).
    DR_R [
        /// Receive data character
        DATA OFFSET(0) NUMBITS(8) [],
    ],
    /// The data register. The write operation initiates transmission from the UART. The data is
    /// prefixed with a start bit, appended with the appropriate parity bit (if parity is enabled),
    /// and a stop bit. The resultant word is then transmitted.
    ///
    /// If the FIFOs are enabled, data written to this location is pushed onto the transmit FIFO.
    /// If the FIFOs are not enabled, data is stored in the transmitter holding register (the
    /// bottom word of the transmit FIFO).
    DR_W [
        /// Transmit data character
        DATA OFFSET(0) NUMBITS(8),
    ],
    /// Tne flag register
    FR [
        /// Transmit FIFO full. The meaning of this bit depends on the state of the `FEN` bit in
        /// the `UART_LCRH` Register.
        ///
        /// If the FIFO is disabled, this bit is set when the transmit holding register is full.
        ///
        /// If the FIFO is enabled, the `TXFF` bit is set when the transmit FIFO is full.
        TXFF OFFSET(5) NUMBITS(1) [
            Nonfull = 0,
            Full = 1
        ],
        TXFE OFFSET(7) NUMBITS(1) [],
    ],
    /// The raw interrupt status register
    RIS [
        /// Overrun error interrupt status
        OERIS OFFSET(10) NUMBITS(1) [
            Idle = 0,
            Pending = 1,
        ],
        /// Break error interrupt status
        BERIS OFFSET(9) NUMBITS(1) [
            Idle = 0,
            Pending = 1,
        ],
        /// Parity error interrupt status
        PERIS OFFSET(8) NUMBITS(1) [
            Idle = 0,
            Pending = 1,
        ],
        /// Frame error interrupt status
        FERIS OFFSET(7) NUMBITS(1) [
            Idle = 0,
            Pending = 1,
        ],
        /// Receive timeout interrupt status
        RTRIS OFFSET(6) NUMBITS(1) [
            Idle = 0,
            Pending = 1,
        ],
        /// Transmit interrupt status
        TXRIS OFFSET(5) NUMBITS(1) [
            Idle = 0,
            Pending = 1,
        ],
        /// Receive interrupt status
        RXRIS OFFSET(4) NUMBITS(1) [
            Idle = 0,
            Pending = 1,
        ],
        /// nUARTCTS modem interrupt status
        CTSRMIS OFFSET(1) NUMBITS(1) [
            Idle = 0,
            Pending = 1,
        ],
    ],
}

register_structs! {
    pub UartRegisters {
        (0x00 => dr: Aliased<u32, DR_R::Register, DR_W::Register>),
        (0x04 => _unused0),
        (0x18 => fr: ReadOnly<u32, FR::Register>),
        (0x1C => _unused1),
        (0x3C => ris: ReadOnly<u32, RIS::Register>),
        (0x40 => @END),
    }
}

impl<'uart> Uart<'uart> {
    /// Creates a wrapper for a memory-mapped UART interface at the given base register address,
    /// and initializes the UART appropriately
    ///
    /// Returns `None` if the pointer is not suitably aligned
    ///
    /// # Safety
    /// * The address must point to a valid memory-mapped UART register set
    /// * The UART registers must be valid for at least as long as this wrapper exists
    /// * The UART registers must not be accessed in any other way while this wrapper exists
    pub unsafe fn new(base_address: NonZeroUsize) -> Option<Self> {
        let mut registers =
            // SAFETY: `base_address` is guaranteed to be nonzero
            unsafe { NonNull::new_unchecked(ptr::from_exposed_addr_mut::<UartRegisters>(base_address.get())) };

        if !registers.as_ptr().is_aligned() {
            return None;
        }

        Some(Self {
            registers:
                // SAFETY:
                // * The pointer is properly aligned by the above check
                // * The caller guarantees that the addess points to a valid, dereferencable set of UART
                // registers
                // * The caller guarantees that the lifetime of the registers is at least as long as this
                // struct
                // * The caller guarantees that the registers are never accessed in any other way while
                // this struct lives, and this memory is only accessed via this reference inside this
                // struct
        unsafe { registers.as_mut() },
        })
    }

    /// Returns `Ok` if no errors are currently found on the UART, otherwise returns an `Err`
    /// corresponding to the first error found (arbitrarily decided).
    fn check_errors(&self) -> Result<(), IoError> {
        let ris = self.registers.ris.extract();
        if ris.matches_any(&[RIS::OERIS::Pending]) {
            Err(IoError::Overrun)
        } else if ris.matches_any(&[RIS::BERIS::Pending]) {
            Err(IoError::Break)
        } else if ris.matches_any(&[RIS::PERIS::Pending]) {
            Err(IoError::Parity)
        } else if ris.matches_any(&[RIS::FERIS::Pending]) {
            Err(IoError::Frame)
        } else {
            Ok(())
        }
    }

    /// Writes a single byte to the UART
    ///
    /// Returns `Ok` if successful
    ///
    /// Returns an `Err` if an IO error occurs
    pub fn write_byte(&mut self, byte: u8) -> Result<(), IoError> {
        // SAFETY: This is well defined on the Raspberry Pi
        unsafe {
            aarch64::__dmb(OSH);
        }
        while self.registers.fr.matches_any(&[FR::TXFF::Full]) {
            self.check_errors()?;
            hint::spin_loop();
        }
        self.registers.dr.write(DR_W::DATA.val(byte.into()));
        // SAFETY: This is well defined on the Raspberry Pi
        unsafe {
            aarch64::__dmb(OSH);
        }
        Ok(())
    }

    /// Writes multiple bytes to the UART
    ///
    /// Returns `Ok` if all bytes are written
    ///
    /// Returns an `Err` if an IO error occurs at any point
    pub fn write_bytes(&mut self, bytes: &[u8]) -> Result<(), IoError> {
        for byte in bytes {
            self.write_byte(*byte)?;
        }
        Ok(())
    }
}

#[expect(clippy::missing_trait_methods, reason = "Specialization not necessary")]
impl Write for Uart<'_> {
    fn write_str(&mut self, string: &str) -> fmt::Result {
        self.write_bytes(string.as_bytes())
            .map_err(|_err| fmt::Error)
    }
}

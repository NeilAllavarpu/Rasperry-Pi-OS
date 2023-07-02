//! Driver for the Raspberry Pi's UART. See items for more information

use core::{
    fmt::{self, Write},
    hint,
    mem::MaybeUninit,
    num::NonZeroUsize,
    ptr::{self, NonNull},
};
use tock_registers::interfaces::ReadWriteable;
use tock_registers::{
    interfaces::{Readable, Writeable},
    register_bitfields, register_structs,
    registers::{Aliased, ReadOnly, ReadWrite, WriteOnly},
};

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
        /// Overrun error
        OE OFFSET(11) NUMBITS(1) [
            /// There is an empty space in the FIFO and a new character can be written to it
            HasSpace = 0,
            /// Data has been received and the receive FIFO is already full
            Overrun = 1
        ],
        /// Break error. This bit is set to 1 if a break condition was detected, indicating that
        /// the received data input was held LOW for longer than a full-word transmission time
        /// (defined as start, data, parity and stop bits).
        ///
        /// In FIFO mode, this error is associated with the character at the top of the FIFO. When
        /// a break occurs, only one 0 character is loaded into the FIFO. The next character is
        /// only enabled after the receive data input goes to a 1 (marking state), and the next
        /// valid start bit is received.
        BE OFFSET(10) NUMBITS(1) [
            NoBreak = 0,
            Break = 1,
        ],
        /// Parity error. When set to 1, it indicates that the parity of the received data
        /// character does not match the parity that the EPS and SPS bits in the Line Control
        /// Register,` UART_LCRH` select.
        ///
        /// In FIFO mode, this error is associated with the character at the top of the FIFO.
        PE OFFSET(9) NUMBITS(1) [
            ParityMatch = 0,
            ParityMismatch = 1,
        ],
        /// Framing error. When set to 1, it indicates that the received character did not have a
        /// valid stop bit (a valid stop bit is 1).
        ///
        /// In FIFO mode, this error is associated with the character at the top of the FIFO.
        FE OFFSET(8) NUMBITS(1) [
            Stop = 0,
            NoStop = 1
        ],
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
    /// The flag register
    FR [
        /// Transmit FIFO empty. The meaning of this bit depends on the state of the `FEN` bit in
        /// the Line Control Register, `UART_LCRH`. This bit does not indicate if there is data in
        /// the transmit shift register.
        ///
        /// If the FIFO is disabled, this bit is set when the transmit holding register is empty.
        ///
        /// If the FIFO is enabled, the `TXFE` bit is set when the transmit FIFO is empty.
        TXFE OFFSET(7) NUMBITS(1) [
            Nonempty = 0,
            Empty = 1
        ],
        /// Receive FIFO full. The meaning of this bit depends on the state of the `FEN` bit in the
        /// `UART_LCRH` Register.
        ///
        /// If the FIFO is disabled, this bit is set when the receive holding register is full.
        ///
        /// If the FIFO is enabled, the `RXFF` bit is set when the receive FIFO is full.
        RXFF OFFSET(6) NUMBITS(1) [
            Nonfull = 0,
            Full = 1,
        ],
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
        /// Receive FIFO empty. The meaning of this bit depends on the state of the `FEN` bit in
        /// the `UART_LCRH` Register.
        ///
        /// If the FIFO is disabled, this bit is set when the receive holding register is empty.
        ///
        /// If the FIFO is enabled, the `RXFE` bit is set when the receive FIFO is empty.
        RXFE OFFSET(4) NUMBITS(1) [
            Nonempty = 0,
            Empty = 1
        ],
        /// UART busy. If this bit is set to 1, the UART is busy transmitting data. This bit
        /// remains set until the complete byte, including all the stop bits, has been sent from
        /// the shift register. This bit is set as soon as the transmit FIFO becomes non-empty,
        /// regardless of whether the UART is enabled or not.
        BUSY OFFSET(3) NUMBITS(1) [
            Idle = 0,
            Transmitting = 1
        ],
        /// Clear to send. This bit is the complement of the UART clear to send, nUARTCTS, modem
        /// status input. That is, the bit is 1 when nUARTCTS is LOW.
        CTS OFFSET(0) NUMBITS(1) []
    ],
    /// The integer part of the baud rate divisor value.
    IBRD [
        /// The integer baud rate divisor.
        IBRD OFFSET(0) NUMBITS(16) []
    ],
    /// The fractional part of the baud rate divisor value.
    ///
    /// The baud rate divisor is calculated as follows:
    /// Baud rate divisor `BAUDDIV = (FUARTCLK/(16 * Baud rate))`
    /// where `FUARTCLK` is the UART reference clock frequency. The `BAUDDIV` is comprised of the integer value `IBRD` and the fractional value `FBRD`.
    ///
    /// NOTE: The contents of the IBRD and FBRD registers are not updated until transmission or reception of the current character is complete.
    FBRD [
        /// The fractional baud rate divisor.
        FBRD OFFSET(0) NUMBITS(6) []
    ],
    /// The line control register.
    ///
    /// NOTE: The `UART_LCRH`, `UART_IBRD`, and `UART_FBRD` registers must not be changed:
    /// * when the UART is enabled
    /// * when completing a transmission or a reception when it has been programmed to become
    /// disabled.
    LCRH [
        /// Stick parity select
        SPS OFFSET(7) NUMBITS(1) [
            Disabled = 0,
            /// If the `EPS` bit is 0 then the parity bit is transmitted and checked as a 1
            /// If the `EPS` bit is 1 then the parity bit is transmitted and checked as a 0
            Enabled = 1,
        ],
        /// Word length. These bits indicate the number of data bits transmitted or received in a
        /// frame
        WLEN OFFSET(5) NUMBITS(2) [
            Bits8 = 0b11,
            Bits7 = 0b10,
            Bits6 = 0b01,
            Bits5 = 0b00
        ],
        /// Enable FIFOs
        FEN OFFSET(4) NUMBITS(1) [
            /// FIFOs are disabled - the FIFOs become 1-byte-deep holding registers
            Character = 0,
            /// Transmit and receive FIFO buffers are enabled
            Fifo = 1
        ],
        /// Two stop bits select. If this bit is set to 1, two stop bits are transmitted at the end
        /// of the frame. The receive logic does not check for two stop bits being received.
        STP2 OFFSET(3) NUMBITS(1) [
            One = 0,
            Two = 1
        ],
        /// Even parity select. Controls the type of parity the UART uses during transmission and
        /// reception. This bit has no effect when the PEN bit disables parity checking and
        /// generation.
        EPS OFFSET(2) NUMBITS(1) [
            /// The UART generates or checks for an odd number of 1s in the data and parity bits.
            Odd = 0,
            /// The UART generates or checks for an even number of 1s in the data and parity bits.
            Even = 1
        ],
        /// Parity enable
        PEN OFFSET(1) NUMBITS(1) [
            /// Parity is disabled and no parity bit added to the data frame
            Disabled = 0,
            /// Parity checking and generation is enabled
            Enabled = 1,
        ],
        /// Send break. If this bit is set to 1, a low-level is continually output on the `TXD`
        /// output, after completing transmission of the current character.
        BRK OFFSET(0) NUMBITS(1) [
            Off = 0,
            Break = 1,
        ]
    ],
    /// The control register
    CR [
        /// CTS hardware flow control enable.
        CTSEN OFFSET(15) NUMBITS(1) [
            Disabled = 0,
            /// CTS hardware flow control is enabled. Data is only transmitted when the nUARTCTS
            /// signal is asserted.
            Enabled = 1
        ],
        /// RTS hardware flow control enable
        RTSEN OFFSET(14) NUMBITS(1) [
            Disabled = 0,
            /// RTS hardware flow control enable. If this bit is set to 1, RTS hardware flow
            /// control is enabled. Data is only requested when there is space in the receive FIFO
            /// for it to be received.
            Enabled = 1
        ],
        /// Request to send. This bit is the complement of the UART request to send, nUARTRTS,
        /// modem status output. That is, when the bit is programmed to a 1 then nUARTRTS is LOW.
        RTS OFFSET(11) NUMBITS(1) [],
        /// Receive enable
        RXE OFFSET(9) NUMBITS(1) [
            /// When the UART is disabled in the middle of reception, it completes the current
            /// character before stopping.
            Disabled = 0,
            /// The receive section of the UART is enabled. Data reception occurs for UART signals
            Enabled = 1,
        ],
        /// Transmit enable
        TXE OFFSET(8) NUMBITS(1) [
            /// When the UART is disabled in the middle of transmission, it completes the current
            /// character before stopping
            Disabled = 0,
            /// The transmit section of the UART is enabled. Data transmission occurs for UART
            /// signals.
            Enabled = 1,
        ],
        /// Loopback enable. This bit is cleared to 0 on reset, to disable loopback.
        LBE OFFSET(7) NUMBITS(1) [
            Disabled = 0,
            /// The UARTTXD path is fed through to the UARTRXD path. In UART mode, the modem
            /// outputs are also fed through to the modem inputs.
            Enabled = 1
        ],
        /// UART enable
        UARTEN OFFSET(0) NUMBITS(1) [
            /// If the UART is disabled in the middle of transmission or reception, it completes
            /// the current character before stopping
            Disabled = 0,
            Enabled = 1,
        ],
    ],
    /// The interrupt FIFO level select register. You can use this register to define the FIFO level
    /// that triggers the assertion of the combined interrupt signal. The interrupts are generated
    /// based on a transition through a level rather than being based on the level. That is, the
    /// interrupts are generated when the fill level progresses through the trigger level.
    IFLS [
        /// Receive interrupt FIFO level select
        RXIFLSEL OFFSET(3) NUMBITS(3) [
            OneEighth = 0b000,
            OneFourth = 0b001,
            OneHalf = 0b010,
            ThreeFourths = 0b011,
            SevenEights = 0b100,
        ],
        /// Transmit interrupt FIFO level select
        TXIFLSEL OFFSET(0) NUMBITS(3) [
            OneEighth = 0b000,
            OneFourth = 0b001,
            OneHalf = 0b010,
            ThreeFourths = 0b011,
            SevenEights = 0b100,
        ],
    ],
    /// The interrupt mask set/clear register
    IMSC [
        /// Overrun error interrupt mask
        OEIM OFFSET(10) NUMBITS(1) [
            Unmasked = 0,
            Masked = 1,
        ],
        /// Break error interrupt mask
        BEIM OFFSET(9) NUMBITS(1) [
            Unmasked = 0,
            Masked = 1,
        ],
        /// Parity error interrupt mask
        PEIM OFFSET(8) NUMBITS(1) [
            Unmasked = 0,
            Masked = 1,
        ],
        /// Frame error interrupt mask
        FEIM OFFSET(7) NUMBITS(1) [
            Unmasked = 0,
            Masked = 1,
        ],
        /// Receive timeout interrupt mask
        RTIM OFFSET(6) NUMBITS(1) [
            Unmasked = 0,
            Masked = 1,
        ],
        /// Transmit interrupt mask
        TXIM OFFSET(5) NUMBITS(1) [
            Unmasked = 0,
            Masked = 1,
        ],
        /// Receive interrupt mask
        RXIM OFFSET(4) NUMBITS(1) [
            Unmasked = 0,
            Masked = 1,
        ],
        /// nUARTCTS modem interrupt mask
        CTSIMM OFFSET(1) NUMBITS(1) [
            Unmasked = 0,
            Masked = 1,
        ],
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
    /// The masked interrupt status register. This register returns the current masked status value of the corresponding interrupt.
    MIS [
        /// Overrun error masked interrupt status
        OEMIS OFFSET(10) NUMBITS(1) [
            Idle = 0,
            Pending = 1,
        ],
        /// Break error masked interrupt status
        BEMIS OFFSET(9) NUMBITS(1) [
            Idle = 0,
            Pending = 1,
        ],
        /// Parity error masked interrupt status
        PEMIS OFFSET(8) NUMBITS(1) [
            Idle = 0,
            Pending = 1,
        ],
        /// Frame error masked interrupt status
        FEMIS OFFSET(7) NUMBITS(1) [
            Idle = 0,
            Pending = 1,
        ],
        /// Receive timeout masked interrupt status
        RTMIS OFFSET(6) NUMBITS(1) [
            Idle = 0,
            Pending = 1,
        ],
        /// Transmit masked interrupt status
        TXMIS OFFSET(5) NUMBITS(1) [
            Idle = 0,
            Pending = 1,
        ],
        /// Receive masked interrupt status
        RXMIS OFFSET(4) NUMBITS(1) [
            Idle = 0,
            Pending = 1,
        ],
        /// nUARTCTS modem masked interrupt status
        CTSMMIS OFFSET(1) NUMBITS(1) [
            Idle = 0,
            Pending = 1,
        ],
    ],
    /// The interrupt clear register
    ICR [
        /// Overrun error interrupt clear
        OEIC OFFSET(10) NUMBITS(1) [
            Clear = 1,
        ],
        /// Break error interrupt clear
        BEIC OFFSET(9) NUMBITS(1) [
            Clear = 1,
        ],
        /// Parity error interrupt clear
        PEIC OFFSET(8) NUMBITS(1) [
            Clear = 1,
        ],
        /// Frame error interrupt clear
        FEIC OFFSET(7) NUMBITS(1) [
            Clear = 1,
        ],
        /// Receive timeout interrupt clear
        RTIC OFFSET(6) NUMBITS(1) [
            Clear = 1,
        ],
        /// Transmit interrupt clear
        TXIC OFFSET(5) NUMBITS(1) [
            Clear = 1,
        ],
        /// Receive interrupt clear
        RXIC OFFSET(4) NUMBITS(1) [
            Clear = 1,
        ],
        /// nUARTCTS modem interrupt clear
        CTSMIC OFFSET(1) NUMBITS(1) [
            Clear = 1,
        ],
    ],
    /// The DMA control register
    DMACR [
        /// DMA on error. If this bit is set to 1, the DMA receive request outputs are disabled
        /// when the UART error interrupt is asserted.
        DMAONERR OFFSET(2) NUMBITS(1) [
            Disabled = 0,
            Enabled = 1,
        ],
        /// Transmit DMA enable. If this bit is set to 1, DMA for the transmit FIFO is enabled.
        TXDMAE OFFSET(1) NUMBITS(1) [
            Disabled = 0,
            Enabled = 1,
        ],
        /// Receive DMA enable. If this bit is set to 1, DMA for the receive FIFO is enabled.
        RXDMAE OFFSET(0) NUMBITS(1) [
            Disabled = 0,
            Enabled = 1,
        ]
    ]
}

register_structs! {
    pub UartRegisters {
        (0x00 => dr: Aliased<u32, DR_R::Register, DR_W::Register>),
        (0x04 => _unused0),
        (0x18 => fr: ReadOnly<u32, FR::Register>),
        (0x1C => _unused1),
        (0x24 => ibrd: ReadWrite<u32, IBRD::Register>),
        (0x28 => fbrd: ReadWrite<u32, FBRD::Register>),
        (0x2C => lcrh: ReadWrite<u32, LCRH::Register>),
        (0x30 => cr: ReadWrite<u32, CR::Register>),
        (0x34 => ifls: ReadWrite<u32, IFLS::Register>),
        (0x38 => imsc: ReadWrite<u32, IMSC::Register>),
        (0x3C => ris: ReadOnly<u32, RIS::Register>),
        (0x40 => mis: ReadOnly<u32, MIS::Register>),
        (0x44 => icr: WriteOnly<u32, ICR::Register>),
        (0x48 => dmacr: ReadWrite<u32, DMACR::Register>),
        (0x4C => @END),
    }
}

#[allow(dead_code)]
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

        // SAFETY:
        // * The pointer is properly aligned by the above check
        // * The caller guarantees that the addess points to a valid, dereferencable set of UART
        // registers
        // * The caller guarantees that the lifetime of the registers is at least as long as this
        // struct
        // * The caller guarantees that the registers are never accessed in any other way while
        // this struct lives, and this memory is only accessed via this reference inside this
        // struct
        let registers = unsafe { registers.as_mut() };

        // 2. Wait for the end of transmission or reception of the current character.
        // Note: 2 and 1 are swapped because if the FIFO is enabled, then the busy flag will be
        // always set if any characters are left in the transmit FIFO, even though no transmission
        // occurs
        while registers.fr.matches_any(FR::BUSY::Transmitting) {
            hint::spin_loop();
        }

        // 1. Disable the UART
        registers.cr.write(CR::UARTEN::Disabled);

        // 3. Flush the transmit FIFO by setting the FEN bit to 0 in the Line Control Register, UART_LCRH.
        // This step is not necessary because we have already checked that the entire TX FIFO is
        // empty

        // 4. Reprogram the Control Register, UART_CR.
        // 5. Enable the UART.
        // ASSUMPTION: The baud rate is programmed by `config.txt` for us
        registers.icr.set(0xFFFF_FFFF);
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "These do not have side effects"
        )]
        // registers.ibrd.write(IBRD::IBRD.val(3));
        // registers.fbrd.write(FBRD::FBRD.val(16));
        registers.lcrh.write(
            LCRH::SPS::Disabled
                + LCRH::WLEN::Bits8
                + LCRH::FEN::Fifo
                + LCRH::STP2::One
                + LCRH::PEN::Disabled
                + LCRH::BRK::Off,
        );
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "These do not have side effects"
        )]
        registers.cr.write(
            CR::CTSEN::Disabled
                + CR::RTSEN::Disabled
                + CR::RXE::Enabled
                + CR::TXE::Enabled
                + CR::LBE::Disabled
                + CR::UARTEN::Enabled,
        );

        Some(Self { registers })
    }

    /// Sets the integral and fractional divisors of the baud rate
    pub fn set_divider(&mut self, integral: u16, fractional: u8) {
        // 2. Wait for the end of transmission or reception of the current character.
        // Note: 2 and 1 are swapped because if the FIFO is enabled, then the busy flag will be
        // always set if any characters are left in the transmit FIFO, even though no transmission
        // occurs
        while self.registers.fr.matches_any(FR::BUSY::Transmitting) {
            hint::spin_loop();
        }

        // 1. Disable the UART
        self.registers.cr.modify(CR::UARTEN::Disabled);

        // 3. Flush the transmit FIFO by setting the FEN bit to 0 in the Line Control Register, UART_LCRH.
        // This step is not necessary because we have already checked that the entire TX FIFO is
        // empty
        // 4. Reprogram the Control Register, UART_CR.
        self.registers.ibrd.write(IBRD::IBRD.val(integral.into()));
        self.registers.fbrd.write(FBRD::FBRD.val(fractional.into()));
        // 5. Enable the UART.
        self.registers.cr.modify(CR::UARTEN::Enabled);
    }

    /// Returns `Ok` if no errors are currently found on the UART, otherwise returns an `Err`
    /// corresponding to the first error found (arbitrarily decided).
    fn check_errors(&self) -> Result<(), IoError> {
        let ris = self.registers.ris.extract();
        if ris.matches_any(RIS::OERIS::Pending) {
            Err(IoError::Overrun)
        } else if ris.matches_any(RIS::BERIS::Pending) {
            Err(IoError::Break)
        } else if ris.matches_any(RIS::PERIS::Pending) {
            Err(IoError::Parity)
        } else if ris.matches_any(RIS::FERIS::Pending) {
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
        while self.registers.fr.matches_any(FR::TXFF::Full) {
            self.check_errors()?;
            hint::spin_loop();
        }
        self.registers.dr.write(DR_W::DATA.val(byte.into()));
        Ok(())
    }

    /// Writes a little-endian `u32` to the UART
    ///
    /// Returns `Ok` if successful
    ///
    /// Returns an `Err` if an IO error occurs
    pub fn write_u32(&mut self, num: u32) -> Result<(), IoError> {
        for byte in num.to_le_bytes() {
            self.write_byte(byte)?;
        }
        Ok(())
    }

    /// Reads enough bytes to fill the given slice and fully initializes it.
    ///
    /// Guarantees that the buffer is fully initialized if the return value is `Ok`.
    ///
    /// Returns an `Err` if an IO error occurs
    #[expect(clippy::unwrap_in_result, reason = "The conversion can never fail")]
    pub fn read_bytes(&mut self, bytes: &mut [MaybeUninit<u8>]) -> Result<(), IoError> {
        for byte in bytes {
            while self.registers.fr.matches_any(FR::RXFE::Empty) {
                self.check_errors()?;
                hint::spin_loop();
            }
            #[expect(clippy::unwrap_used, reason = "This conversion can never fail")]
            byte.write(self.registers.dr.read(DR_R::DATA).try_into().unwrap());
        }
        Ok(())
    }

    /// Reads a little-endian `u32`
    ///
    /// Returns an `Err` if an IO error occurs
    pub fn read_u32(&mut self) -> Result<u32, IoError> {
        #[expect(
            clippy::as_conversions,
            reason = "A const-conversion is not possible here in other ways"
        )]
        let mut buffer = [MaybeUninit::uninit(); (u32::BITS / 8) as usize];
        self.read_bytes(&mut buffer)?;
        // SAFETY: `read_bytes` promises to initialize the buffer
        Ok(u32::from_le_bytes(unsafe {
            MaybeUninit::array_assume_init(buffer)
        }))
    }

    /// Clears all data from the receive FIFO
    pub fn clear_reads(&mut self) {
        while !self.registers.fr.matches_any(FR::RXFE::Empty) {
            self.registers.dr.read(DR_R::DATA);
        }
    }
}

#[expect(clippy::missing_trait_methods, reason = "Specialization not necessary")]
impl Write for Uart<'_> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.as_bytes() {
            self.write_byte(*byte).map_err(|_ignored| fmt::Error)?;
        }
        Ok(())
    }
}

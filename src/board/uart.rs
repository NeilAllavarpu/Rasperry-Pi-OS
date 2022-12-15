/// Documentation for the UART: <https://datasheets.raspberrypi.com/bcm2711/bcm2711-peripherals.pdf>
use crate::{architecture::SpinLock, board::Mmio, call_once, kernel, kernel::Mutex};
use core::fmt::{self, Write};
use tock_registers::{
    interfaces::{Readable, Writeable},
    register_bitfields, register_structs,
    registers::ReadWrite,
};

register_bitfields! {
    u32,
    /// The UART_DR Register is the data register.
    ///
    /// For words to be transmitted:\
    /// if the FIFOs are enabled, data written to this location is pushed onto
    /// the transmit FIFO.\
    /// if the FIFOs are not enabled, data is stored in the transmitter holding
    /// register (the bottom word of the transmit FIFO). The write operation
    /// initiates transmission from the UART. The data is prefixed with a start
    /// bit, appended with the appropriate parity bit (if parity is enabled),
    /// and a stop bit. The resultant word is then transmitted.
    ///
    /// For received words:\
    /// if the FIFOs are enabled, the data byte and the 4-bit status (break,
    /// frame, parity, and overrun) is pushed onto the 12-bit wide receive FIFO\
    /// if the FIFOs are not enabled, the data byte and status are stored in the
    /// receiving holding register (the bottom word of the receive FIFO).
    DR [
        /// Receive (read) data character.\
        /// Transmit (write) data character.
        DATA OFFSET(0) NUMBITS(8)
    ],

    IMSC [
        RXIM OFFSET(4) NUMBITS(1)
    ]
}

register_structs! {
    #[allow(non_snake_case)]
    pub RegisterBlock {
        (0x00 => DR: ReadWrite<u32, DR::Register>),
        (0x04 => _reserved),
        (0x38 => IMSC: ReadWrite<u32, IMSC::Register>),
        (0x3C => @END),
    }
}

/// Abstraction for the associated MMIO registers.
type Registers = Mmio<RegisterBlock>;

/// Inner representation of the UART
struct UartInner {
    /// The UART registers, memory mapped
    registers: Registers,
}
/// Representation of the UART.
pub struct Uart {
    /// The protected UART
    inner: SpinLock<UartInner>,
}

impl UartInner {
    /// Creates a raw UART instance
    /// # Safety
    /// The start address must be correct, and the range must not be used by anything else.
    /// This includes not initializing the UART multiple times
    pub const unsafe fn new(mmio_start_addr: *mut RegisterBlock) -> Self {
        Self {
            // SAFETY: By assumption, the start address is correct
            registers: unsafe { Registers::new(mmio_start_addr) },
        }
    }

    /// Initializes the UART
    pub fn init(&mut self) {
        self.registers.IMSC.write(IMSC::RXIM::SET);
    }

    /// Sends a byte across the UART
    fn write_byte(&mut self, c: u8) {
        // Write the character to the buffer.
        self.registers.DR.set(c.into());
    }

    /// Reads a byte from the UART, if available
    fn read_byte(&mut self) -> Option<u8> {
        // Read one character.
        Some(
            (self.registers.DR.get() & 0xFF)
                .try_into()
                .expect("Mask should prevent overflow"),
        )
    }
}

impl fmt::Write for UartInner {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            self.write_byte(byte);
        }

        Ok(())
    }
}

impl Uart {
    /// Creates a UART instance
    /// # Safety
    /// The start address must be correct, and the range must not be used by anything else.
    /// This includes not initializing the UART multiple times
    pub const unsafe fn new(start_address: *mut RegisterBlock) -> Self {
        Self {
            inner: SpinLock::new(
                // SAFETY: By assumption, the start address must be correct and proper
                unsafe { UartInner::new(start_address) },
            ),
        }
    }

    /// Initializes the UART
    pub fn init(&self) {
        call_once!();
        self.inner.lock().init();
    }
}

impl kernel::Serial for Uart {
    fn write_fmt(&self, args: core::fmt::Arguments) {
        self.inner
            .lock()
            .write_fmt(args)
            .expect("Writing to the UART should not fail");
    }

    fn read_byte(&self) -> Option<u8> {
        self.inner.lock().read_byte()
    }
}

/// The system-wide UART
// Safety: This starting address should be correct for the Raspberry Pi 3, according to its specifications
#[allow(clippy::undocumented_unsafe_blocks)] // Lint not working properly here
#[allow(clippy::as_conversions)] // Lint not working properly here
static UART: Uart = unsafe { Uart::new(0x3F20_1000 as *mut RegisterBlock) };

/// Gets the system-wide serial connection
pub fn serial() -> &'static Uart {
    &UART
}

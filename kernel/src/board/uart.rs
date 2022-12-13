use crate::{architecture::SpinLock, call_once, kernel, kernel::Mutex};
use core::{
    fmt::{self, Write},
    ops,
};
use tock_registers::{
    interfaces::{Readable, Writeable},
    register_structs,
    registers::ReadWrite,
};

register_structs! {
    #[allow(non_snake_case)]
    pub RegisterBlock {
        (0x00 => DR: ReadWrite<u32>),
        (0x04 => @END),
    }
}

pub struct Mmio<T> {
    start_addr: *mut T,
}

impl<T> Mmio<T> {
    /// Create an instance.
    pub const unsafe fn new(start_addr: *mut T) -> Self {
        Self { start_addr }
    }
}

impl<T> ops::Deref for Mmio<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.start_addr }
    }
}

/// Abstraction for the associated MMIO registers.
type Registers = Mmio<RegisterBlock>;

struct UartInner {
    registers: Registers,
}
/// Representation of the UART.
pub struct Uart {
    inner: SpinLock<UartInner>,
}

impl UartInner {
    /// Creates a raw UART instance
    /// # Safety
    /// The start address must be correct, and the range must not be used by anything else.
    /// This includes not initializing the UART multiple times
    pub const unsafe fn new(mmio_start_addr: *mut RegisterBlock) -> Self {
        Self {
            registers: unsafe { Registers::new(mmio_start_addr) },
        }
    }

    pub fn init(&mut self) {}

    /// Sends a byte across the UART
    fn write_byte(&mut self, c: u8) {
        // Write the character to the buffer.
        self.registers.DR.set(c.into());
    }

    /// Reads a byte from the UART, if available
    fn read_byte(&mut self) -> Option<u8> {
        // Read one character.
        Some(self.registers.DR.get().try_into().unwrap())
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
            inner: SpinLock::new(unsafe { UartInner::new(start_address) }),
        }
    }

    /// Initializes the UART
    pub fn init(&self) {
        call_once!();
        self.inner.lock(|inner| inner.init());
    }
}

impl kernel::Serial for Uart {
    fn write_fmt(&self, args: core::fmt::Arguments) {
        _ = self.inner.lock(|inner| inner.write_fmt(args))
    }

    fn read_byte(&self) -> Option<u8> {
        self.inner.lock(|inner| inner.read_byte())
    }
}

/// The system-wide UART
static UART: Uart = unsafe { Uart::new(0x3F201000 as *mut RegisterBlock) };

/// Gets the system-wide serial connection
pub fn serial() -> &'static Uart {
    &UART
}

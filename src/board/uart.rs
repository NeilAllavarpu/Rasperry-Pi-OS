use crate::architecture::Spinlock;
use crate::serial;
use crate::Mutex;
use core::fmt::Write;

struct UARTInner {}

impl UARTInner {
    const fn new() -> Self {
        Self {}
    }
}

impl core::fmt::Write for UARTInner {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        for byte in s.bytes() {
            unsafe {
                // Write a byte to output
                core::ptr::write_volatile(0x3F20_1000 as *mut u8, byte);
            }
        }
        Ok(())
    }
}

struct UART {
    inner: Spinlock<UARTInner>,
}

impl UART {
    // Create a new instance.
    pub const fn new() -> Self {
        Self {
            inner: Spinlock::new(UARTInner::new()),
        }
    }
}

impl serial::Write for UART {
    fn write_format_string(&self, args: core::fmt::Arguments) -> () {
        self.inner
            .lock(|raw_uart| raw_uart.write_fmt(args))
            .expect("Writing to output should not fail")
    }
}

static UART: UART = UART::new();

pub fn get() -> &'static dyn serial::Write {
    &UART
}

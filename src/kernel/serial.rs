/// A serial output
pub trait Serial {
    /// Writes a format string
    fn write_fmt(&self, bytes: core::fmt::Arguments) -> ();

    /// Attempt to read a byte as input
    fn read_byte(&self) -> Option<u8>;
}

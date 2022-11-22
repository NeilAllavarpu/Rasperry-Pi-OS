/// A serial output
pub trait Write {
    /// Writes a format string
    fn write_format_string(&self, bytes: core::fmt::Arguments);
}

pub fn get() -> &'static dyn Write {
    crate::board::serial::get()
}

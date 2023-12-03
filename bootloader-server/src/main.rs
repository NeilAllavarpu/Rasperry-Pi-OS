//! Server to accompnay the bootloader
//!
//! Communicates over a serial connection that can be selected via the `--port` flag, or an
//! automatically discovered port connected via USB, if possible. Will error if no suitable port is
//! found.
//!
//! This will by default echo all characters received over the serial connection to standard
//! output, unless an escape character (ASCII 0x1B) is received, which indicates an operation for
//! the server to do. Escape characters are followed by a byte to indicate the operation to run.
//!
//! A byte of 0 sends a kernel over the serial connection. A kernel must be selected via the
//! `--kernel` flag. First, the size of the kernel, in bytes, as a `u32` in little-endian, is sent,
//! and then the kernel itself is sent. After this, normal operation resumes. Note that the kernel
//! is loaded only when asked, so that it can be recompiled without having to restart the server.

#![warn(clippy::all)]
#![warn(clippy::restriction)]
#![warn(clippy::complexity)]
#![deny(clippy::correctness)]
#![warn(clippy::nursery)]
#![warn(clippy::pedantic)]
#![deny(clippy::perf)]
#![warn(clippy::style)]
#![deny(clippy::suspicious)]
#![deny(unsafe_op_in_unsafe_fn)]
#![allow(clippy::blanket_clippy_restriction_lints)]
#![allow(clippy::implicit_return)]
#![allow(clippy::question_mark_used)]
#![allow(clippy::shadow_reuse)]
#![allow(clippy::separated_literal_suffix)]

use clap::Parser;
use core::slice;
use core::time::Duration;
use serialport::SerialPortType;
use serialport::{DataBits, FlowControl, Parity, SerialPort, StopBits};

use std::error::Error;
use std::fs::File;
use std::io::{self, ErrorKind, Read, Write};

/// The default baud rate when opening a connection; is clamped to the maximum rate passed as an
/// argument
const DEFAULT_BAUD_RATE: u32 = 921_600;

/// Arguments to control the server conection and operations
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Serial port to connect to; will attempt to auto-discover a suitable port if not given
    #[arg(short, long)]
    port: Option<String>,

    /// Kernel to load; if not given, kernel loading support is disabled
    #[arg(short, long)]
    kernel: Option<String>,

    /// Maximum baud rate to use over the connection
    #[arg(short, long, default_value_t = DEFAULT_BAUD_RATE)]
    max_baud: u32,
}

/// Reads a single byte from the given reader. See `Read::read` for more information on error
/// conditions
///
/// Additionally, returns an `Error` of kind `UnexpectedEof` if the `read` operation returns 0
/// bytes
fn read_byte(reader: &mut impl Read) -> io::Result<u8> {
    let mut byte = 0_u8;
    reader.read(slice::from_mut(&mut byte)).and_then(|written| {
        if written == 0 {
            Err(ErrorKind::UnexpectedEof.into())
        } else {
            Ok(byte)
        }
    })
}

/// Reads a little-endian `u32` over the connection.
///
/// Propogates any errors from reading the connection
fn read_u32(reader: &mut impl Read) -> io::Result<u32> {
    let mut bytes = [0; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

/// Checks for an OK signal over the connection.
///
/// Propogates any errors from reading the connection
fn check_ok(reader: &mut impl Read) {
    #[allow(clippy::print_stderr)]
    match read_byte(reader) {
        Ok(0) => {
            eprintln!("[LOG] Operation successful!")
        }
        Ok(code) => {
            eprintln!("[WARN] Did not receive acknowledgement of operation, error code {code}")
        }
        Err(err) => match err.kind() {
            ErrorKind::TimedOut => eprintln!(
                "[WARN] Did not receive acknowledgement of operation, operation timed out"
            ),
            _ => {}
        },
    }
}

#[allow(clippy::print_stdout)]
#[allow(clippy::print_stderr)]
fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let port_name = if let Some(port_name) = args.port {
        port_name
    } else {
        let ports = serialport::available_ports()?;
        if ports.is_empty() {
            return Err("No serial ports detected".into());
        }
        println!("Available ports:");
        let mut port_name = None;
        for port in ports {
            print!("{}: ", port.port_name);
            println!(
                "{}",
                match port.port_type {
                    SerialPortType::UsbPort(_) => {
                        port_name = Some(port.port_name);
                        "USB"
                    }
                    SerialPortType::PciPort => "PCI",
                    SerialPortType::BluetoothPort => "Bluetooth",
                    SerialPortType::Unknown => "Unknown",
                }
            );
        }
        if let Some(port_name) = port_name {
            println!("Selecting port {port_name}");
            port_name
        } else {
            return Err("No USB port found to auto-select".into());
        }
    };

    let mut uart = serialport::new(port_name, DEFAULT_BAUD_RATE.min(args.max_baud))
        .data_bits(DataBits::Eight)
        .flow_control(FlowControl::None)
        .parity(Parity::None)
        .stop_bits(StopBits::One)
        .timeout(Duration::from_secs(1))
        .open_native()?;

    loop {
        match read_byte(&mut uart) {
            Ok(b'\x1B') => {
                // Received a special sequence: use the next byte to figure out what is requested
                match read_byte(&mut uart)? {
                    0 => {
                        eprintln!("[LOG] Kernel requested");
                        // Kernel loading mode
                        // 1. We send the kernel file size, in bytes
                        let mut kernel =
                            File::open(args.kernel.as_ref().ok_or("No kernel provided")?)?;
                        let kernel_size: u32 = kernel.metadata()?.len().try_into()?;
                        uart.write_all(&kernel_size.to_le_bytes())?;
                        // 3. The contents of the kernel are sent, with the amount of bytes as
                        //    specified above
                        io::copy(&mut kernel, &mut uart)?;
                        // 4. Wait for a confirmation response
                        check_ok(&mut uart);
                    }
                    1 => {
                        eprintln!("[LOG] Baud configuration requested");
                        // Clock configuration mode
                        // 1. The connection sends its maximum supported baud rate
                        let max_supported_baud_rate = read_u32(&mut uart)?;
                        // 2. We respond with the actual baud rate to use
                        let baud_rate = max_supported_baud_rate; // args.max_baud.min(max_supported_baud_rate);
                        eprintln!(
                            "[LOG] Setting baud rate to {baud_rate} baud ({} KiB/s)",
                            f64::from(baud_rate) * 0.8 / 1024.0 / 8.0
                        );
                        uart.write_all(&baud_rate.to_le_bytes())?;
                        // 3. Wait for a confirmation response
                        check_ok(&mut uart);
                        // 4. Now, we can set the baud rate of the connection
                        uart.set_baud_rate(baud_rate)?;
                    }
                    byte => {
                        eprintln!("[WARN] Bad opcode received: {byte}");
                        continue;
                    }
                }
            }
            Ok(byte) => {
                let mut stdout = io::stdout().lock();
                stdout.write_all(slice::from_ref(&byte))?;
                // We want to flush every byte to ensure as accurate printing as possible
                stdout.flush()?;
            }
            Err(err) => {
                match err.kind() {
                    ErrorKind::TimedOut => {}
                    _ => Err(err)?,
                };
            }
        }
    }
}

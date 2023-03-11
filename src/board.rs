/// Wrapper for memory-mapped registers
mod mmio;
use mmio::Mmio;
/// UART (PL011) support
mod uart;
pub use uart::serial;
/// IRQ handling
pub mod irq;

use crate::{
    call_once,
    memory::{kernel::KERNEL_TABLE, Ppn, Vpn},
};

/// The possible types of MMIO to register mappings for
pub enum MmioDevices {
    Uart = 0,
    Peripheral = 1,
    Local = 2,
}

/// Stores the virtual and physical addresses of the MMIO mapping
pub struct MmioMapping {
    pub physical_addr: usize,
    pub virtual_addr: usize,
}

/// Memory mappings of board devices
pub const MMIO_MAPPINGS: phf::Map<u8, MmioMapping> = phf::phf_map! {
    0_u8 => MmioMapping {
        physical_addr: 0x3F20_0000,
        virtual_addr: 0xFFFF_FFFF_FE20_0000,
    },
    1_u8 => MmioMapping {
        physical_addr: 0x4000_0000,
        virtual_addr: 0xFFFF_FFFF_FE21_0000,
    },
    2_u8 => MmioMapping {
        physical_addr: 0x3F00_0000,
        virtual_addr: 0xFFFF_FFFF_FE22_0000,
    }
};
/// Board-specific initialization sequences
/// # Safety
/// Must be initialized only once
pub unsafe fn init() {
    call_once!();
    for mapping in MMIO_MAPPINGS.values() {
        unsafe {
            KERNEL_TABLE
                .get_entry(Vpn::from_addr(mapping.virtual_addr))
                .expect("Should be valid")
                .set_valid(Ppn::from_addr(mapping.physical_addr));
        }
    }
    serial().init();
    irq::init();
}

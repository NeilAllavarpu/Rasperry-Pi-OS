/// Wrapper for memory-mapped registers
mod mmio;
use mmio::Mmio;
/// UART (PL011) support
mod uart;
pub use uart::serial;
/// IRQ handling
pub mod irq;

use crate::call_once;

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
        virtual_addr: 0x20_0000,
    },
    1_u8 => MmioMapping {
        physical_addr: 0x4000_0000,
        virtual_addr: 0x21_0000,
    },
    2_u8 => MmioMapping {
        physical_addr: 0x3F00_0000,
        virtual_addr: 0x22_0000,
    }
};

extern "C" {
    // Must not be run on concurrent execution paths with the same core ID
    fn _per_core_init() -> !;
}

/// Wakes up all cores and runs their per-core initialization sequences
/// # Safety
/// Must only be called once
#[allow(dead_code)]
pub unsafe fn wake_all_cores() {
    call_once!();
    #[allow(clippy::as_conversions)]
    // SAFETY: These addresses are taken from the spec for Raspbeery Pi 4
    unsafe {
        // Tell the cores to start running the per core init sequence
        // Mask, so that the address is a physical address instead of virtual
        // TODO: Perform a conversion to get this address, instead of hard
        // coding
        core::ptr::write_volatile(
            0xFFFF_FFFF_FE00_00E0 as *mut _,
            (_per_core_init as *const ()).mask(0x1FF_FFFF),
        );
        core::ptr::write_volatile(
            0xFFFF_FFFF_FE00_00E8 as *mut _,
            (_per_core_init as *const ()).mask(0x1FF_FFFF),
        );
        core::ptr::write_volatile(
            0xFFFF_FFFF_FE00_00F0 as *mut _,
            (_per_core_init as *const ()).mask(0x1FF_FFFF),
        );
    }
    // make sure the cores are notified to wake up
    aarch64_cpu::asm::sev();
}

/// Board-specific initialization sequences
/// # Safety
/// Must be initialized only once
pub unsafe fn init() {
    call_once!();
    serial().init();
    irq::init();
}

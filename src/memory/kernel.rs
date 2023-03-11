use super::{PageDescriptor, Vpn};
use tock_registers::registers::InMemoryRegister;

/// Base 2 logarithm of the size of kernel granules
const PAGE_SIZE: u8 = 16;
/// Meaningful bits in an address
const ADDRESS_BITS: u8 = 25;
/// Mask for extracting the meaningful portion of a VPN
const PAGE_ADDRESS_MASK: usize = (1 << (ADDRESS_BITS - PAGE_SIZE)) - 1;

#[repr(C, align(4096))]
/// The kernel translation table
pub struct TranslationTable {
    /// The actual descriptors
    descriptors: [PageDescriptor<PAGE_SIZE>; 1 << 9],
    // Mutex lock
    // lock: BlockingLock,
}

impl TranslationTable {
    /// Gets a mutable reference to the entry associated with the given vpn
    pub const fn get_entry(
        &mut self,
        vpn: Vpn<PAGE_SIZE>,
    ) -> Option<&mut PageDescriptor<PAGE_SIZE>> {
        if is_higher_half(vpn) {
            self.descriptors.get_mut(vpn.0 & PAGE_ADDRESS_MASK)
        } else {
            None
        }
    }
}

/// Returns whether or not the given VPN lies in the higher half address space
const fn is_higher_half<const SIZE: u8>(vpn: Vpn<SIZE>) -> bool {
    /// Masks out the top byte of a VPN
    const MASK_TOP_BYTE: usize = (1_usize << (usize::BITS - 8 - u32::from(PAGE_SIZE))) - 1;
    // True if and only if the mask range is all set bits
    (vpn.0 & !PAGE_ADDRESS_MASK & MASK_TOP_BYTE) == (!PAGE_ADDRESS_MASK & MASK_TOP_BYTE)
}

/// The global translation table for the kernel address space
#[no_mangle]
pub static mut KERNEL_TABLE: TranslationTable = TranslationTable {
    descriptors: [const { PageDescriptor(InMemoryRegister::new(0)) }; _],
    // lock: BlockingLock::new(()),
};

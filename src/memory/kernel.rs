use super::{PageDescriptor, Vpn};
use crate::sync::SpinLock;
use tock_registers::registers::InMemoryRegister;

/// Base 2 logarithm of the size of kernel granules
pub const PAGE_SIZE_LOG: u8 = 16;
/// The size of a kernel page
pub const PAGE_SIZE: usize = 1 << PAGE_SIZE_LOG;
/// Meaningful bits in an address
const ADDRESS_BITS: u8 = 25;
/// Mask for extracting the meaningful portion of a VPN
const PAGE_ADDRESS_MASK: usize = (1 << (ADDRESS_BITS - PAGE_SIZE_LOG)) - 1;

#[repr(C, align(4096))]
/// The kernel translation table
pub struct TranslationTable([PageDescriptor<PAGE_SIZE_LOG>; 1 << (ADDRESS_BITS - PAGE_SIZE_LOG)]);

impl TranslationTable {
    /// Gets a mutable reference to the entry associated with the given vpn
    pub const fn get_entry(
        &mut self,
        vpn: Vpn<PAGE_SIZE_LOG>,
    ) -> Option<&mut PageDescriptor<PAGE_SIZE_LOG>> {
        if is_higher_half(vpn) {
            self.0.get_mut(vpn.0 & PAGE_ADDRESS_MASK)
        } else {
            None
        }
    }
}

/// Returns whether or not the given VPN lies in the higher half address space
const fn is_higher_half<const SIZE: u8>(vpn: Vpn<SIZE>) -> bool {
    /// Masks out the top byte of a VPN
    const MASK_TOP_BYTE: usize = (1_usize << (usize::BITS - 8 - u32::from(PAGE_SIZE_LOG))) - 1;
    // True if and only if the mask range is all set bits
    (vpn.0 & !PAGE_ADDRESS_MASK & MASK_TOP_BYTE) == (!PAGE_ADDRESS_MASK & MASK_TOP_BYTE)
}

/// The global translation table for the kernel address space
pub static KERNEL_TABLE: SpinLock<TranslationTable> = SpinLock::new(TranslationTable(
    [const { PageDescriptor(InMemoryRegister::new(0)) }; _],
));

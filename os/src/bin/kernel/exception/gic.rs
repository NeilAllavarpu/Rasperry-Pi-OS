use core::ptr;

pub const GICD_START: usize = 0xFFFF_FFFF_FE64_1000;
pub const GICC_START: usize = 0xFFFF_FFFF_FE64_2000;

pub fn init() {
    unsafe {
        ptr::write_volatile((GICD_START + 0) as *mut u32, 0b11); // gicd_ctlr

        ptr::write_volatile((GICD_START + 0x100) as *mut u32, 0xFFFF_FFFF); // gicd_isenable

        ptr::write_volatile((GICC_START + 0) as *mut u32, 0b11); // gicc_ctlr
    }
}

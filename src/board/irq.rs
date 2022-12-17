use crate::{
    architecture::{self, machine::core_id},
    board::Mmio,
};
use core::ops::Deref;
use tock_registers::{
    interfaces::{Readable, Writeable},
    register_bitfields, register_structs,
    registers::{ReadOnly, WriteOnly},
};

register_bitfields! {u32,
    TIMER_CONTROL [
        CNT_PNS_IRQ OFFSET(1) NUMBITS(1) [],
    ],
    INTERRUPT_SOURCE [
        CORE_IRQ OFFSET(8) NUMBITS(1) [],
        CNTVIRQ OFFSET(3) NUMBITS(1) [],
        CNTHPIRQ OFFSET(2) NUMBITS(1) [],
        CNT_PNS_IRQ OFFSET(1) NUMBITS(1) [],
        CNTPSIRQ OFFSET(0) NUMBITS(1) [],
    ],
    PENDING [
        /// This bit is the logical OR of all the interrupt pending bits for interrupts 63 to 32. If set, read the PENDING1 register to determine which interrupts are pending from this set.
        INT63_32 OFFSET(25) NUMBITS(1) [],
        /// This bit is the logical OR of all the interrupt pending bits for interrupts 31 to 0. If set, read the PENDING0 register to determine which interrupts are pending from this set.
        INT31_0 OFFSET(24) NUMBITS(1) []
    ]
}

register_structs! {
    #[allow(non_snake_case)]
    Local_IRQ_Register_Block {
        (0x00 => _reserved),
        (0x60 => CORE0_INTERRUPT_SOURCE: ReadOnly<u32, INTERRUPT_SOURCE::Register>),
        (0x64 => CORE1_INTERRUPT_SOURCE: ReadOnly<u32, INTERRUPT_SOURCE::Register>),
        (0x68 => CORE2_INTERRUPT_SOURCE: ReadOnly<u32, INTERRUPT_SOURCE::Register>),
        (0x6C => CORE3_INTERRUPT_SOURCE: ReadOnly<u32, INTERRUPT_SOURCE::Register>),
        (0x70 => @END),
    }
}

register_structs! {
    #[allow(non_snake_case)]
    Local_IRQ_Register_Block_Init {
        (0x00 => _reserved),
        (0x40 => CORE0_TIMER_INTERRUPT_CONTROL: WriteOnly<u32, TIMER_CONTROL::Register>),
        (0x44 => CORE1_TIMER_INTERRUPT_CONTROL: WriteOnly<u32, TIMER_CONTROL::Register>),
        (0x48 => CORE2_TIMER_INTERRUPT_CONTROL: WriteOnly<u32, TIMER_CONTROL::Register>),
        (0x4C => CORE3_TIMER_INTERRUPT_CONTROL: WriteOnly<u32, TIMER_CONTROL::Register>),
        (0x50 => @END),
    }
}

register_structs! {
    #[allow(non_snake_case)]
    Peripheral_Register_Block {
        (0x00 => PENDING0: ReadOnly<u32>),
        (0x04 => PENDING1: ReadOnly<u32>),
        (0x08 => PENDING2: ReadOnly<u32, PENDING::Register>),
        (0x0C => _reserved1),
        (0x10 => ENABLE0: WriteOnly<u32>),
        (0x14 => ENABLE1: WriteOnly<u32>),
        (0x18 => @END),
    }
}

/// Source: <https://datasheets.raspberrypi.com/bcm2836/bcm2836-peripherals.pdf>
#[allow(clippy::as_conversions)]
const LOCAL_REGISTERS_ADDRESS: *mut Local_IRQ_Register_Block =
    0x4000_0000 as *mut Local_IRQ_Register_Block;
/// Source: <https://datasheets.raspberrypi.com/bcm2836/bcm2836-peripherals.pdf>
#[allow(clippy::as_conversions)]
const PERIPH_REGISTERS_ADDRESS: *mut Peripheral_Register_Block =
    0x3F00_B200 as *mut Peripheral_Register_Block;

/// Wrapper for the memory-mapped IRQ registers
struct Registers<T> {
    /// The actual registers
    registers: Mmio<T>,
}

impl<T> Registers<T> {
    /// Instantiates the memory-mapped registers
    /// # Safety
    /// Should only be called once
    const unsafe fn new(start_addr: *mut T) -> Self {
        Self {
            // SAFETY: This should only be used once
            registers: unsafe { Mmio::new(start_addr) },
        }
    }
}

impl<T> Deref for Registers<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.registers
    }
}

// SAFETY: These are per-core entries, so it should be safe since we have mutual exclusion
unsafe impl<T> Send for Registers<T> {}
// SAFETY: These are per-core entries, so it should be safe since we have mutual exclusion
unsafe impl<T> Sync for Registers<T> {}

/// The memory mapped IRQ-related registers
#[allow(clippy::undocumented_unsafe_blocks)]
static IRQ_REGISTERS: Registers<Local_IRQ_Register_Block> =
    unsafe { Registers::new(LOCAL_REGISTERS_ADDRESS) };
/// The memory mapped IRQ-related registers for peripherals
static PERIPHERAL_REGISTERS: Registers<Peripheral_Register_Block> =
    // SAFETY: These registers are only ever used during the initialization process
    unsafe { Registers::new(PERIPH_REGISTERS_ADDRESS) };

/// Dispatches an IRQ to the correct handler
#[allow(clippy::module_name_repetitions)]
pub fn handle_irq() {
    match core_id() {
        0 => handle_core_irq(&IRQ_REGISTERS.CORE0_INTERRUPT_SOURCE),
        1 => handle_core_irq(&IRQ_REGISTERS.CORE1_INTERRUPT_SOURCE),
        2 => handle_core_irq(&IRQ_REGISTERS.CORE2_INTERRUPT_SOURCE),
        3 => handle_core_irq(&IRQ_REGISTERS.CORE3_INTERRUPT_SOURCE),
        _ => unreachable!(),
    }
}

/// Exception handlers for VideoCore IRQs
static VIDEOCORE_IRQ_HANDLERS: phf::Map<u32, fn() -> ()> = phf::phf_map! {
    57_u32 => crate::board::uart::handle_interrupt
};

/// The main IRQ handler
fn handle_core_irq(interrupt_source: &ReadOnly<u32, INTERRUPT_SOURCE::Register>) {
    if interrupt_source.matches_any(INTERRUPT_SOURCE::CNT_PNS_IRQ::SET) {
        // Timer interrupt detected
        architecture::time::handle_irq();
        // Interrupt is handled
    } else if interrupt_source.matches_any(INTERRUPT_SOURCE::CORE_IRQ::SET) {
        assert!(core_id() == 0);
        // Videocore interrupt, figure out the range
        let pending2 = PERIPHERAL_REGISTERS.PENDING2.extract();
        assert!(pending2.matches_any(PENDING::INT63_32::SET + PENDING::INT31_0::SET));
        // TODO: Fix IRQ detection
        if pending2.matches_any(PENDING::INT31_0::SET) {
            let mut pending = PERIPHERAL_REGISTERS.PENDING0.get();
            assert_ne!(pending, 0);
            while pending != 0 {
                let irq = pending.trailing_zeros();
                if let Some(handler) = VIDEOCORE_IRQ_HANDLERS.get(&(irq)) {
                    handler.call(());
                } else {
                    panic!("WARNING: Ignoring IRQ {}", irq);
                }
                pending &= !(1 << irq);
            }
        }
        if pending2.matches_any(PENDING::INT63_32::SET) {
            // let mut pending = PERIPHERAL_REGISTERS.PENDING1.get();
            // assert_ne!(pending, 0);
            // while pending != 0 {
            //     let irq = pending.trailing_zeros();
            //     if let Some(handler) = VIDEOCORE_IRQ_HANDLERS.get(&(irq + 32)) {
            //         handler.call(());
            //     } else {
            //         panic!("WARNING: Ignoring IRQ {}", irq + 32);
            //     }
            //     pending &= !(1 << irq);
            // }
            VIDEOCORE_IRQ_HANDLERS.get(&57).unwrap().call(());
        }
    } else {
        panic!("Unhandled IRQ");
    }
}

/// Enables IRQs (timer, UART)
pub fn init() {
    let control_registers =
        // SAFETY: These registers are only ever used during the initialization process
        unsafe { Mmio::<Local_IRQ_Register_Block_Init>::new(LOCAL_REGISTERS_ADDRESS.cast()) };

    // Enable timer interrupts for all cores
    control_registers
        .CORE0_TIMER_INTERRUPT_CONTROL
        .write(TIMER_CONTROL::CNT_PNS_IRQ::SET);
    control_registers
        .CORE1_TIMER_INTERRUPT_CONTROL
        .write(TIMER_CONTROL::CNT_PNS_IRQ::SET);
    control_registers
        .CORE2_TIMER_INTERRUPT_CONTROL
        .write(TIMER_CONTROL::CNT_PNS_IRQ::SET);
    control_registers
        .CORE3_TIMER_INTERRUPT_CONTROL
        .write(TIMER_CONTROL::CNT_PNS_IRQ::SET);

    for interrupt in VIDEOCORE_IRQ_HANDLERS.keys() {
        if interrupt >= &32 {
            PERIPHERAL_REGISTERS.ENABLE1.set(1 << (interrupt - 32));
        } else {
            PERIPHERAL_REGISTERS.ENABLE0.set(1 << interrupt);
        }
    }
}

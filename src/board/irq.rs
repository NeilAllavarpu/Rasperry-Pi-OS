use crate::{
    architecture::{machine::core_id, time::frequency},
    board::Mmio,
    kernel, log,
};
use aarch64_cpu::registers::{CNTP_CTL_EL0, CNTP_CVAL_EL0};
use core::ops::Deref;
use tock_registers::{
    interfaces::{ReadWriteable, Readable, Writeable},
    register_bitfields, register_structs,
    registers::{ReadOnly, WriteOnly},
};

register_bitfields! {u32,
    TIMER_CONTROL [
        nCNTPNSIRQ OFFSET(1) NUMBITS(1) [],
    ],
    INTERRUPT_SOURCE [
        CNTVIRQ OFFSET(3) NUMBITS(1) [],
        CNTHPIRQ OFFSET(2) NUMBITS(1) [],
        CNTPNSIRQ OFFSET(1) NUMBITS(1) [],
        CNTPSIRQ OFFSET(0) NUMBITS(1) [],
    ]
}

register_structs! {
    #[allow(non_snake_case)]
    IRQ_Register_Block {
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
    IRQ_Register_Block_Init {
        (0x00 => _reserved),
        (0x40 => CORE0_TIMER_INTERRUPT_CONTROL: WriteOnly<u32, TIMER_CONTROL::Register>),
        (0x44 => CORE1_TIMER_INTERRUPT_CONTROL: WriteOnly<u32, TIMER_CONTROL::Register>),
        (0x48 => CORE2_TIMER_INTERRUPT_CONTROL: WriteOnly<u32, TIMER_CONTROL::Register>),
        (0x4C => CORE3_TIMER_INTERRUPT_CONTROL: WriteOnly<u32, TIMER_CONTROL::Register>),
        (0x50 => @END),
    }
}

/// Source: <https://datasheets.raspberrypi.com/bcm2836/bcm2836-peripherals.pdf>
#[allow(clippy::as_conversions)]
const REGISTERS_ADDRESS: *mut IRQ_Register_Block = 0x4000_0000 as *mut IRQ_Register_Block;

/// Wrapper for the memory-mapped IRQ registers
struct IrqRegisters {
    /// The actual registers
    registers: Mmio<IRQ_Register_Block>,
}

impl IrqRegisters {
    /// Instantiates the memory-mapped registers
    /// # Safety
    /// Should only be called once
    const unsafe fn new() -> Self {
        Self {
            // SAFETY: This should only be used once
            registers: unsafe { Mmio::new(REGISTERS_ADDRESS) },
        }
    }
}

impl Deref for IrqRegisters {
    type Target = IRQ_Register_Block;

    fn deref(&self) -> &Self::Target {
        &self.registers
    }
}

// SAFETY: These are per-core entries, so it should be safe since we have mutual exclusion
unsafe impl Send for IrqRegisters {}
// SAFETY: These are per-core entries, so it should be safe since we have mutual exclusion
unsafe impl Sync for IrqRegisters {}

/// The memory mapped IRQ-related registers
#[allow(clippy::undocumented_unsafe_blocks)]
static IRQ_REGISTERS: IrqRegisters = unsafe { IrqRegisters::new() };

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

/// The main IRQ handler
fn handle_core_irq(interrupt_source: &ReadOnly<u32, INTERRUPT_SOURCE::Register>) {
    if interrupt_source.matches_any(INTERRUPT_SOURCE::CNTPNSIRQ::SET) {
        // Timer interrupt detected
        kernel::exception::handle_timer();
        // Interrupt is handled
        CNTP_CTL_EL0.modify(CNTP_CTL_EL0::ENABLE::CLEAR);
    } else {
        log!("big sad");
    }
}

/// Enables IRQs from the peripherals
pub fn init() {
    let control_registers =
        // SAFETY: These registers are only ever used during the initialization process
        unsafe { Mmio::<IRQ_Register_Block_Init>::new(REGISTERS_ADDRESS.cast()) };

    // Enable timer interrupts for all cores
    control_registers
        .CORE0_TIMER_INTERRUPT_CONTROL
        .write(TIMER_CONTROL::nCNTPNSIRQ::SET);
    control_registers
        .CORE1_TIMER_INTERRUPT_CONTROL
        .write(TIMER_CONTROL::nCNTPNSIRQ::SET);
    control_registers
        .CORE2_TIMER_INTERRUPT_CONTROL
        .write(TIMER_CONTROL::nCNTPNSIRQ::SET);
    control_registers
        .CORE3_TIMER_INTERRUPT_CONTROL
        .write(TIMER_CONTROL::nCNTPNSIRQ::SET);
}

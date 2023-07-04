//! Driver for DMA controllers. Currently only supports the DMA Lite engines (as higher performance
//! is not necessary for the paced transfers of the UART)

use bitfield_struct::bitfield;
use core::arch::aarch64::__dmb;
use core::arch::aarch64::OSHST;
use core::mem::MaybeUninit;
use core::num::NonZeroUsize;
use core::ptr::{self, NonNull};
use num_derive::FromPrimitive;
use num_derive::ToPrimitive;
use tock_registers::interfaces::ReadWriteable;
use tock_registers::interfaces::Readable;
use tock_registers::interfaces::Writeable;
use tock_registers::registers::ReadOnly;
use tock_registers::registers::ReadWrite;
use tock_registers::{register_bitfields, register_structs};

register_bitfields! {
    u32,
    /// DMA Control and Status register contains the main control and status bits for this DMA
    /// channel.
    CS [
        /// DMA Channel Reset
        ///
        /// The bit cannot be read, and will self clear.
        RESET OFFSET(31) NUMBITS(1) [
            /// Reset the DMA
            Reset = 0b1,
        ],
        /// Abort DMA
        ///
        /// The bit cannot be read, and will self clear.
        ABORT OFFSET(30) NUMBITS(1) [
            /// Abort the current DMA CB. The DMA will load the next CB and attempt to continue.
            AbortCurrent = 0b1,
        ],
        /// Disable debug pause signal
        DISDEBUG OFFSET(29) NUMBITS(1) [
            PauseForDebug = 0b0,
            /// The DMA will not stop when the debug pause signal is asserted.
            IgnoreDebug = 0b1,
        ],
        /// Wait for outsnading writes
        WAIT_FOR_OUTSTANDING_WRITES OFFSET(28) NUMBITS(1) [
            NoPause = 0b0,
            /// When set to 1, the DMA will keep a tally of the AXI writes going out and the write
            /// responses coming in. At the very end of the current DMA transfer it will wait until
            /// the last outstanding write response has been received before indicating the
            /// transfer is complete. Whilst waiting it will load the next CB address (but will not
            /// fetch the CB), clear the active flag (if the next CB address = zero), and it will
            /// defer setting the END flag or the INT flag until the last outstanding write
            /// response has been received. In this mode, the DMA will pause if it has more than 13
            /// outstanding writes at any one time.
            PauseForOustandingWrites = 0b1,
        ],
        /// AXI Panic Priority Level
        ///
        /// Sets the priority of panicking AXI bus transactions. This value is used when the panic
        /// bit of the selected peripheral channel is 1.
        ///
        /// Zero is the lowest priority.
        PANIC_PRIORITY OFFSET(20) NUMBITS(4) [],
        /// AXI Priority Level
        ///
        /// Sets the priority of normal AXI bus transactions. This value is used when the panic bit
        /// of the selected peripheral channel is zero.
        ///
        /// Zero is the lowest priority.
        PRIORITY OFFSET(16) NUMBITS(4) [],
        /// DMA Error
        ///
        /// Indicates if the DMA has detected an error. The error flags are available in the debug
        /// register, and have to be cleared by writing to that register.
        ERROR OFFSET(8) NUMBITS(1) [
            /// DMA channel is OK
            NoError = 0b0,
            /// DMA channel has an error flag set
            Error = 0b1,
        ],
        /// DMA is Waiting for the Last Write to be Received
        ///
        /// Indicates if the DMA is currently waiting for any outstanding writes to be received,
        /// and is not transferring data.
        WAITING_FOR_OUTSTANDING_WRITES OFFSET(6) NUMBITS(1) [
            NotWaiting = 0b0,
            /// DMA channel is waiting
            Waiting = 0b1,
        ],
        /// DMA Paused by DREQ State
        ///
        /// Indicates if the DMA is currently paused and not transferring data due to the `DREQ`
        /// being inactive.
        DREQ_STOPS_DMA OFFSET(5) NUMBITS(1) [
            /// DMA channel is running
            Running = 0b0,
            /// DMA channel is paused
            Paused = 0b1,
        ],
        /// DMA Paused State
        ///
        /// Indicates if the DMA is currently paused and not transferring data. This will occur
        /// if:
        /// * the active bit has been cleared
        /// * the DMA is currently executing wait cycles
        /// * the debug_pause signal has been set by the debug block
        /// * or, the number of outstanding writes has exceeded the max count.
        PAUSED OFFSET(4) NUMBITS(1) [
            /// DMA channel is running
            Running = 0b0,
            /// DMA channel is paused
            Paused = 0b1,
        ],
        /// DREQ State
        ///
        /// Indicates the state of the selected DREQ (Data Request) signal, i.e. the DREQ selected
        /// by the `PERMAP` field of the transfer info.
        ///
        /// If `PERMAP` is set to zero (un-paced transfer) then this bit will read back as 1.
        DREQ OFFSET(3) NUMBITS(1) [
            /// No data request
            NoRequest = 0b0,
            /// Requesting data. This will only be valid once the DMA has started and the `PERMAP`
            /// field has been loaded from the CB. It will remain valid, indicating the selected
            /// `DREQ` signal, until a new CB is loaded.
            Requesting = 0b1,
        ],
        /// Interrupt Status
        ///
        /// This is set when the transfer for the CB ends and `INTEN` is set to 1. Once set it must
        /// be manually cleared down, even if the next CB has `INTEN` = 0.
        INT OFFSET(2) NUMBITS(1) [
            NoInterrupt = 0b0,
            /// Write to clear
            Interrupt = 0b1,
        ],
        /// DMA End Flag
        ///
        /// Set when the transfer described by the current Control Block is complete.
        END OFFSET(1) NUMBITS(1) [
            InProgress = 0b0,
            /// Write to clear
            End = 0b1,
        ],
        /// Activate the DMA
        ///
        /// This bit enables the DMA. The DMA will start if this bit is set and the CB_ADDR is non
        /// zero. The DMA transfer can be paused and resumed by clearing, then setting it again.
        ///
        /// This bit is automatically cleared at the end of the complete DMA transfer, i.e. after a
        /// `NEXTCONBK = 0x0000_0000` has been loaded.
        ACTIVE OFFSET(0) NUMBITS(1) [
            Idle = 0b0,
            Active = 0b1,
        ]
    ],
    /// DMA Control Block Address register.
    CONBLK_AD [
        /// Control Block Address
        ///
        /// This tells the DMA where to find a Control Block stored in memory. When the `ACTIVE`
        /// bit is set and this address is non zero, the DMA will begin its transfer by loading the
        /// contents of the addressed CB into the relevant DMA channel registers. At the end of the
        /// transfer this register will be updated with the `ADDR` field of the `NEXTCONBK` Control
        /// Block register. If this field is zero, the DMA will stop. Reading this register will
        /// return the address of the currently active CB (in the linked list of CBs). The address
        /// must be 256-bit aligned, so the bottom 5 bits of the address must be zero.
        SCB_ADDR OFFSET(0) NUMBITS(31) []
    ],
    CS4 [
        HALT OFFSET(31) NUMBITS(1) [
            Halt = 0b1,
        ],
        ABORT OFFSET(30) NUMBITS(1) [
            Abort = 0b1,
        ],

    ],
    /// Global enable bits for each channel.
    ///
    /// Setting these to 0 will disable the DMA for power saving reasons. Disabling whilst the DMA
    /// is operating will be fatal.
    ENABLE [
        /// Set the 1G SDRAM ram page that the DMA Lite engines (DMA7-10) will access when
        /// addressing the 1G uncached range `C000_0000->ffff_ffff`.
        /// This allows the 1G uncached page to be moved around the 16G SDRAM space
        PAGELITE OFFSET(28) NUMBITS(4) [],
        /// Set the 1G SDRAM ram page that the 30-bit DMA engines (DMA0-6) will access when
        /// addressing the 1G uncached range `C000_0000->ffff_ffff`.
        PAGE OFFSET(24) NUMBITS(4) [],
        /// This allows the 1G uncached page to be moved around the 16G SDRAM space.
        /// Enable DMA engines 0-14
        EN OFFSET(0) NUMBITS(14) []
    ]

}

register_structs! {
    pub Registers {
        (0x000 => cs: ReadWrite<u32, CS::Register>),
        (0x004 => conblk_ad: ReadWrite<u32, CONBLK_AD::Register>),
        (0x008 => _unused0),
        (0xFE0 => int_status: ReadOnly<u32>),
        (0xFE4 => _unused1),
        (0xFF0 => enable: ReadWrite<u32, ENABLE::Register>),
        (0xFF4 => @END),
    }
}

/// Peripherals to use with the DMA engines
#[derive(FromPrimitive, ToPrimitive, Debug)]
pub enum Peripheral {
    Unpaced = 0,
    Uart0Tx = 12,
    Uart0Rx = 14,
}

#[allow(clippy::missing_docs_in_private_items)]
impl Peripheral {
    #[expect(
        clippy::as_conversions,
        reason = "Simplest way to const-convert to primitves"
    )]
    const fn into_bits(self) -> u32 {
        self as u32
    }

    const fn from_bits(value: u32) -> Self {
        match value {
            0 => Self::Unpaced,
            12 => Self::Uart0Tx,
            14 => Self::Uart0Rx,
            _ =>
            #[expect(clippy::unreachable, reason = "No other values should be used here")]
            {
                unreachable!()
            }
        }
    }
}

#[bitfield(u32, debug = true)]
struct TransferInfo {
    /// Interrupt Enable
    /// * 1 = Generate an interrupt when the transfer described by the current Control Block
    /// completes.
    /// * 0 = Do not generate an interrupt.
    inten: bool,
    tdmode: bool,
    _res: bool,
    /// Wait for a Write Response
    ///
    /// When set this makes the DMA wait until it receives the AXI write response for each write.
    /// This ensures that multiple writes cannot get stacked in the AXI bus pipeline.
    /// * 1 = Wait for the write response to be received before proceeding.
    /// * 0 = Dont wait; continue as soon as the write data is sent.
    wait_resp: bool,
    /// Destination Address Increment
    /// * 1 = Destination address increments after each write. The address will increment by 4, if
    /// `DEST_WIDTH = 0` else by 32.
    /// * 0 = Destination address does not change.
    dest_inc: bool,
    /// Destination Transfer Width
    /// * 1 = Use 128-bit destination write width.
    /// * 0 = Use 32-bit destination write width.
    dest_width: bool,
    /// Control Destination Writes with DREQ
    /// * 1 = The DREQ selected by PERMAP will gate the destination writes.
    /// * 0 = DREQ has no effect.
    dest_dreq: bool,
    /// Ignore Writes
    /// * 1 = Do not perform destination writes.
    /// * 0 = Write data to destination.
    dest_ignore: bool,
    /// Source Address Increment
    /// * 1 = Source address increments after each read. The address will increment by 4, if
    /// `SRC_WIDTH = 0` else by 32.
    /// * 0 = Source address does not change.
    src_inc: bool,
    /// Source Transfer Width
    /// * 1 = Use 128-bit source read width.
    /// * 0 = Use 32-bit source read width.
    src_width: bool,
    /// Control Source Reads with DREQ
    /// * 1 = The DREQ selected by `PERMA`P will gate the source reads.
    /// * 0 = DREQ has no effect.
    scr_dreq: bool,
    /// Ignore Reads
    /// * 1 = Do not perform source reads. In addition, destination writes will zero all the write
    /// strobes. This is used for fast cache fill operations.
    /// * 0 = Perform source reads.
    src_ignore: bool,
    /// Burst Transfer Length
    ///
    /// Indicates the burst length of the DMA transfers. The DMA will attempt to transfer data as
    /// bursts of this number of words. A value of zero will produce a single transfer. Bursts are
    /// only produced for specific conditions, see main text.
    #[bits(4)]
    burst_length: u8,
    /// Peripheral Mapping
    ///
    /// Indicates the peripheral whose ready signal shall be used to control the rate of the
    /// transfers, and whose panic signals will be output on the DMA AXI bus.
    #[bits(5)]
    permap: Peripheral,
    /// Add Wait Cycles
    ///
    /// This slows down the DMA throughput by setting the number of dummy cycles burnt after each
    /// DMA read or write operation is completed. A value of 0 means that no wait cycles are to be
    /// added.
    #[bits(5)]
    waits: u8,
    /// Don’t do wide writes as a 2 beat burst
    ///
    /// This prevents the DMA from issuing wide writes as 2 beat AXI bursts. This is an inefficient
    /// access mode, so the default is to use the bursts.
    no_wide_bursts: bool,
    #[bits(5)]
    _res1: u8,
}

/// Control blocks for the DMA/DMA Lite engines
// Control Blocks (CB) are 8 words (256 bits) in length and must start at a 256-bit aligned address
#[repr(C, align(32))]
#[derive(Debug)]
struct DmaControlBlock {
    /// Transfer information for the DMA operation
    transfer_info: TransferInfo,
    /// DMA Source Address
    ///
    /// Source address for the DMA operation. Updated by the DMA engine as the transfer progresses.
    src_addr: u32,
    /// DMA Destination Address
    ///
    /// Destination address for the DMA operation. Updated by the DMA engine as the transfer
    /// progresses.
    dest_addr: u32,
    /// DMA Transfer Length. This specifies the amount of data to be transferred in bytes.
    ///
    /// In normal (non 2D) mode this specifies the amount of bytes to be transferred.
    ///
    /// The length register is updated by the DMA engine as the transfer progresses, so it will
    /// indicate the data left to transfer.
    transfer_len: u32,
    /// 2D mode stride, reserved on DMA lites
    stride: u32,
    /// DMA Next Control Block Address
    ///
    /// The value loaded into this register can be overwritten so that the linked list of Control
    /// Block data structures can be altered. However it is only safe to do this when the DMA is
    /// paused. The address must be 256-bit aligned and so the bottom 5 bits cannot be set and will
    /// read back as zero.
    next_block_addr: u32,
    #[expect(clippy::missing_docs_in_private_items, reason = "Unused field")]
    _res: u32,
    #[expect(clippy::missing_docs_in_private_items, reason = "Unused field")]
    _res2: u32,
}

#[bitfield(u32, debug = true)]
struct TransferInfo4 {
    /// Interrupt Enable
    /// * 1 = Generate an interrupt when the transfer described by the current Control Block
    /// completes.
    /// * 0 = Do not generate an interrupt.
    inten: bool,
    tdmode: bool,
    /// Wait for a Write Response
    ///
    /// When set this makes the DMA wait until it receives the AXI write response for each write.
    /// This ensures that multiple writes cannot get stacked in the AXI bus pipeline.
    /// * 1 = Wait for the write response to be received before proceeding.
    /// * 0 = Don’t wait; continue as soon as the write data is sent.
    wait_resp: bool,
    wait_rd_resp: bool,
    #[bits(5)]
    _res: u8,
    /// Peripheral Mapping
    ///
    /// Indicates the peripheral whose ready signal shall be used to control the rate of the
    /// transfers, and whose panic signals will be output on the DMA AXI bus.
    #[bits(5)]
    permap: Peripheral,
    /// Control Source Reads with DREQ
    /// * 1 = The DREQ selected by `PERMA`P will gate the source reads.
    /// * 0 = DREQ has no effect.
    src_dreq: bool,
    /// Control Destination Writes with DREQ
    /// * 1 = The DREQ selected by PERMAP will gate the destination writes.
    /// * 0 = DREQ has no effect.
    dest_dreq: bool,
    /// Add Wait Cycles
    ///
    /// This slows down the DMA throughput by setting the number of dummy cycles burnt after each
    /// DMA read or write operation is completed. A value of 0 means that no wait cycles are to be
    /// added.
    s_waits: u8,
    /// Add Wait Cycles
    ///
    /// This slows down the DMA throughput by setting the number of dummy cycles burnt after each
    /// DMA read or write operation is completed. A value of 0 means that no wait cycles are to be
    /// added.
    d_waits: u8,
}

#[bitfield(u32)]
struct SrcDestInfo {
    addr: u8,
    #[bits(4)]
    burst_len: u8,
    inc: bool,
    #[bits(2)]
    size: u8,
    ignore: bool,
    stride: u16,
}

/// Control blocks for the DMA4 engines
// Control Blocks (CB) are 8 words (256 bits) in length and must start at a 256-bit aligned address
#[repr(C, align(32))]
#[derive(Debug)]
struct Dma4ControlBlock {
    /// Transfer information for the DMA operation
    transfer_info: TransferInfo4,
    /// DMA Source Address
    ///
    /// Source address for the DMA operation. Updated by the DMA engine as the transfer progresses.
    src_addr: u32,
    /// Additional information for the source
    src_info: SrcDestInfo,
    /// DMA Destination Address
    ///
    /// Destination address for the DMA operation. Updated by the DMA engine as the transfer
    /// progresses.
    dest_addr: u32,
    /// Additional information for the destination
    dest_info: SrcDestInfo,
    /// DMA Transfer Length. This specifies the amount of data to be transferred in bytes.
    ///
    /// In normal (non 2D) mode this specifies the amount of bytes to be transferred.
    ///
    /// The length register is updated by the DMA engine as the transfer progresses, so it will
    /// indicate the data left to transfer.
    transfer_len: u32,
    /// DMA Next Control Block Address
    ///
    /// The value loaded into this register can be overwritten so that the linked list of Control
    /// Block data structures can be altered. However it is only safe to do this when the DMA is
    /// paused. The address must be 256-bit aligned and so the bottom 5 bits cannot be set and will
    /// read back as zero.
    next_block_addr: u32,
    #[expect(clippy::missing_docs_in_private_items, reason = "Unused field")]
    _reserved: u32,
}

/// A driver for (normal) DMA engines
pub struct Dma<'dma> {
    /// The memory-mapped DMA registers
    registers: &'dma mut Registers,
}

#[allow(dead_code)]
impl Dma<'_> {
    /// Creates a wrapper for a memory-mapped mailbox interface at the given base register address.
    ///
    /// Returns `None` if the pointer is not suitably aligned
    ///
    /// # Safety
    /// * The address must point to a valid memory-mapped mailbox register set
    /// * The mailbox registers must be valid for at least as long as this wrapper exists
    /// * The mailbox registers must not be accessed in any other way while this wrapper exists
    #[expect(clippy::arithmetic_side_effects, reason = "No side effects")]
    pub unsafe fn new(base_address: NonZeroUsize) -> Option<Self> {
        let mut registers: NonNull<Registers> =
            NonNull::new(ptr::from_exposed_addr_mut(base_address.get()))?;

        if !registers.as_ptr().is_aligned() {
            return None;
        }

        // SAFETY: The caller upholds the conditions necessary for exclusivity and accessing,
        // and we have verified alignment

        let registers = unsafe { registers.as_mut() };
        let mut prev = registers.enable.extract();
        let mut prev_en = prev.read(ENABLE::EN);
        prev_en |= (1 << 0xB) | 1;
        prev.modify(ENABLE::PAGELITE.val(0) + ENABLE::EN.val(prev_en));
        registers.enable.set(prev.get());
        registers
            .cs
            .write(CS::RESET::Reset + CS::WAIT_FOR_OUTSTANDING_WRITES::NoPause);

        Some(Self { registers })
    }

    /// Reads a peripheral. Returns false if an error occurs
    pub fn read_peripheral(
        &mut self,
        peripheral: Peripheral,
        peripheral_addr: u32,
        dest: &mut [MaybeUninit<u8>],
    ) -> bool {
        /// Pages are 1GB for the DMA engines
        const PAGE_SHIFT: usize = 30;
        /// Mask for DMA pages
        const PAGE_MASK: usize = (1 << PAGE_SHIFT) - 1;
        /// Mask to convert to bus addresses
        const BUS_MASK: u32 = 0xC000_0000;
        let mut cb = MaybeUninit::uninit();
        let dest_addr = dest.as_mut_ptr_range();
        let page = dest_addr.start.addr() >> PAGE_SHIFT;
        if page != dest_addr.end.addr() >> PAGE_SHIFT {
            return false;
        }
        self.registers.enable.modify(ENABLE::PAGE.val(
            #[expect(clippy::unwrap_used, reason = "This conversion should never fail")]
            page.try_into().unwrap(),
        ));
        // SAFETY: The pointer is properly obtained from `cb`
        unsafe {
            ptr::from_mut(&mut cb).write_volatile(MaybeUninit::new(DmaControlBlock {
                transfer_info: TransferInfo::new()
                    .with_inten(true)
                    .with_tdmode(false)
                    .with_wait_resp(true)
                    .with_dest_inc(true)
                    .with_dest_width(false)
                    .with_dest_dreq(true)
                    .with_dest_ignore(false)
                    .with_src_inc(false)
                    .with_src_width(false)
                    .with_scr_dreq(true)
                    .with_src_ignore(false)
                    .with_burst_length(0)
                    .with_permap(peripheral)
                    .with_waits(u8::MAX),
                src_addr: peripheral_addr,
                dest_addr: #[expect(
                    clippy::unwrap_used,
                    reason = "This conversion should never fail"
                )]
                u32::try_from(ptr::from_mut(dest).mask(PAGE_MASK).addr()).unwrap()
                    | BUS_MASK,
                transfer_len: #[expect(
                    clippy::unwrap_used,
                    reason = "This conversion should never fail"
                )]
                dest.len().try_into().unwrap(),
                next_block_addr: 0,
                stride: 0,
                _res: 0,
                _res2: 0,
            }));
        };
        // SAFETY: This only runs for `aarch64`
        unsafe {
            __dmb(OSHST);
        };
        self.registers.conblk_ad.set(
            #[expect(clippy::unwrap_used, reason = "This conversion should never fail")]
            u32::try_from(ptr::from_ref(&cb).addr()).unwrap()
                | 0xC000_0000,
        );
        self.registers.cs.modify(CS::ACTIVE::Active);
        while self.registers.cs.matches_any(CS::ACTIVE::Active) {
            core::hint::spin_loop();
        }
        self.registers.cs.modify(CS::INT::Interrupt);
        true
    }
}

use tock_registers::{
    fields::FieldValue,
    interfaces::{ReadWriteable, Readable, Writeable},
    register_bitfields, register_structs,
    registers::ReadWrite,
    LocalRegisterCopy,
};

use bitfield_struct::bitfield;
use core::{arch::asm, marker::PhantomData, ops, time::Duration};
use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::{FromPrimitive, ToPrimitive};

use crate::println;
//--------------------------------------------------------------------------------------------------
// Public Definitions
//--------------------------------------------------------------------------------------------------

pub struct MMIODerefWrapper<T> {
    start_addr: usize,
    phantom: PhantomData<T>,
}

//--------------------------------------------------------------------------------------------------
// Public Code
//--------------------------------------------------------------------------------------------------

impl<T> MMIODerefWrapper<T> {
    /// Create an instance.
    #[must_use]
    pub const unsafe fn new(start_addr: usize) -> Self {
        Self {
            start_addr,
            phantom: PhantomData,
        }
    }
}

impl<T> ops::Deref for MMIODerefWrapper<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*(self.start_addr as *const _) }
    }
}

register_bitfields![u32,
    BLKSIZECNT [
        BLKCNT OFFSET(16) NUMBITS(16),
        BLKSIZE OFFSET(0) NUMBITS(10)
    ],
    CMDTM [
        CMD_INDEX OFFSET(24) NUMBITS(6) [
            GoIdleState = 0,
            AllSendCid = 2,
            SendRelativeAddr = 3,
            SelectCard = 7,
            SendIFCond = 8,
            SendCSD = 9,
            SetBlocklen = 16,
            ReadSingleBlock = 17,
            SDSendOpCond = 41,
            AppCmd = 55,
        ],
        CMD_TYPE OFFSET(22) NUMBITS(2) [
            Normal = 0b00,
            Suspend = 0b01,
            Resume = 0b10,
            Abort = 0b11,
        ],
        CMD_ISDATA OFFSET(21) NUMBITS(1) [],
        CMD_IXCHK_EN OFFSET(20) NUMBITS(1) [],
        CMD_CRCCHK_EN OFFSET(19) NUMBITS(1) [],
        CMD_RSPNS_TYPE OFFSET(16) NUMBITS(2) [
            NoResponse = 0b00,
            Bits136 = 0b01,
            Bits48 = 0b10,
            Bits48Busy = 0b11,
        ],
        TM_MULTI_BLOCK OFFSET(5) NUMBITS(1) [],
        TM_DAT_DIR OFFSET(4) NUMBITS(1) [
            HostToCard = 0b0,
            CardToHost = 0b1,
        ],
        TM_AUTO_CMD_EN OFFSET(2) NUMBITS(2) [
            NoCommand = 0b00,
            CMD12 = 0b01,
            CMD23 = 0b10,
        ],
        TM_BLKCNT_EN OFFSET(1) NUMBITS(1) [],
    ],
    STATUS [
        DAT_INHIBIT OFFSET(1) NUMBITS(1),
        CMD_INHIBIT OFFSET(0) NUMBITS(1),
    ],
    CONTROL1 [
        SRST_HC OFFSET(24) NUMBITS(1),
        DATA_TOUNIT OFFSET(16) NUMBITS(4),
        CLK_FREQ8 OFFSET(8) NUMBITS(8),
        CLK_FREQ_MS2 OFFSET(6) NUMBITS(2),
        CLK_EN OFFSET(2) NUMBITS(1),
        CLK_STABLE OFFSET(1) NUMBITS(1),
        CLK_INTLEN OFFSET(0) NUMBITS(1),
    ],
    INTERRUPT [
        DTO_ERR OFFSET(20) NUMBITS(1),
        CTO_ERR OFFSET(12) NUMBITS(1),
        READ_RDY OFFSET(5) NUMBITS(1),
        CMD_DONE OFFSET(0) NUMBITS(1),
    ],
    SLOTISR_VER [
        VENDOR OFFSET(24) NUMBITS(8),
        SDVERSION OFFSET(16) NUMBITS(8),
        SLOT_STATUS OFFSET(0) NUMBITS(8),
    ]
];

register_bitfields![u128,
    CSD [
        READ_BL_LEN OFFSET(72) NUMBITS(4),
        C_SIZE OFFSET(54) NUMBITS(12),
        C_SIZE_MULT OFFSET(39) NUMBITS(3),
    ]
];

register_structs! {
    #[expect(non_snake_case)]
    pub EmmcRegs {
        (0x0 => _reserved1),
        (0x4 => BLKSIZECNT: ReadWrite<u32, BLKSIZECNT::Register>),
        (0x8 => ARG1: ReadWrite<u32>),
        (0xC => CMDTM: ReadWrite<u32, CMDTM::Register>),
        (0x10 => RESP0: ReadWrite<u32>),
        (0x14 => RESP1: ReadWrite<u32>),
        (0x18 => RESP2: ReadWrite<u32>),
        (0x1C => RESP3: ReadWrite<u32>),
        (0x20 => DATA: ReadWrite<u32>),
        (0x24 => STATUS: ReadWrite<u32, STATUS::Register>),
        (0x28 => CONTROL0: ReadWrite<u32>),
        (0x2C => CONTROL1: ReadWrite<u32, CONTROL1::Register>),
        (0x30 => INTERRUPT: ReadWrite<u32, INTERRUPT::Register>),
        (0x34 => IRPT_MASK: ReadWrite<u32, INTERRUPT::Register>),
        (0x38 => IRPT_EN: ReadWrite<u32, INTERRUPT::Register>),
        (0x3C => _reserved3),
        (0xFC => SLOTISR_VER: ReadWrite<u32, SLOTISR_VER::Register>),
        (0x100 => @END),
    }
}

#[derive(FromPrimitive, ToPrimitive, Debug)]
enum SdState {
    Idle = 0,
    Ready = 1,
    Ident = 2,
    Stby = 3,
    Tran = 4,
    Data = 5,
    Rcv = 6,
    Prg = 7,
    Dis = 8,
    Reserved,
    ReservedIoMode = 15,
}

impl From<u32> for SdState {
    fn from(value: u32) -> Self {
        FromPrimitive::from_u32(value).unwrap_or(Self::Reserved)
    }
}

impl From<SdState> for u32 {
    fn from(value: SdState) -> Self {
        ToPrimitive::to_u32(&value).unwrap()
    }
}

#[bitfield(u32)]
struct SdStatus {
    #[bits(2)]
    _reserved_manufacturer_test_mode: u8,
    _resserved_app_specific_command: bool,
    ake_seq_error: bool,
    _resserved_sdio: bool,
    app_cmd: bool,
    fx_event: bool,
    _reserved0: bool,
    ready_for_data: bool,
    #[bits(4)]
    current_state: SdState,
    erase_reset: bool,
    card_ecc_disabled: bool,
    wp_erase_skip: bool,
    csd_overwrite: bool,
    _reserved_deferred_response: bool,
    _reserved1: bool,
    error: bool,
    cc_error: bool,
    card_ecc_failed: bool,
    illegal_command: bool,
    com_crc_error: bool,
    lock_unlock_failed: bool,
    card_is_locked: bool,
    wp_violation: bool,
    erase_param: bool,
    erase_seq_error: bool,
    block_len_error: bool,
    address_error: bool,
    out_of_range: bool,
}

fn tick() -> u64 {
    let tick: u64;
    unsafe {
        asm!("
        isb sy",
        "mrs {}, CNTPCT_EL0", out(reg) tick)
    }
    tick
}

fn spin_for(delay: Duration) {
    let frequency: u64;
    unsafe {
        asm!("mrs {}, CNTFRQ_EL0", out(reg) frequency);
    };
    let start = tick();
    while (tick() - start) as f64 / frequency as f64 <= delay.as_secs_f64() {
        core::hint::spin_loop()
    }
}

pub struct Emmc {
    registers: MMIODerefWrapper<EmmcRegs>,
    rca: u16,
    csd: LocalRegisterCopy<u128, CSD::Register>,
}

impl Emmc {
    #[must_use]
    pub const fn new(addr: usize) -> Self {
        Self {
            registers: unsafe { MMIODerefWrapper::new(addr) },
            rca: 0,
            csd: LocalRegisterCopy::new(0),
        }
    }

    fn send_command(&mut self, command: FieldValue<u32, CMDTM::Register>, argument: u32) {
        assert!(self.wait_for_cmd_done());
        // Clear existing interrupts
        self.registers.INTERRUPT.set(self.registers.INTERRUPT.get());
        self.registers.ARG1.set(argument);
        self.registers.CMDTM.write(command);
        spin_for(Duration::from_micros(100));
        assert!(self.wait_for_cmd_done());
        println!(
            "COMMAND: {:08X}, ARG {:08X}, RESPONSE: {:08X}",
            self.registers.CMDTM.get(),
            argument,
            self.registers.RESP0.get()
        );
    }

    fn send_app_command(&mut self, command: FieldValue<u32, CMDTM::Register>, argument: u32) {
        self.send_command(CMDTM::CMD_INDEX::AppCmd, u32::from(self.rca) << 16);
        self.send_command(command, argument);
    }

    fn wait_for_cmd_done(&mut self) -> bool {
        while !self
            .registers
            .INTERRUPT
            .matches_any(INTERRUPT::CMD_DONE::SET + INTERRUPT::CTO_ERR::SET)
            && self.registers.STATUS.matches_any(STATUS::CMD_INHIBIT::SET)
        {
            core::hint::spin_loop();
        }
        !self
            .registers
            .INTERRUPT
            .matches_any(INTERRUPT::CTO_ERR::SET)
    }

    fn read_128bit_response(&self) -> u128 {
        (u128::from(self.registers.RESP3.get()) << 96)
            | (u128::from(self.registers.RESP2.get()) << 64)
            | (u128::from(self.registers.RESP1.get()) << 32)
            | u128::from(self.registers.RESP0.get())
    }

    fn all_send_cid(&mut self) {
        self.send_command(
            CMDTM::CMD_INDEX::AllSendCid + CMDTM::CMD_RSPNS_TYPE::Bits136,
            0,
        );
        let full_response = self.read_128bit_response();
        println!("CID: {:032X}", full_response);
    }

    fn select_card(&mut self) {
        self.send_command(
            CMDTM::CMD_INDEX::SelectCard + CMDTM::CMD_RSPNS_TYPE::Bits48Busy,
            u32::from(self.rca) << 16,
        );
        println!("status: {:?}", SdStatus::from(self.registers.RESP0.get()));
    }

    fn send_relative_addr(&mut self) {
        self.send_command(
            CMDTM::CMD_INDEX::SendRelativeAddr + CMDTM::CMD_RSPNS_TYPE::Bits48,
            0,
        );
        let response = self.registers.RESP0.get();
        let state = (response >> 9) & 0b11;
        assert_eq!(state, 2); // IDENT state before this command was executed
        self.rca = u16::try_from(response >> 16).unwrap();
        println!("rca {:X}", self.rca);
    }

    fn go_idle_state(&mut self) {
        self.send_command(CMDTM::CMD_INDEX::GoIdleState, 0);
    }

    fn send_if_cond(&mut self) {
        // Check code of 0xAA recommended by SD
        // Set to the normal voltage level
        const ARG: u32 = 0x1AA;
        self.send_command(
            CMDTM::CMD_INDEX::SendIFCond + CMDTM::CMD_RSPNS_TYPE::Bits48,
            0x1AA,
        );
        assert_eq!(self.registers.RESP0.get(), ARG);
    }

    fn send_csd(&mut self) {
        self.send_command(
            CMDTM::CMD_INDEX::SendCSD + CMDTM::CMD_RSPNS_TYPE::Bits136,
            u32::from(self.rca) << 16,
        );
        self.csd = LocalRegisterCopy::new(self.read_128bit_response());
        println!("CSD: {:032X}", self.csd.get());
        println!("blk size {:b}", self.csd.read(CSD::READ_BL_LEN));
        println!("cs {:X}", self.csd.read(CSD::C_SIZE));
        println!("cm {:X}", self.csd.read(CSD::C_SIZE_MULT));
        println!(
            "mem? {:X}",
            (self.csd.read(CSD::C_SIZE) + 1)
                * (self.csd.read(CSD::C_SIZE_MULT) << 8)
                * (self.csd.read(CSD::READ_BL_LEN) << 12)
        );
    }

    fn sd_send_op_cond(&mut self) {
        // Allow fancy SD cards
        // Use power saving mode
        // Magic argument for voltage?
        const ARG: u32 = 0x40FF_8000;
        self.send_app_command(
            CMDTM::CMD_INDEX::SDSendOpCond + CMDTM::CMD_RSPNS_TYPE::Bits48,
            ARG,
        );
        let mut response;
        // Wait for the response to be ready
        while {
            response = self.registers.RESP0.get();
            response & 0x8000_0000 == 0
        } {
            core::hint::spin_loop();
        }
        println!("IS SDSC/HC: {}", response & 0x4000_0000);
        println!("IS UHS II: {}", response & 0x2000_0000);
    }

    // Source: https://github.com/LdB-ECM/Raspberry-Pi/blob/master/SD_FAT32/SDCard.c#L1183
    fn set_clock_frequency(&mut self, hz: u32) {
        // The base clock frequency of the SD card, in hz
        const BASE_FREQUENCY: u32 = 41_666_667;
        // Wait for the card to be ready
        while self
            .registers
            .STATUS
            .matches_any(STATUS::CMD_INHIBIT::SET + STATUS::DAT_INHIBIT::SET)
        {
            core::hint::spin_loop();
        }

        // Disable the clock
        self.registers.CONTROL1.modify(CONTROL1::CLK_EN::CLEAR);
        spin_for(Duration::from_micros(10));

        // Figure out the desired divider
        let divisor = BASE_FREQUENCY.div_ceil(hz);
        // The divisor is split into two parts for the register
        let div_lo = divisor & 0xFF;
        let div_hi = divisor >> 8;
        self.registers
            .CONTROL1
            .modify(CONTROL1::CLK_FREQ8.val(div_lo) + CONTROL1::CLK_FREQ_MS2.val(div_hi));
        spin_for(Duration::from_micros(10));

        // Enable clock, wait for it to stabilize
        self.registers.CONTROL1.modify(CONTROL1::CLK_EN::SET);
        while !self
            .registers
            .CONTROL1
            .matches_any(CONTROL1::CLK_STABLE::SET)
        {
            core::hint::spin_loop();
        }
    }

    // Source: https://github.com/LdB-ECM/Raspberry-Pi/blob/master/SD_FAT32/SDCard.c#L1100
    fn read_scr() {
        // Size 1 block, count 8
    }

    fn set_blocklen(&mut self, len: u32) {
        assert!(len.is_power_of_two());
        self.send_command(
            CMDTM::CMD_INDEX::SetBlocklen + CMDTM::CMD_RSPNS_TYPE::Bits48,
            len,
        );
        println!("status: {:?}", SdStatus::from(self.registers.RESP0.get()));
    }

    pub fn read_blk(&mut self, blk: u32, buf: &mut [u8; 512]) {
        self.registers
            .BLKSIZECNT
            .write(BLKSIZECNT::BLKSIZE.val(512) + BLKSIZECNT::BLKCNT.val(1));
        self.send_command(
            CMDTM::CMD_INDEX::ReadSingleBlock
                + CMDTM::CMD_RSPNS_TYPE::Bits48
                + CMDTM::CMD_ISDATA::SET
                + CMDTM::TM_DAT_DIR::CardToHost,
            blk << 9,
        );

        println!("status: {:?}", SdStatus::from(self.registers.RESP0.get()));
        while !self
            .registers
            .INTERRUPT
            .matches_any(INTERRUPT::READ_RDY::SET)
        {
            core::hint::spin_loop();
        }

        for _c in buf.chunks_exact(4) {
            let c = self.registers.DATA.get();
            println!("0x{:08X}", c);
        }
    }

    pub fn init(&mut self) {
        const INIT_FREQUENCY: u32 = 400_000;
        const MAIN_FREQUENCY: u32 = 2_500_000;
        // Reset the card
        self.registers.CONTROL0.set(0);
        self.registers.CONTROL1.write(CONTROL1::SRST_HC::SET);
        while self.registers.CONTROL1.matches_all(CONTROL1::SRST_HC::SET) {
            core::hint::spin_loop();
        }

        // Enable internal clock and configure data timeouts
        self.registers
            .CONTROL1
            .modify(CONTROL1::DATA_TOUNIT.val(0b1110) + CONTROL1::CLK_INTLEN::SET);

        self.set_clock_frequency(INIT_FREQUENCY);

        // Enable masked interrupts (i.e., let's use polling)
        self.registers.IRPT_MASK.set(0xFFFF_FFFF);
        self.registers.IRPT_EN.set(0xFFFF_FFFF);

        self.go_idle_state();
        self.send_if_cond();
        self.sd_send_op_cond();
        self.all_send_cid();
        self.send_relative_addr();
        self.send_csd();

        self.set_clock_frequency(MAIN_FREQUENCY);

        self.select_card();
        self.set_blocklen(512);
    }
}

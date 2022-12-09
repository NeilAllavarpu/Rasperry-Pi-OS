use aarch64_cpu::registers::{ESR_EL1, FAR_EL1};
use tock_registers::{interfaces::Readable, register_bitfields};

use crate::log;

#[no_mangle]
extern "C" fn curr_el0_sync() {
    panic!("Synchronous exception taken with SP_EL0");
}

#[no_mangle]
extern "C" fn curr_el0_irq() {
    panic!("IRQ taken with SP_EL0");
}

#[no_mangle]
extern "C" fn curr_el0_fiq() {
    panic!("FIQ taken with SP_EL0");
}

#[no_mangle]
extern "C" fn curr_el0_other() {
    panic!("Miscellaneous exception taken with SP_EL0");
}

#[no_mangle]
extern "C" fn curr_elx_sync() {
    match ESR_EL1.read_as_enum(ESR_EL1::EC) {
        Some(ESR_EL1::EC::Value::InstrAbortCurrentEL) => handle_instruction_abort(),
        Some(ESR_EL1::EC::Value::DataAbortCurrentEL) => handle_data_abort(),
        None => panic!("Invalid synchronous exception taken with SP_ELX"),
        _ => todo!(
            "Unhandled synchronous exception taken with SP_ELX: {:06b}",
            ESR_EL1.read(ESR_EL1::EC)
        ),
    };
}

register_bitfields![u64, DataAbortISS [
    VALID OFFSET(24) NUMBITS(1) [
        INVALID = 0b0,
        VALID = 0b1,
    ],
    SAS OFFSET(22) NUMBITS(2) [
        BYTE = 0b00,
        HALFWORD = 0b01,
        WORD = 0b10,
        DOUBLEWORD = 0b11,
    ],
    DFSC OFFSET(0) NUMBITS(6) [
        ALIGNMENT_FAULT = 0b100001,
    ]
]];

fn handle_instruction_abort() {
    log!(
        "Instruction Abort exception, taken from the current EL: {:b}",
        ESR_EL1.read(ESR_EL1::ISS)
    );
    log!("Faulting address: 0x{:16X}", FAR_EL1.get());
    match ESR_EL1.read_as_enum::<DataAbortISS::VALID::Value>(ESR_EL1::ISS) {
        Some(DataAbortISS::VALID::Value::VALID) => {
            match ESR_EL1.read_as_enum::<DataAbortISS::SAS::Value>(ESR_EL1::ISS) {
                Some(DataAbortISS::SAS::Value::BYTE) => log!("Size: byte"),
                Some(DataAbortISS::SAS::Value::HALFWORD) => log!("Size: halfword"),
                Some(DataAbortISS::SAS::Value::WORD) => log!("Size: word"),
                Some(DataAbortISS::SAS::Value::DOUBLEWORD) => log!("Size: doubleword"),
                _ => unreachable!(),
            }
        }
        _ => log!("Invalid syndrome"),
    }

    match ESR_EL1.read_as_enum::<DataAbortISS::DFSC::Value>(ESR_EL1::ISS) {
        Some(DataAbortISS::DFSC::Value::ALIGNMENT_FAULT) => {
            log!("Reason: Alignment fault")
        }
        _ => log!("Unhandled status code"),
    }

    log!("{:b}", ESR_EL1.get() & 0b111111);

    panic!("Unable to handle exception");
}

fn handle_data_abort() {
    log!(
        "Data Abort exception, taken from the current EL: {:b}",
        ESR_EL1.read(ESR_EL1::ISS)
    );
    log!("Faulting address: 0x{:16X}", FAR_EL1.get());
    match ESR_EL1.read_as_enum::<DataAbortISS::VALID::Value>(ESR_EL1::ISS) {
        Some(DataAbortISS::VALID::Value::VALID) => {
            match ESR_EL1.read_as_enum::<DataAbortISS::SAS::Value>(ESR_EL1::ISS) {
                Some(DataAbortISS::SAS::Value::BYTE) => log!("Size: byte"),
                Some(DataAbortISS::SAS::Value::HALFWORD) => log!("Size: halfword"),
                Some(DataAbortISS::SAS::Value::WORD) => log!("Size: word"),
                Some(DataAbortISS::SAS::Value::DOUBLEWORD) => log!("Size: doubleword"),
                _ => unreachable!(),
            }
        }
        _ => log!("Invalid syndrome"),
    }

    match ESR_EL1.read_as_enum::<DataAbortISS::DFSC::Value>(ESR_EL1::ISS) {
        Some(DataAbortISS::DFSC::Value::ALIGNMENT_FAULT) => {
            log!("Reason: Alignment fault")
        }
        _ => log!("Unhandled status code"),
    }

    log!("{:b}", ESR_EL1.get() & 0b111111);

    panic!("Unable to handle exception");
}

#[no_mangle]
extern "C" fn curr_elx_irq() {
    panic!("IRQ taken with SP_ELX");
}

#[no_mangle]
extern "C" fn curr_elx_fiq() {
    panic!("FIQ taken with SP_ELX");
}

#[no_mangle]
extern "C" fn curr_elx_other() {
    panic!("Miscellaneous exception taken with SP_ELX");
}

#[no_mangle]
extern "C" fn lower_el_sync_64() {
    panic!("Synchronous exception taken from lower EL, in 64-bit");
}

#[no_mangle]
extern "C" fn lower_el_irq_64() {
    panic!("IRQ taken from lower EL, in 64-bit");
}

#[no_mangle]
extern "C" fn lower_el_fiq_64() {
    panic!("FIQ taken from lower EL, in 64-bit");
}

#[no_mangle]
extern "C" fn lower_el_other_64() {
    panic!("Miscellaneous exception taken from lower EL, in 64-bit");
}

#[no_mangle]
extern "C" fn lower_el_sync_32() {
    panic!("Synchronous exception taken from lower EL, in 32-bit");
}

#[no_mangle]
extern "C" fn lower_el_irq_32() {
    panic!("IRQ taken from lower EL, in 32-bit");
}

#[no_mangle]
extern "C" fn lower_el_fiq_32() {
    panic!("FIQ taken from lower EL, in 32-bit");
}

#[no_mangle]
extern "C" fn lower_el_other_32() {
    panic!("Miscellaneous exception taken from lower EL, in 32-bit");
}

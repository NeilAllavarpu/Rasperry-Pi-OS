use crate::{architecture, kernel, call_once, log};
use aarch64_cpu::registers::{MIDR_EL1, MPIDR_EL1};
use core::num::NonZeroU32;
use tock_registers::interfaces::Readable;

struct ConfigEntry<T> {
    description: &'static str,
    value: T,
}

impl<T> ConfigEntry<T> {
    const fn new(description: &'static str, value: T) -> Self {
        Self { description, value }
    }
}

impl ConfigEntry<bool> {
    fn log(&self) -> () {
        log!(
            "{}: {}",
            self.description,
            if self.value { "Yes" } else { "No" }
        );
    }
}

impl ConfigEntry<&'static str> {
    fn log(&self) -> () {
        log!("{}: {}", self.description, self.value);
    }
}

impl ConfigEntry<NonZeroU32> {
    fn log(&self) -> () {
        log!("{}: {}", self.description, self.value);
    }
}

impl ConfigEntry<(u8, u8, u8)> {
    fn log(&self) -> () {
        log!(
            "{}: {}.{}.{}",
            self.description,
            self.value.0,
            self.value.1,
            self.value.2
        );
    }
}

pub struct Config {
    architecture: ConfigEntry<&'static str>,
    implementer: ConfigEntry<&'static str>,
    is_uniprocessor: ConfigEntry<bool>,
    multithreading_low_affinity: ConfigEntry<bool>,
    product_info: ConfigEntry<(u8, u8, u8)>,
    timer_frequency: ConfigEntry<NonZeroU32>,
}

use MIDR_EL1::{Architecture, Implementer};

impl Config {
    /// Discovers configuration of the system
    pub fn create() -> Self {
        Self {
            architecture: ConfigEntry::new(
                "Architecture",
                match MIDR_EL1.read(Architecture) {
                    0b0001 => "Armv4",
                    0b0010 => "Armv4T",
                    0b0011 => "Armv5 (obsolete)",
                    0b0100 => "Armv5T",
                    0b0101 => "Armv5TE",
                    0b0110 => "Armv5TEJ",
                    0b0111 => "Armv6",
                    0b1111 => "Features individually identifed",
                    _ => unreachable!("Invalid architecture version value"),
                },
            ),
            implementer: ConfigEntry::new(
                "Implementer",
                MIDR_EL1.read_as_enum(Implementer).map_or(
                    "(Unknown)",
                    |value: Implementer::Value| match value {
                        Implementer::Value::Reserved => "(Reserved for software use)",
                        Implementer::Value::Arm => "Arm Limited",
                        Implementer::Value::Broadcom => "Broadcom Corporation",
                        Implementer::Value::Cavium => "Cavium Inc",
                        Implementer::Value::DigitalEquipment => "Digital Equipment Corporation",
                        Implementer::Value::Fujitsu => "Fujitsu Ltd",
                        Implementer::Value::Infineon => "Infineon Technologies AG",
                        Implementer::Value::MotorolaOrFreescale => {
                            "Motorola or Freescale Semiconductor Inc"
                        }
                        Implementer::Value::NVIDIA => "NVIDIA Corporation",
                        Implementer::Value::AppliedMicroCircuits => {
                            "Applied Micro Circuits Corporation"
                        }
                        Implementer::Value::Qualcomm => "Qualcomm Inc",
                        Implementer::Value::Marvell => "Marvell International Ltd.",
                        Implementer::Value::Intel => "Intel Corporation",
                        Implementer::Value::Ampere => "Ampere Computing",
                    },
                ),
            ),
            is_uniprocessor: ConfigEntry::new(
                "Is uniprocessor system",
                (MPIDR_EL1.get() & 0x40000000) == 1,
            ),
            multithreading_low_affinity: ConfigEntry::new(
                "Hardware threading",
                (MPIDR_EL1.get() & 0x800000) == 1,
            ),
            product_info: ConfigEntry::new(
                "Device variant/version",
                (
                    MIDR_EL1.read(MIDR_EL1::Variant) as u8,
                    MIDR_EL1.read(MIDR_EL1::PartNum) as u8,
                    MIDR_EL1.read(MIDR_EL1::Revision) as u8,
                ),
            ),
            timer_frequency: ConfigEntry::new("Timer frequency (Hz)", architecture::timer::timer_frequency()),
        }
    }

    pub fn log(&self) -> () {
        log!("---  ABOUT  ME  ---");

        log!("*** Device info");
        self.architecture.log();
        self.implementer.log();
        self.product_info.log();

        log!("*** Multiprocessing info");
        self.is_uniprocessor.log();
        self.multithreading_low_affinity.log();

        log!("*** Timer info");
        self.timer_frequency.log();

        log!("--- END ABOUT ME ---")
    }
}

// These *should* be static so sharing is fine
unsafe impl Sync for Config {}
unsafe impl Send for Config {}

pub static CONFIG: kernel::SetOnce<Config> = kernel::SetOnce::new();

/// Initializes the configuration
pub fn init() {
    // Should only attempt to set the config once
    call_once!();
    CONFIG.set(Config::create());
}

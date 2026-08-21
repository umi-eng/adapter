use anyhow::{Context, bail};
use probe_rs::{MemoryInterface, Permissions, probe::list::Lister};
use std::{thread, time::Duration};

// registers
const FLASH: u64 = 0x4002_2000;
const FLASH_KEYR: u64 = FLASH + 0x08;
const FLASH_OPTKEYR: u64 = FLASH + 0x0c;
const FLASH_SR: u64 = FLASH + 0x10;
const FLASH_CR: u64 = FLASH + 0x14;
const FLASH_OPTR: u64 = FLASH + 0x20;

// constants
const FLASH_KEY1: u32 = 0x4567_0123;
const FLASH_KEY2: u32 = 0xcdef_89ab;
const FLASH_OPTKEY1: u32 = 0x0819_2a3b;
const FLASH_OPTKEY2: u32 = 0x4c5d_6e7f;

// bit offsets
const BFB2: u32 = 1 << 20;
const BSY: u32 = 1 << 16;
const OPTSTRT: u32 = 1 << 17;
const OBL_LAUNCH: u32 = 1 << 27;
const FLASH_ERROR_BITS: u32 = 0x0000_c3fa;

#[derive(Debug, clap::Parser)]
pub struct SwitchBanks {
    /// Specify a desired bank. Without this argument, switch to the other bank.
    #[arg(value_enum)]
    bank: Option<Bank>,

    /// Do not reload the option bytes and restart the MCU.
    #[arg(long)]
    no_restart: bool,
}

impl SwitchBanks {
    pub fn run(self) -> anyhow::Result<()> {
        let lister = Lister::new();
        let probe_info = lister
            .list_all()
            .into_iter()
            .next()
            .context("no debug probe found")?;
        let probe = probe_info.open().context("failed to open debug probe")?;
        let mut session = probe
            .attach("STM32G474CEUx", Permissions::default())
            .with_context(|| format!("failed to attach to target"))?;
        let mut core = session.core(0).context("failed to access MCU core")?;

        let option_bytes = core
            .read_word_32(FLASH_OPTR)
            .context("failed to read FLASH_OPTR")?;
        let current_bank = if option_bytes & BFB2 == 0 {
            Bank::Bank1
        } else {
            Bank::Bank2
        };
        let desired_bank = self.bank.unwrap_or_else(|| current_bank.other());

        if desired_bank == current_bank {
            println!("already using {desired_bank:?}");
            if self.no_restart {
                return Ok(());
            }
        }

        let option_bytes = match desired_bank {
            Bank::Bank1 => option_bytes & !BFB2,
            Bank::Bank2 => option_bytes | BFB2,
        };

        // Both unlock sequences are required before changing option bytes.
        core.write_word_32(FLASH_KEYR, FLASH_KEY1)
            .context("failed to write first flash unlock key")?;
        core.write_word_32(FLASH_KEYR, FLASH_KEY2)
            .context("failed to write second flash unlock key")?;
        core.write_word_32(FLASH_OPTKEYR, FLASH_OPTKEY1)
            .context("failed to write first option-byte unlock key")?;
        core.write_word_32(FLASH_OPTKEYR, FLASH_OPTKEY2)
            .context("failed to write second option-byte unlock key")?;
        core.write_word_32(FLASH_OPTR, option_bytes)
            .context("failed to write FLASH_OPTR")?;
        let control = core.read_word_32(FLASH_CR).context(
            "failed to read FLASH_CR before starting option-byte programming",
        )?;
        core.write_word_32(FLASH_CR, control | OPTSTRT)
            .context("failed to start flash option-byte programming")?;

        for _ in 0..500 {
            let status = core
                .read_word_32(FLASH_SR)
                .context("failed to read FLASH_SR while waiting for option-byte programming")?;
            if status & BSY == 0 {
                if status & FLASH_ERROR_BITS != 0 {
                    bail!(
                        "flash option-byte programming failed (FLASH_SR = {status:#010x})"
                    );
                }
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }

        let status = core
            .read_word_32(FLASH_SR)
            .context("failed to read FLASH_SR after option-byte programming")?;
        if status & BSY != 0 {
            bail!("timed out waiting for flash option-byte programming");
        }
        println!("selected {desired_bank:?}");

        if !self.no_restart {
            // OBL_LAUNCH reloads the option bytes and resets the MCU. The debug
            // connection is expected to disappear as the new bank starts.
            let control = core
                .read_word_32(FLASH_CR)
                .context("failed to read FLASH_CR before restarting the MCU")?;
            // The reset disconnects SWD while this write is completing. probe-rs
            // can therefore return SwdDpError even though the launch succeeded.
            let _ = core.write_word_32(FLASH_CR, control | OBL_LAUNCH);
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Bank {
    Bank1,
    Bank2,
}

impl Bank {
    fn other(self) -> Self {
        match self {
            Self::Bank1 => Self::Bank2,
            Self::Bank2 => Self::Bank1,
        }
    }
}

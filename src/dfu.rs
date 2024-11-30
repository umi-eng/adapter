//! Device firmware upgrade.

use crate::hal::stm32::{FLASH, SYSCFG};
use core::ops::RangeInclusive;
use usbd_dfu::*;

const KEY1: u32 = 0x4567_0123;
const KEY2: u32 = 0xCDEF_89AB;
const OPT_KEY1: u32 = 0x0819_2A3B;
const OPT_KEY2: u32 = 0x4C5D_6E7F;
const FLASH_MEMORY: RangeInclusive<u32> = 0x0800_0000..=0x0803_FFFF;
const BANK2_OFFSET: u32 = 0x00040000;

/// Bank erase selection.
const CR_BKER: u32 = 1 << 11;
/// Boot from bank 2 enabled bit.
const OPTR_BFB2: u32 = 1 << 20;
/// Dual bank mode enabled bit.
const OPTR_DBANK: u32 = 1 << 22;

#[derive(Debug, Clone, Copy, PartialEq, Eq, defmt::Format)]
#[repr(u8)]
pub enum Bank {
    Bank1 = 0,
    Bank2 = 1,
}

pub struct DfuFlash {
    buffer: [u8; 2048],
    flash: FLASH,
    syscfg: SYSCFG,
}

impl DfuFlash {
    pub fn new(flash: FLASH, syscfg: SYSCFG) -> Self {
        let mut this = Self {
            buffer: [0; 2048],
            flash,
            syscfg,
        };

        this.enable_dual_bank();

        let active = this.active_bank();
        defmt::info!("Active flash bank: {}", active);

        this
    }

    fn unlock<F, T>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut FLASH, &mut [u8]) -> T,
    {
        self.flash.keyr.write(|w| unsafe { w.bits(KEY1) });
        self.flash.keyr.write(|w| unsafe { w.bits(KEY2) });

        // Flash should unlock on first try. If not we are in an unrecoverable
        // state.
        if self.flash.cr.read().lock().bit() {
            panic!("Flash is still locked");
        }

        let result = f(&mut self.flash, &mut self.buffer);

        self.flash.cr.modify(|_, w| w.lock().set_bit());

        result
    }

    fn opt_unlock<F, T>(&mut self, f: F) -> T
    where
        F: FnOnce(&mut FLASH) -> T,
    {
        self.unlock(|mut flash, _| {
            flash.optkeyr.write(|w| unsafe { w.bits(OPT_KEY1) });
            flash.optkeyr.write(|w| unsafe { w.bits(OPT_KEY2) });

            // Flash options should unlock on first try. If not we are in an
            // unrecoverable state.
            if flash.cr.read().optlock().bit() {
                panic!("Flash opt is still locked");
            }

            let result = f(&mut flash);

            flash.cr.modify(|_, w| w.optlock().set_bit());

            result
        })
    }

    /// Enable dual bank flash mode.
    pub fn enable_dual_bank(&mut self) {
        self.opt_unlock(|f| {
            f.optr
                .modify(|r, w| unsafe { w.bits(r.bits() | OPTR_DBANK) });

            f.cr.modify(|_, w| w.optstrt().set_bit());

            // wait while busy
            while f.sr.read().bsy().bit_is_set() {}
        });
    }

    fn sector_from_address(&mut self, address: u32) -> Option<u8> {
        let base = 0x0800_0000;
        let sector_size = 2048;

        // Ensure address is within range
        if address < base {
            return None;
        }

        // Check if address is at start of sector
        if (address - base) % sector_size != 0 {
            return None;
        }

        // Calculate sector number
        let sector = (address - base) / sector_size;

        // Verify sector is within valid range
        if sector <= 127 {
            Some(sector as u8)
        } else {
            None
        }
    }

    /// Get active bank number.
    fn active_bank(&self) -> Bank {
        let bank = (self.flash.optr.read().bits() & OPTR_BFB2) != 0;
        match bank {
            false => Bank::Bank1,
            true => Bank::Bank2,
        }
    }

    #[allow(unused)]
    fn inactive_bank(&self) -> Bank {
        match self.active_bank() {
            Bank::Bank1 => Bank::Bank2,
            Bank::Bank2 => Bank::Bank1,
        }
    }

    /// Swap flash bank boot selection.
    fn swap_banks(&mut self) {
        let bank = self.active_bank();

        self.opt_unlock(|f| {
            match bank {
                Bank::Bank1 => f
                    .optr
                    .modify(|r, w| unsafe { w.bits(r.bits() | OPTR_BFB2) }),
                Bank::Bank2 => f
                    .optr
                    .modify(|r, w| unsafe { w.bits(r.bits() & !OPTR_BFB2) }),
            };

            f.cr.modify(|_, w| w.optstrt().set_bit());

            while f.sr.read().bsy().bit_is_set() {}

            // launch new firmware
            f.cr.modify(|_, w| w.obl_launch().set_bit());
        });
    }
}

impl DfuMemory for DfuFlash {
    const MEM_INFO_STRING: &'static str = "@Flash/0x08000000/128*2Kf";
    const INITIAL_ADDRESS_POINTER: u32 = *FLASH_MEMORY.start();
    const PROGRAM_TIME_MS: u32 = 3;
    const ERASE_TIME_MS: u32 = 25;
    const FULL_ERASE_TIME_MS: u32 = 25;
    const TRANSFER_SIZE: u16 = 128;
    const MANIFESTATION_TOLERANT: bool = false;

    fn read(
        &mut self,
        address: u32,
        length: usize,
    ) -> Result<&[u8], DfuMemoryError> {
        if !FLASH_MEMORY.contains(&address) {
            return Err(DfuMemoryError::Address);
        }

        let address = address as *const u8;
        Ok(unsafe { core::slice::from_raw_parts(address, length) })
    }

    fn erase(&mut self, address: u32) -> Result<(), DfuMemoryError> {
        if !FLASH_MEMORY.contains(&address) {
            return Err(DfuMemoryError::Address);
        }

        let sector = self.sector_from_address(address).unwrap();

        self.unlock(|f, _| {
            // clear any existing operations
            f.cr.modify(|_, w| unsafe { w.bits(0) });

            f.cr.modify(|_, w| unsafe {
                w.bits(CR_BKER).pnb().bits(sector).per().set_bit()
            });

            f.cr.modify(|_, w| w.strt().set_bit());

            // wait while busy
            while f.sr.read().bsy().bit_is_set() {}

            // remove page erase operation bit
            f.cr.modify(|_, w| w.per().clear_bit());
        });

        Ok(())
    }

    fn erase_all(&mut self) -> Result<(), DfuMemoryError> {
        defmt::warn!("Mass erase not supported.");
        Err(DfuMemoryError::Unknown)
    }

    fn store_write_buffer(&mut self, src: &[u8]) -> Result<(), ()> {
        if src.len() <= self.buffer.len() {
            self.buffer[..src.len()].copy_from_slice(src);
            Ok(())
        } else {
            Err(())
        }
    }

    fn program(
        &mut self,
        address: u32,
        length: usize,
    ) -> Result<(), DfuMemoryError> {
        if !FLASH_MEMORY.contains(&address) {
            return Err(DfuMemoryError::Address);
        }

        // Always write to the inactive bank.
        let address = address + BANK2_OFFSET;

        self.unlock(|f, buffer| {
            let data = &mut buffer[..length];

            for idx in (0..data.len()).step_by(8) {
                let address1 = (address as u32 + idx as u32) as *mut u32;
                let address2 = (address as u32 + 4 + idx as u32) as *mut u32;

                let word1: u32;
                let word2: u32;

                // Check the data is enough to fill two words. If not, pad the
                // data with 0xff.
                // Taken from: https://github.com/stm32-rs/stm32g4xx-hal/blob/main/src/flash.rs
                if idx + 8 > data.len() {
                    let mut tmp_buffer = [255u8; 8];
                    tmp_buffer[idx..data.len()].copy_from_slice(
                        &data[(idx + idx)..(data.len() + idx)],
                    );
                    let tmp_dword = u64::from_le_bytes(tmp_buffer);
                    word1 = tmp_dword as u32;
                    word2 = (tmp_dword >> 32) as u32;
                } else {
                    word1 = (data[idx] as u32)
                        | (data[idx + 1] as u32) << 8
                        | (data[idx + 2] as u32) << 16
                        | (data[idx + 3] as u32) << 24;

                    word2 = (data[idx + 4] as u32)
                        | (data[idx + 5] as u32) << 8
                        | (data[idx + 6] as u32) << 16
                        | (data[idx + 7] as u32) << 24;
                }

                f.cr.modify(|_, w| w.pg().set_bit());

                // wait while busy
                while f.sr.read().bsy().bit_is_set() {}

                unsafe {
                    core::ptr::write_volatile(address1, word1);
                    core::ptr::write_volatile(address2, word2);
                }
            }
        });

        Ok(())
    }
    fn manifestation(&mut self) -> Result<(), DfuManifestationError> {
        self.swap_banks();

        crate::hal::cortex_m::peripheral::SCB::sys_reset()
    }
}

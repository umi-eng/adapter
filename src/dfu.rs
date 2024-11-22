//! Device firmware upgrade.

use crate::hal::stm32::FLASH;
use usbd_dfu::*;

pub struct DfuFlash {
    write_buffer: [u8; 2048],
    flash: FLASH,
}

impl DfuFlash {
    pub fn new(flash: FLASH) -> Self {
        Self {
            write_buffer: [0; 2048],
            flash,
        }
    }
}

impl DFUMemIO for DfuFlash {
    const MEM_INFO_STRING: &'static str = "@Flash/0x08000000/128*2Kg";
    const INITIAL_ADDRESS_POINTER: u32 = 0x0800_0000;
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

    fn manifestation(&mut self) -> Result<(), DFUManifestationError> {
        todo!()
    }
}

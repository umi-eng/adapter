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
    ) -> Result<(), DFUMemError> {
        todo!()
    }

    fn manifestation(&mut self) -> Result<(), DFUManifestationError> {
        todo!()
    }
}

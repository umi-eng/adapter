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
    const INITIAL_ADDRESS_POINTER: u32 = 0x08000000;
    const PROGRAM_TIME_MS: u32 = 3;
    const ERASE_TIME_MS: u32 = 25;
    const FULL_ERASE_TIME_MS: u32 = 25;
    const TRANSFER_SIZE: u16 = 32;

    fn read(
        &mut self,
        address: u32,
        length: usize,
    ) -> Result<&[u8], DFUMemError> {
        let address = address as *const u8;

        Ok(unsafe { core::slice::from_raw_parts(address, length) })
    }

    fn erase(&mut self, address: u32) -> Result<(), DFUMemError> {
        todo!()
    }

    fn erase_all(&mut self) -> Result<(), DFUMemError> {
        todo!()
    }

    fn store_write_buffer(&mut self, src: &[u8]) -> Result<(), ()> {
        todo!()
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

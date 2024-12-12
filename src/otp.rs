use crate::{dfu::KEY, hal::stm32::FLASH};

const OTP_LEN: usize = 1024; // 1 kilobyte
const OTP_ADDRESS: *const u8 = 0x1FFF7000 as *const u8;

/// Reads the 1 kilobyte of OTP memory.
#[allow(unused)]
pub fn read() -> &'static [u8] {
    unsafe { core::slice::from_raw_parts(OTP_ADDRESS, OTP_LEN) }
}

/// Write data to OTP memory.
pub fn write(
    flash: &mut FLASH,
    data: &[u8],
    offset: usize,
) -> Result<(), OtpWriteError> {
    if data.len() + offset > OTP_LEN {
        return Err(OtpWriteError::PayloadSize);
    }

    // check otp is blank.
    let otp = &read()[offset..data.len() + offset];
    for byte in otp {
        if *byte != 0xff {
            return Err(OtpWriteError::Occupied);
        }
    }

    // unlock flash writing.
    flash.keyr.write(|w| unsafe { w.bits(KEY[0]) });
    flash.keyr.write(|w| unsafe { w.bits(KEY[1]) });

    // check unlock worked.
    if flash.cr.read().lock().bit() {
        panic!("Flash is still locked.");
    }

    defmt::info!("Pretend write!: {:x}", data);

    // lock flash
    flash.cr.modify(|_, w| w.lock().set_bit());

    Ok(())
}

/// OTP memory write error.
#[derive(Debug, defmt::Format, Clone, Copy, PartialEq, Eq)]
pub enum OtpWriteError {
    /// Payload will not fit in OTP.
    PayloadSize,
    /// Memory region is already occupied.
    Occupied,
}

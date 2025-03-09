//! # One-time-Programmable Memory
//!
//! Read and write operations for OTP memory.

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

    let address = OTP_ADDRESS as u32 + offset as u32;

    for idx in (0..data.len()).step_by(8) {
        let address1 = (address + idx as u32) as *mut u32;
        let address2 = (address + 4 + idx as u32) as *mut u32;

        let (word1, word2) = if idx + 8 > data.len() {
            // pad writes smaller than double word.
            let mut tmp_buffer = [0xff; 8];
            let remaining = data.len() - idx;
            tmp_buffer[..remaining].copy_from_slice(&data[idx..]);
            let tmp_dword = u64::from_le_bytes(tmp_buffer);
            (tmp_dword as u32, (tmp_dword >> 32) as u32)
        } else {
            // convert 8 bytes into two 32-bit words
            let bytes1 = &data[idx..idx + 4];
            let bytes2 = &data[idx + 4..idx + 8];
            (
                u32::from_le_bytes(bytes1.try_into().unwrap()),
                u32::from_le_bytes(bytes2.try_into().unwrap()),
            )
        };

        flash.cr.modify(|_, w| w.pg().set_bit());

        // wait while busy
        while flash.sr.read().bsy().bit_is_set() {}

        unsafe {
            core::ptr::write_volatile(address1, word1);
            core::ptr::write_volatile(address2, word2);
        }
    }

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

//! Vital product data.

use crate::{dfu::KEY, hal::stm32::FLASH};
use bitflags::bitflags;
use core::{convert::Infallible, fmt::Formatter, slice};
use defmt::Format;
use tlvc::{TlvcReadError, TlvcReader};
use zerocopy::{AsBytes, FromBytes, FromZeroes};

/// Vital product data
#[derive(Debug, Format, AsBytes, FromZeroes, FromBytes)]
#[repr(C)]
pub struct VitalProductData {
    pub serial: Serial,
    pub version: Version,
    pub sku: Sku,
    pub features: Features,
}

impl VitalProductData {
    /// Read TLV-C product data.
    ///
    /// If a tag is not presen, the default value for the type is used.
    pub fn from_tlvc(buf: &[u8]) -> Result<Self, TlvcReadError<Infallible>> {
        let mut serial = None;
        let mut version = None;
        let mut sku = None;
        let mut features = None;

        let mut reader = TlvcReader::begin(buf)?;
        while let Ok(Some(chunk)) = reader.next() {
            match &chunk.header().tag {
                b"SER " => serial = Self::process_chunk(&chunk)?,
                b"VER " => version = Self::process_chunk(&chunk)?,
                b"SKU " => sku = Self::process_chunk(&chunk)?,
                b"FEAT" => features = Self::process_chunk(&chunk)?,
                _ => {} // do nothing for unknown tags
            }
        }

        Ok(Self {
            serial: serial.unwrap_or_default(),
            version: version.unwrap_or_default(),
            sku: sku.unwrap_or_default(),
            features: features.unwrap_or_default(),
        })
    }

    /// Process a TLV-C chunk, unmarshalling the given type from the data or
    /// returning `None` if that fails.
    fn process_chunk<T: FromBytes + AsBytes + FromZeroes>(
        chunk: &tlvc::ChunkHandle<&[u8]>,
    ) -> Result<Option<T>, TlvcReadError<Infallible>> {
        if chunk.len() as usize != core::mem::size_of::<T>() {
            defmt::error!("Chunk length {} incorrect.", chunk.len());
            return Ok(None);
        }

        let mut checksum_buf = [0; 2];
        chunk.check_body_checksum(&mut checksum_buf)?;

        let mut out = T::new_zeroed();
        chunk.read_exact(0, out.as_bytes_mut())?;
        Ok(Some(out))
    }
}

/// Serial number.
#[derive(Debug, AsBytes, FromZeroes, FromBytes)]
#[repr(C)]
pub struct Serial {
    pub year: u8,
    pub week: u8,
    pub seq: u16,
}

impl Default for Serial {
    fn default() -> Self {
        Self {
            year: 99,
            week: 99,
            seq: 0x9999,
        }
    }
}

impl Serial {
    /// Assert size at compile time.
    const _SIZE: () = assert!(core::mem::size_of::<Self>() == 4);

    /// Creates a new [`Serial`]
    pub fn new(year: u8, week: u8, seq: u16) -> Self {
        Self { year, week, seq }
    }
}

impl defmt::Format for Serial {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(fmt, "{:02}{:02}-{:04X}", self.year, self.week, self.seq)
    }
}

impl core::fmt::Display for Serial {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:02}{:02}-{:04X}", self.year, self.week, self.seq)
    }
}

/// Semantic version number.
#[derive(Debug, AsBytes, FromZeroes, FromBytes)]
#[repr(C)]
pub struct Version {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
    pub pre: u8,
}

impl Default for Version {
    fn default() -> Self {
        Self {
            major: 0,
            minor: 0,
            patch: 0,
            pre: 0,
        }
    }
}

impl Version {
    /// Assert size at compile time.
    const _SIZE: () = assert!(core::mem::size_of::<Self>() == 4);

    /// Creates a new [`Version`].
    pub fn new(major: u8, minor: u8, patch: u8, pre: u8) -> Self {
        Self {
            major,
            minor,
            patch,
            pre,
        }
    }
}

impl defmt::Format for Version {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(fmt, "{}.{}.{}", self.major, self.minor, self.patch);
        if self.pre != 0 {
            defmt::write!(fmt, "-rc.{}", self.pre);
        }
    }
}

/// Product variant.
#[derive(Debug, Format, AsBytes, FromBytes, FromZeroes)]
#[repr(C)]
pub struct Sku([u8; 4]);

impl Default for Sku {
    fn default() -> Self {
        Self([b'N', b'O', b'N', b'E'])
    }
}

impl Sku {
    /// Assert size at compile time
    const _SIZE: () = assert!(core::mem::size_of::<Self>() == 4);

    /// Create a new sku.
    ///
    /// The id provided must be valid ASCII.
    pub fn new(id: [u8; 4]) -> Self {
        assert!(id.is_ascii());
        Self(id)
    }

    /// Get product variant id.
    pub fn id(&self) -> [u8; 4] {
        self.0
    }
}

/// Optional features that may be present on the board.
/// Up to 16 unique features are supported by this field.
#[derive(Debug, Default, Format, AsBytes, FromZeroes, FromBytes)]
#[repr(C)]
pub struct Features(u32);

impl Features {
    /// Assert size at compile time.
    const _SIZE: () = assert!(core::mem::size_of::<Self>() == 4);
}

bitflags! {
    impl Features: u32 {
        const Test = 1 << 0;
    }
}

const OTP_LEN: usize = 1024; // 1 kilobyte
const OTP_ADDRESS: *const u8 = 0x1FFF7000 as *const u8;

/// Reads the 1 kilobyte of OTP memory.
#[allow(unused)]
pub fn read_otp() -> &'static [u8] {
    unsafe { slice::from_raw_parts(OTP_ADDRESS, OTP_LEN) }
}

/// Write data to OTP memory.
pub fn write_otp(
    flash: &mut FLASH,
    data: &[u8],
    offset: usize,
) -> Result<(), OtpWriteError> {
    if data.len() + offset > OTP_LEN {
        return Err(OtpWriteError::PayloadSize);
    }

    // check otp is blank.
    let otp = &read_otp()[offset..data.len() + offset];
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
#[derive(Debug, Format, Clone, Copy, PartialEq, Eq)]
pub enum OtpWriteError {
    /// Payload will not fit in OTP.
    PayloadSize,
    /// Memory region is already occupied.
    Occupied,
}

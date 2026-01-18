//! # Vital Product Data
//!
//! This is the metadata written to the device during manufacturing that
//! provides useful information to the firmware.
//!
//! ## Writing Product Data
//!
//! <div class="warning">Once written, changing the VPD is not possible due to
//! the one-time-programmable memory it is written to.</div>
//!
//! To write VPD to a device, first fill out the `vpd.ron` file with your
//! desired data, then run `WRITE_VPD=vpd.ron cargo run` which will write the
//! encoded data to OTP memory and then startup normally.
//!
//! Once the VPD has been written for the first time, sucessive attempts to
//! write VPD will fail silently as the OTP memory is write-only.
//!
//! ## Fail-safety Behaviour
//!
//! The system implements fault tollerance by reverting back to defaults for
//! each key for corrupt or malformed data. This ensures that the device will
//! still sucessfully start up rather than being bricked.

use core::{convert::Infallible, fmt::Formatter};
use defmt::Format;
use tlvc::{TlvcReadError, TlvcReader};
use zerocopy::{AsBytes, FromBytes, FromZeroes};

/// Vital product data
#[derive(Debug, Format)]
#[repr(C)]
pub struct VitalProductData {
    pub serial: Serial,
    pub hardware: Version,
    pub sku: MaybeSku,
}

impl VitalProductData {
    /// Read TLV-C product data.
    ///
    /// If a tag is not presen, the default value for the type is used.
    pub fn from_tlvc(buf: &[u8]) -> Result<Self, TlvcReadError<Infallible>> {
        let mut serial = None;
        let mut version = None;
        let mut sku: Option<u8> = None;

        let mut reader = TlvcReader::begin(buf)?;
        while let Ok(Some(chunk)) = reader.next() {
            match &chunk.header().tag {
                b"SER " => serial = Self::process_chunk(&chunk)?,
                b"HW  " => version = Self::process_chunk(&chunk)?,
                b"SKU " => sku = Self::process_chunk(&chunk)?,
                _ => {} // do nothing for unknown tags
            }
        }

        Ok(Self {
            serial: serial.unwrap_or_default(),
            hardware: version.unwrap_or_default(),
            sku: MaybeSku::from(sku.unwrap_or_default()),
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
#[derive(Debug, Default, AsBytes, FromZeroes, FromBytes)]
#[repr(C)]
pub struct Version {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
    pub pre: u8,
}

impl Version {
    /// Assert size at compile time.
    const _SIZE: () = assert!(core::mem::size_of::<Self>() == 4);
}

impl defmt::Format for Version {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(fmt, "{}.{}.{}", self.major, self.minor, self.patch);
        if self.pre != 0 {
            defmt::write!(fmt, "-rc.{}", self.pre);
        }
    }
}

/// SKU identity
#[derive(Debug, Format)]
#[repr(u8)]
pub enum SkuId {
    M2KeyB = 1,
    MiniPCIe = 2,
}

impl TryFrom<u8> for SkuId {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            x if x == Self::M2KeyB as u8 => Ok(Self::M2KeyB),
            x if x == Self::MiniPCIe as u8 => Ok(Self::MiniPCIe),
            _ => Err(value),
        }
    }
}

#[derive(Debug, Format)]
pub enum MaybeSku {
    Known(SkuId),
    Unknown(u8),
}

impl From<u8> for MaybeSku {
    fn from(value: u8) -> Self {
        match SkuId::try_from(value) {
            Ok(sku) => Self::Known(sku),
            Err(sku) => Self::Unknown(sku),
        }
    }
}

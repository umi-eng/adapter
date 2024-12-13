//! Vital product data.

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
    pub sku: Sku,
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
            sku: Sku::from(sku.unwrap_or_default()),
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

/// SKU identity
#[derive(Debug, Format)]
#[repr(u8)]
pub enum SkuId {
    M2KeyE = 1,
    MiniPCIe = 2,
}

impl TryFrom<u8> for SkuId {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            x if x == Self::M2KeyE as u8 => Ok(Self::M2KeyE),
            x if x == Self::MiniPCIe as u8 => Ok(Self::MiniPCIe),
            _ => Err(value),
        }
    }
}

#[derive(Debug, Format)]
pub enum Sku {
    Known(SkuId),
    Unknown(u8),
}

impl From<u8> for Sku {
    fn from(value: u8) -> Self {
        match SkuId::try_from(value) {
            Ok(sku) => Self::Known(sku),
            Err(sku) => Self::Unknown(sku),
        }
    }
}

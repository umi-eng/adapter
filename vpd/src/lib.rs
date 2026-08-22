#![no_std]

#[cfg(feature = "host")]
extern crate alloc;

#[cfg(feature = "host")]
use alloc::vec::Vec;
use core::{convert::Infallible, fmt::Formatter};
#[cfg(feature = "host")]
use tlvc::{ChunkHeader, compute_body_crc};
use tlvc::{TlvcReadError, TlvcReader};
use zerocopy::{FromBytes, FromZeros, IntoBytes};

/// Start address of the VPD region in STM32G4 OTP memory.
pub const VPD_START_ADDRESS: u64 = 0x1FFF_7000;
/// Size of the VPD OTP region.
pub const OTP_SIZE: usize = 1024;

/// Vital product data.
#[derive(Debug)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(C)]
pub struct VitalProductData {
    pub serial: Serial,
    pub hardware: Version,
    pub sku: MaybeSku,
}

impl VitalProductData {
    /// Read TLV-C product data.
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
                _ => {}
            }
        }

        Ok(Self {
            serial: serial.unwrap_or_default(),
            hardware: version.unwrap_or_default(),
            sku: MaybeSku::from(sku.unwrap_or_default()),
        })
    }

    /// Encode VPD into TLV-C bytes.
    #[cfg(feature = "host")]
    pub fn to_tlvc(&self) -> Vec<u8> {
        let mut out = Vec::new();
        append_chunk(&mut out, *b"SER ", &self.serial.as_bytes());
        append_chunk(&mut out, *b"HW  ", &self.hardware.as_bytes());
        append_chunk(&mut out, *b"SKU ", &[self.sku.id()]);
        out
    }

    fn process_chunk<T: FromBytes + IntoBytes + FromZeros>(
        chunk: &tlvc::ChunkHandle<&[u8]>,
    ) -> Result<Option<T>, TlvcReadError<Infallible>> {
        if chunk.len() as usize != core::mem::size_of::<T>() {
            return Ok(None);
        }

        let mut checksum_buf = [0; 2];
        chunk.check_body_checksum(&mut checksum_buf)?;

        let mut out = T::new_zeroed();
        chunk.read_exact(0, out.as_mut_bytes())?;
        Ok(Some(out))
    }
}

#[cfg(feature = "host")]
fn append_chunk(out: &mut Vec<u8>, tag: [u8; 4], body: &[u8]) {
    let body_len = u32::try_from(body.len()).unwrap();
    let header = ChunkHeader {
        tag,
        len: body_len.into(),
        header_checksum: tlvc::header_checksum(tag, body_len).into(),
    };
    out.extend_from_slice(&header.tag);
    out.extend_from_slice(&body_len.to_le_bytes());
    out.extend_from_slice(&header.header_checksum.get().to_le_bytes());
    out.extend_from_slice(body);
    while out.len() % 4 != 0 {
        out.push(0);
    }
    out.extend_from_slice(&compute_body_crc(body).to_le_bytes());
}

/// Serial number.
#[derive(Debug, Default, IntoBytes, FromBytes)]
#[repr(C)]
pub struct Serial {
    pub year: u8,
    pub week: u8,
    pub seq: u16,
}

impl Serial {
    #[cfg(feature = "host")]
    fn as_bytes(&self) -> [u8; 4] {
        [self.year, self.week, self.seq as u8, (self.seq >> 8) as u8]
    }
}

#[cfg(feature = "defmt")]
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

/// Hardware version and manufacturing batch number.
#[derive(Debug, Default, IntoBytes, FromBytes)]
#[repr(C)]
pub struct Version {
    pub major: u8,
    pub minor: u8,
    pub patch: u8,
    pub batch: u8,
}

#[cfg(feature = "defmt")]
impl defmt::Format for Version {
    fn format(&self, fmt: defmt::Formatter) {
        defmt::write!(
            fmt,
            "{}.{}.{}-batch.{}",
            self.major,
            self.minor,
            self.patch,
            self.batch
        );
    }
}

impl Version {
    #[cfg(feature = "host")]
    fn as_bytes(&self) -> [u8; 4] {
        [self.major, self.minor, self.patch, self.batch]
    }
}

/// SKU identity.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
#[repr(u8)]
pub enum SkuId {
    M2KeyB = 1,
    MiniPCIe = 2,
    M2KeyE = 3,
}

impl core::fmt::Display for SkuId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::M2KeyB => "M.2 Key B",
            Self::MiniPCIe => "Mini PCIe",
            Self::M2KeyE => "M.2 Key E",
        })
    }
}

impl TryFrom<u8> for SkuId {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::M2KeyB),
            2 => Ok(Self::MiniPCIe),
            3 => Ok(Self::M2KeyE),
            _ => Err(value),
        }
    }
}

/// A recognized or unrecognized SKU value read from the device.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "defmt", derive(defmt::Format))]
pub enum MaybeSku {
    Known(SkuId),
    Unknown(u8),
}

impl MaybeSku {
    /// Return the numeric value stored in TLV-C.
    pub fn id(self) -> u8 {
        match self {
            Self::Known(sku) => sku as u8,
            Self::Unknown(value) => value,
        }
    }
}

impl From<u8> for MaybeSku {
    fn from(value: u8) -> Self {
        match SkuId::try_from(value) {
            Ok(sku) => Self::Known(sku),
            Err(sku) => Self::Unknown(sku),
        }
    }
}

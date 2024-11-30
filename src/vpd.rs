//! Vital product data.

use bitflags::bitflags;
use core::{convert::Infallible, fmt::Formatter, io::BorrowedBuf, slice};
use defmt::Format;
use tlvc::{ChunkHeader, TlvcReadError, TlvcReader};
use zerocopy::{AsBytes, FromBytes, FromZeroes};

static CRC: crc::Crc<u32> = crc::Crc::<u32>::new(&crc::CRC_32_ISCSI);

/// TLV-C chunk tag.
///
/// The last character is the version number used by the firmware to track
/// breaking changes to the VPD.
pub const TAG: [u8; 4] = *b"VPD0";

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
    /// Serializes the VPD into the provided buffer in TLV-C format, returning
    /// the number of bytes written.
    pub fn serialize(&self, buf: &mut [u8]) -> Result<usize, ()> {
        let mut borrowed_buf = BorrowedBuf::from(buf);
        let mut cursor = borrowed_buf.unfilled();

        let mut header = ChunkHeader {
            tag: TAG,
            len: (core::mem::size_of::<VitalProductData>() as u32).into(),
            header_checksum: 0.into(), // placeholder until we calculate this.
        };
        header.header_checksum = header.compute_checksum().into();
        cursor.append(header.as_bytes());

        cursor.append(self.as_bytes());

        let mut digest = CRC.digest();
        digest.update(self.as_bytes());
        let data_checksum = digest.finalize();
        cursor.append(&data_checksum.to_le_bytes());

        Ok(borrowed_buf.len())
    }

    /// Attempt to read the VPD from a byte slice.
    pub fn deserialize(
        buf: &[u8],
    ) -> Result<Option<Self>, TlvcReadError<Infallible>> {
        let mut reader = TlvcReader::begin(buf)?;

        let Some(chunk) = reader.next()? else {
            return Ok(None);
        };

        let mut temp = [0; 16];
        chunk.check_body_checksum(&mut temp)?;

        if chunk.header().tag != TAG {
            return Ok(None);
        }

        let mut this = VitalProductData::new_zeroed();
        chunk.read_exact(0, this.as_bytes_mut())?;
        Ok(Some(this))
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
#[derive(Debug, Format, AsBytes, FromZeroes, FromBytes)]
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

/// Reads the 1 kilobyte of OTP memory.
#[allow(unused)]
pub fn read_otp() -> &'static [u8] {
    const OTP_ADDRESS: *const u8 = 0x1FFF7000 as *const u8;
    const SLICE_LENGTH: usize = 1024; // 1 kilobyte

    unsafe { slice::from_raw_parts(OTP_ADDRESS, SLICE_LENGTH) }
}

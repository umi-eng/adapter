use anyhow::{Context, anyhow, bail};
use probe_rs::{MemoryInterface, Session, SessionConfig, config::Registry};
use std::cmp::max;
use tlvc::TlvcReader;
use vpd::{OTP_SIZE, VPD_START_ADDRESS, VitalProductData};

#[derive(Debug, clap::Parser)]
pub struct VpdRead {}

impl VpdRead {
    pub fn run(self) -> anyhow::Result<()> {
        let mut registry = Registry::new();
        registry
            .add_target_family_from_yaml(include_str!("../STM32G47x.yaml"))
            .context("failed to load STM32G47x target definition")?;

        let mut session = Session::auto_attach_with_registry(
            "STM32G47xCE",
            SessionConfig::default(),
            &registry,
        )
        .context("failed to attach to STM32G47x target")?;

        let mut otp = vec![0xff; OTP_SIZE];
        session
            .core(0)
            .context("failed to access target core")?
            .read(VPD_START_ADDRESS, &mut otp)
            .context("failed to read OTP")?;

        let used = otp
            .iter()
            .rposition(|&byte| byte != 0xff)
            .map_or(0, |index| index + 1);
        if used == 0 {
            bail!("OTP does not contain VPD data");
        }
        let data = &otp[..used];
        let vpd = VitalProductData::from_tlvc(data)
            .map_err(|error| anyhow!("failed to decode VPD: {error:?}"))?;

        println!("VPD:");
        println!("  Serial:   {}", vpd.serial);
        println!(
            "  Hardware: v{}.{}.{} (batch {})",
            vpd.hardware.major,
            vpd.hardware.minor,
            vpd.hardware.patch,
            vpd.hardware.batch
        );
        println!("  SKU:      {}", vpd.sku);

        let mut reader = TlvcReader::begin(data).map_err(|error| {
            anyhow!("failed to read VPD TLV-C stream: {error:?}")
        })?;

        while let Some(chunk) = reader.next().map_err(|error| {
            anyhow!("failed to read VPD TLV-C chunk: {error:?}")
        })? {
            let tag = chunk.header().tag;
            if tag == *b"SER " || tag == *b"HW  " || tag == *b"SKU " {
                continue;
            }

            let mut checksum_buffer = vec![0; max(1, chunk.len() as usize)];
            chunk.check_body_checksum(&mut checksum_buffer).map_err(
                |error| anyhow!("unknown tag has invalid checksum: {error:?}"),
            )?;
            let mut body = vec![0; chunk.len() as usize];
            chunk.read_exact(0, &mut body).map_err(|error| {
                anyhow!("failed to read unknown tag body: {error:?}")
            })?;

            println!(
                "  Unknown tag {}: {body:?}",
                String::from_utf8_lossy(&tag)
            );
        }

        Ok(())
    }
}

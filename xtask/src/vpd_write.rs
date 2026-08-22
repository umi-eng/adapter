use anyhow::{Context, anyhow, bail};
use probe_rs::{
    MemoryInterface, Session, SessionConfig, config::Registry,
    flashing::DownloadOptions,
};
use std::io::{self, Write};
use vpd::{
    MaybeSku, OTP_SIZE, Serial, SkuId, VPD_START_ADDRESS, Version,
    VitalProductData,
};

const DOUBLE_WORD_SIZE: usize = 8;

#[derive(Debug, clap::Parser)]
pub struct VpdWrite {
    /// Validate the target and show the write without programming OTP.
    #[arg(long)]
    dry_run: bool,
}

impl VpdWrite {
    pub fn run(self) -> anyhow::Result<()> {
        let serial = prompt("Serial number (YYWW-XXXX)", parse_serial)?;
        let hardware =
            prompt("Hardware version (vMAJOR.MINOR.PATCH)", parse_version)?;
        let batch = prompt("Hardware batch (0-255)", |input| {
            parse_decimal(input, "hardware batch")
        })?;
        let sku = prompt_sku()?;

        println!("\nThe following VPD will be programmed:");
        println!("  Serial:   {}", serial);
        println!(
            "  Hardware: v{}.{}.{} (batch {})",
            hardware.major, hardware.minor, hardware.patch, batch
        );
        println!("  SKU:      {sku}");
        if !confirm("Are you sure? [y/N]")? {
            println!("Aborted; OTP was not modified.");
            return Ok(());
        }

        let vpd = VitalProductData {
            serial,
            hardware: Version { batch, ..hardware },
            sku: MaybeSku::Known(sku),
        }
        .to_tlvc();
        let raw_len = vpd.len();
        let mut otp_data = vpd;
        pad_to_double_word(&mut otp_data);

        if otp_data.len() > OTP_SIZE {
            bail!(
                "VPD will not fit in OTP memory: {} > {} bytes",
                otp_data.len(),
                OTP_SIZE
            );
        }

        println!(
            "VPD: {raw_len} bytes ({} bytes after double-word padding)",
            otp_data.len()
        );
        println!("Data: {otp_data:?}");

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

        if !self.dry_run {
            // Never ask a flash algorithm to touch an occupied OTP range. This
            // makes a second manufacturing attempt fail safely and explicitly.
            let mut existing = vec![0xff; otp_data.len()];
            session
                .core(0)
                .context("failed to access target core")?
                .read(VPD_START_ADDRESS, &mut existing)
                .context("failed to read OTP")?;
            if existing.iter().any(|&byte| byte != 0xff) {
                bail!(
                    "OTP already contains data; refusing to program one-time memory"
                );
            }
        }

        println!("Writing {} bytes to OTP.", otp_data.len());
        let mut loader = session.target().flash_loader();
        loader
            .add_data(VPD_START_ADDRESS, &otp_data)
            .map_err(|e| anyhow!("failed to prepare OTP write: {e}"))?;

        let mut options = DownloadOptions::new();
        options.dry_run = self.dry_run;
        options.verify = true;
        if self.dry_run {
            println!("Dry run; OTP was not modified");
        }
        loader
            .commit(&mut session, options)
            .context("failed to write VPD to OTP")?;

        if !self.dry_run {
            let mut written = vec![0; otp_data.len()];
            session
                .core(0)
                .context("failed to access target core for verification")?
                .read(VPD_START_ADDRESS, &mut written)
                .context("failed to read OTP after writing")?;
            if written != otp_data {
                bail!(
                    "OTP verification failed: read-back data differs from VPD"
                );
            }
            println!("✓ VPD written and verified successfully");
        }

        Ok(())
    }
}

fn parse_serial(input: &str) -> anyhow::Result<Serial> {
    let (date, sequence) = input
        .split_once('-')
        .ok_or_else(|| anyhow!("expected serial in the form YYWW-XXXX"))?;
    if date.len() != 4
        || sequence.len() != 4
        || !date.is_ascii()
        || !sequence.is_ascii()
    {
        bail!("expected serial in the form YYWW-XXXX");
    }
    let year = parse_decimal(&date[..2], "serial year")?;
    let week = parse_decimal(&date[2..], "serial week")?;
    if !(1..=53).contains(&week) {
        bail!("serial week must be between 01 and 53");
    }
    let sequence = u16::from_str_radix(sequence, 16)
        .with_context(|| "serial sequence must be four hexadecimal digits")?;
    Ok(Serial {
        year,
        week,
        seq: sequence,
    })
}

fn parse_version(input: &str) -> anyhow::Result<Version> {
    let version = input
        .strip_prefix('v')
        .ok_or_else(|| anyhow!("hardware version must start with 'v'"))?;
    let parts: Vec<_> = version.split('.').collect();
    if parts.len() != 3 || parts.iter().any(|part| part.is_empty()) {
        bail!("expected hardware version in the form v0.0.0");
    }
    Ok(Version {
        major: parse_decimal(parts[0], "hardware major version")?,
        minor: parse_decimal(parts[1], "hardware minor version")?,
        patch: parse_decimal(parts[2], "hardware patch version")?,
        batch: 0,
    })
}

fn prompt<T>(
    label: &str,
    parser: fn(&str) -> anyhow::Result<T>,
) -> anyhow::Result<T> {
    loop {
        let input = read_line(&format!("{label}: "))?;
        match parser(&input) {
            Ok(value) => return Ok(value),
            Err(error) => eprintln!("Invalid input: {error}"),
        }
    }
}

fn prompt_sku() -> anyhow::Result<SkuId> {
    println!("Product variant:");
    println!("  1) M.2 Key B");
    println!("  2) Mini PCIe");
    println!("  3) M.2 Key E");
    loop {
        match read_line("Select SKU [1-3]: ")?.parse::<u8>() {
            Ok(1) => return Ok(SkuId::M2KeyB),
            Ok(2) => return Ok(SkuId::MiniPCIe),
            Ok(3) => return Ok(SkuId::M2KeyE),
            _ => eprintln!("Invalid SKU selection; choose 1, 2, or 3."),
        }
    }
}

fn confirm(label: &str) -> anyhow::Result<bool> {
    Ok(matches!(
        read_line(&format!("{label}: "))?.as_str(),
        "y" | "Y" | "yes" | "YES"
    ))
}

fn read_line(prompt: &str) -> anyhow::Result<String> {
    print!("{prompt}");
    io::stdout().flush().context("failed to flush prompt")?;
    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .context("failed to read input")?;
    Ok(input.trim().to_owned())
}

fn parse_decimal(input: &str, field: &str) -> anyhow::Result<u8> {
    if !input.chars().all(|character| character.is_ascii_digit()) {
        bail!("{field} must contain decimal digits only");
    }
    input
        .parse()
        .with_context(|| format!("{field} must fit in 0..255"))
}

fn pad_to_double_word(data: &mut Vec<u8>) {
    let padding =
        (DOUBLE_WORD_SIZE - data.len() % DOUBLE_WORD_SIZE) % DOUBLE_WORD_SIZE;
    data.resize(data.len() + padding, 0xff);
}

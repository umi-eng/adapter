mod switch_banks;
mod vpd_read;
mod vpd_write;

use clap::Parser;

#[derive(clap::Parser)]
enum Cli {
    /// Switch flash banks.
    SwitchBanks(switch_banks::SwitchBanks),
    /// Read vital product data from OTP memory.
    VpdRead(vpd_read::VpdRead),
    /// Write and verify vital product data in OTP memory.
    VpdWrite(vpd_write::VpdWrite),
}

fn main() -> anyhow::Result<()> {
    match Cli::parse() {
        Cli::SwitchBanks(cmd) => cmd.run(),
        Cli::VpdRead(cmd) => cmd.run(),
        Cli::VpdWrite(cmd) => cmd.run(),
    }
}

mod switch_banks;

use clap::Parser;

#[derive(clap::Parser)]
enum Cli {
    SwitchBanks(switch_banks::SwitchBanks),
}

fn main() -> anyhow::Result<()> {
    match Cli::parse() {
        Cli::SwitchBanks(cmd) => cmd.run(),
    }
}

use chrono::{DateTime, SecondsFormat, Utc};
use std::{
    fs::File, io::Write, path::PathBuf, process::Command, time::SystemTime,
};
use tlvc_text::{load, pack};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // get output directory
    let out = &PathBuf::from(std::env::var("OUT_DIR")?);

    // optionally inject vital product data
    println!("cargo:rerun-if-changed=vpd.ron");
    if let Some(path) = option_env!("WRITE_VPD") {
        let vpd_file = File::open(path)?;
        let vpd = pack(&load(vpd_file)?);
        File::create(out.join("vpd.bin"))?.write_all(&vpd)?;
    } else {
        // write empty file to satisfy clippy
        File::create(out.join("vpd.bin"))?.set_len(0)?;
    }

    // put `memory.x` in the output directory and ensure it's in the linker
    // search path.
    File::create(out.join("memory.x"))?
        .write_all(include_bytes!("memory.x"))?;
    println!("cargo:rustc-link-search={}", out.display());

    // inject compilation timestamp
    let date_time: DateTime<Utc> = SystemTime::now().into();
    println!(
        "cargo:rustc-env=CRATE_BUILT_AT={}",
        date_time.to_rfc3339_opts(SecondsFormat::Secs, true)
    );

    // inject git commit hash
    let git_hash = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()?
            .stdout,
    )?;
    println!("cargo:rustc-env=CRATE_GIT_HASH={}", git_hash);

    // ensure the project is rebuilt when memory.x is changed.
    println!("cargo:rerun-if-changed=memory.x");

    // rebuild when config is changed.
    println!("cargo:rerun-if-changed=.cargo/config.toml");

    // rebuild when this file is changed.
    println!("cargo:rerun-if-changed=build.rs");

    Ok(())
}

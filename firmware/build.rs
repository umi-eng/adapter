use jiff::Timestamp;
use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use tlvc_text::load;
use tlvc_text::pack;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // get output directory
    let out = &PathBuf::from(std::env::var("OUT_DIR")?);

    // optionally inject vital product data
    println!("cargo:rerun-if-changed=vpd.ron");
    if let Some(path) = option_env!("WRITE_VPD") {
        let vpd_file = File::open(path)?;
        let vpd = pack(&load(vpd_file)?);
        assert!(vpd.len() <= 1024, "VPD will not fit into OTP memory");
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
    let timestamp: Timestamp = Timestamp::now();
    println!("cargo:rustc-env=CRATE_BUILT_AT={}", timestamp.as_second());

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

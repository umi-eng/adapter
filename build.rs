use chrono::{DateTime, SecondsFormat, Utc};
use std::{
    fs::File, io::Write, path::PathBuf, process::Command, time::SystemTime,
};
use tlvc_text::{load, pack};

fn main() {
    // Get out directory.
    let out = &PathBuf::from(std::env::var_os("OUT_DIR").unwrap());

    // Optionally inject vital product data.
    println!("cargo:rerun-if-changed=vpd.ron");
    if let Some(path) = option_env!("WRITE_VPD") {
        let vpd_file = File::open(path).unwrap();
        let vpd = pack(&load(vpd_file).unwrap());
        File::create(out.join("vpd.bin"))
            .unwrap()
            .write_all(&vpd)
            .unwrap();
    }

    // put `memory.x` in our output directory and ensure it's on the linker
    // search path.
    File::create(out.join("memory.x"))
        .unwrap()
        .write_all(include_bytes!("memory.x"))
        .unwrap();
    println!("cargo:rustc-link-search={}", out.display());

    let date_time: DateTime<Utc> = SystemTime::now().into();
    println!(
        "cargo:rustc-env=CRATE_BUILT_AT={}",
        date_time.to_rfc3339_opts(SecondsFormat::Secs, true)
    );

    let git_hash = String::from_utf8(
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .output()
            .unwrap()
            .stdout,
    )
    .unwrap();
    println!("cargo:rustc-env=CRATE_GIT_HASH={}", git_hash);

    // ensure the project is rebuilt when memory.x is changed.
    println!("cargo:rerun-if-changed=memory.x");

    // rebuild when config is changed.
    println!("cargo:rerun-if-changed=.cargo/config.toml");

    // rebuild when this file is changed.
    println!("cargo:rerun-if-changed=build.rs");
}

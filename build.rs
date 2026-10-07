//! A compiled service-protocol identity for each build, including development
//! builds with the same package version. Do not identify a running image by
//! reopening its executable path: a rebuild can replace that path underneath it.

fn main() {
    for path in [
        "src",
        "vendor",
        "Cargo.toml",
        "Cargo.lock",
        "build.rs",
        ".cargo/config.toml",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    println!("cargo:rerun-if-env-changed=RUSTFLAGS");
    println!("cargo:rerun-if-env-changed=CARGO_ENCODED_RUSTFLAGS");
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the build clock must be after the Unix epoch")
        .as_nanos();
    println!(
        "cargo:rustc-env=TSPARSE_SERVICE_BUILD={timestamp:x}-{:x}",
        std::process::id()
    );
}

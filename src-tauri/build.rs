fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "windows")]
    println!(
        "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
    );

    embed_bundled_core_sha256()?;

    tonic_build::configure()
        .build_server(false)
        .compile(&["../proto/started_service.proto"], &["../proto"])?;
    tauri_build::build();
    Ok(())
}

/// Bake the SHA-256 of the bundled zeytun-core into the (code-signed) app
/// binary as `ZEYTUN_CORE_SHA256`. The privileged install compares against
/// this constant, not against the core file on disk, so swapping the core
/// inside the app bundle cannot also swap the hash it is checked against.
///
/// When the core is absent (fresh checkout without native resources) no
/// variable is emitted; release builds then refuse TUN instead of trusting
/// whatever file is on disk at runtime.
fn embed_bundled_core_sha256() -> Result<(), Box<dyn std::error::Error>> {
    use sha2::{Digest, Sha256};

    // Same key as `platform_key()` at runtime, but for the *target*.
    let os = std::env::var("CARGO_CFG_TARGET_OS")?;
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH")?;
    let binary = if os == "windows" {
        "zeytun-core.exe"
    } else {
        "zeytun-core"
    };
    let core = std::path::Path::new("resources")
        .join("bin")
        .join(format!("{os}-{arch}"))
        .join(binary);

    println!("cargo:rerun-if-changed={}", core.display());
    if !core.is_file() {
        println!(
            "cargo:warning=bundled core not found at {}; ZEYTUN_CORE_SHA256 not embedded",
            core.display()
        );
        return Ok(());
    }

    let digest = Sha256::digest(std::fs::read(&core)?);
    let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    println!("cargo:rustc-env=ZEYTUN_CORE_SHA256={hex}");
    Ok(())
}

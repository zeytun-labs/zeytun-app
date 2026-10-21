fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_os = "windows")]
    println!(
        "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
    );

    tonic_build::configure()
        .build_server(false)
        .compile(&["../proto/started_service.proto"], &["../proto"])?;
    tauri_build::build();
    Ok(())
}

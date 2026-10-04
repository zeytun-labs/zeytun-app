fn main() -> Result<(), Box<dyn std::error::Error>> {
    tonic_build::configure()
        .build_server(false)
        .compile(&["../proto/started_service.proto"], &["../proto"])?;
    tauri_build::build();
    Ok(())
}

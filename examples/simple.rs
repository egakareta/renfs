fn main() -> Result<(), Box<dyn std::error::Error>> {
    #[cfg(target_arch = "wasm32")]
    console_log::init_with_level(log::Level::Debug).unwrap();

    #[cfg(not(target_arch = "wasm32"))]
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Debug)
        .init();

    let app_dir = renfs::app_dir("test")?;
    app_dir.write_file("test.txt", b"hello")?;
    log::info!("{}", app_dir.read_text("test.txt")?);

    Ok(())
}

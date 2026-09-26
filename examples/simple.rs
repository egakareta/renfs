fn main() -> Result<(), Box<dyn std::error::Error>> {
    examplify::init().with_log_level(examplify::log::LevelFilter::Info);

    let app_dir = renfs::app_dir("test")?;

    let counter_file = "runs.txt";

    let count = match app_dir.read_text(counter_file) {
        Ok(text) => text.trim().parse::<u64>().unwrap_or(0),
        Err(_) => 0,
    };

    let count = count + 1;

    app_dir.write_file(counter_file, count.to_string().as_bytes())?;

    examplify::log::info!("Runs: {}", count);

    Ok(())
}

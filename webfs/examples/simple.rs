//! Run this in
//! - native `cargo run --example simple`
//! - web `trunk serve --example simple`

use examplify::log;
use webfs::fs;

fn main() -> Result<(), std::io::Error> {
    examplify::init().with_log_level(log::LevelFilter::Info);

    fs::create_dir_all("notes")?;
    fs::write("notes/today.txt", "Hello from the browser")?;
    let text = fs::read_to_string("notes/today.txt")?;
    log::info!("{}", text);

    Ok(())
}

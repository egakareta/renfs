fn main() {
    let _ = console_log::init_with_level(log::Level::Debug);

    renfs::write_file("test.txt", b"hello").unwrap();
    log::info!(
        "{}",
        String::from_utf8(renfs::read_file("test.txt").unwrap()).unwrap()
    );
}

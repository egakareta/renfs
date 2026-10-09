# webfs

`webfs` provides a synchronous `std::fs`-style API for browser WebAssembly,
backed by `localStorage`. On native targets it re-exports `std::fs`, so the same
imports work in native tools and browser builds.

```rust,no_run
use webfs::fs;

fn main() -> Result<(), std::io::Error> {
    fs::create_dir_all("notes")?;
    fs::write("notes/today.txt", "Hello from the browser")?;
    let text = fs::read_to_string("notes/today.txt")?;
    log::info!("{}", text);
    Ok(())
}
```

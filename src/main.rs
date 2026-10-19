#[cfg(not(all(target_family = "wasm", not(target_os = "wasi"))))]
mod cli;

#[cfg(not(all(target_family = "wasm", not(target_os = "wasi"))))]
fn main() -> std::process::ExitCode {
    match cli::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!("{error}");
            for (n, cause) in error.chain().enumerate().skip(1) {
                tracing::info!("  {n}: {cause}");
            }
            std::process::ExitCode::FAILURE
        }
    }
}

#[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
fn main() {}

#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

#[cfg(all(test, target_family = "wasm", not(target_os = "wasi")))]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg(all(target_family = "wasm", not(target_os = "wasi")))]
#[path = "web.rs"]
pub mod fs;

#[cfg(not(all(target_family = "wasm", not(target_os = "wasi"))))]
/// Filesystem operations. On native targets this is [`std::fs`].
pub mod fs {
    pub use std::fs::*;
}

/// Re-exports filesystem operations at the crate root for convenience.
pub use fs::*;

#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

#[cfg(all(test, target_arch = "wasm32"))]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg(target_arch = "wasm32")]
#[path = "web.rs"]
pub mod fs;

#[cfg(not(target_arch = "wasm32"))]
/// Filesystem operations. On native targets this is [`std::fs`].
pub mod fs {
    pub use std::fs::*;
}

/// Re-exports filesystem operations at the crate root for convenience.
pub use fs::*;

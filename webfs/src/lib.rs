//! A filesystem API backed by the browser's synchronous `localStorage` store.
//!
//! On `wasm32`, [`fs`] stores files and directories in local storage. The
//! implementation is synchronous, like [`std::fs`], and stores file contents
//! as Base64-encoded values. On native targets this crate re-exports
//! [`std::fs`] so applications can use the same imports in both environments.
//!
//! ```no_run
//! use webfs::fs;
//!
//! fs::create_dir_all("/notes")?;
//! fs::write("/notes/today.txt", "Hello from the browser")?;
//! let text = fs::read_to_string("/notes/today.txt")?;
//! # let _ = text;
//! # Ok::<(), std::io::Error>(())
//! ```

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

#![deny(missing_docs)]
#![doc = include_str!("../README.md")]

#[cfg(all(test, target_family = "wasm", not(target_os = "wasi")))]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg(not(all(target_family = "wasm", not(target_os = "wasi"))))]
mod common;

#[cfg(not(all(target_family = "wasm", not(target_os = "wasi"))))]
#[path = "native.rs"]
mod implementation;

#[cfg(all(
    target_family = "wasm",
    not(target_os = "wasi"),
    feature = "webfs",
    feature = "zenfs"
))]
#[path = "common.rs"]
mod webfs_backend;

#[cfg(all(
    target_family = "wasm",
    not(target_os = "wasi"),
    feature = "webfs",
    feature = "zenfs"
))]
#[path = "wasm.rs"]
mod zenfs;

#[cfg(all(
    target_family = "wasm",
    not(target_os = "wasi"),
    feature = "webfs",
    feature = "zenfs"
))]
#[path = "hybrid.rs"]
mod implementation;

#[cfg(all(
    target_family = "wasm",
    not(target_os = "wasi"),
    feature = "webfs",
    not(feature = "zenfs")
))]
#[path = "common.rs"]
mod implementation;

#[cfg(all(
    target_family = "wasm",
    not(target_os = "wasi"),
    feature = "webfs",
    not(feature = "zenfs")
))]
#[path = "wasm.rs"]
#[allow(dead_code)]
mod zenfs;

#[cfg(all(
    target_family = "wasm",
    not(target_os = "wasi"),
    any(not(feature = "webfs"), feature = "zenfs"),
    not(all(feature = "webfs", feature = "zenfs"))
))]
#[path = "wasm.rs"]
mod implementation;

#[cfg(all(
    target_family = "wasm",
    not(target_os = "wasi"),
    feature = "webfs",
    not(feature = "zenfs")
))]
use zenfs as wasm_api;

#[cfg(all(
    target_family = "wasm",
    not(target_os = "wasi"),
    feature = "webfs",
    feature = "zenfs"
))]
use zenfs as wasm_api;

#[cfg(all(
    target_family = "wasm",
    not(target_os = "wasi"),
    any(not(feature = "webfs"), feature = "zenfs"),
    not(all(feature = "webfs", feature = "zenfs"))
))]
use implementation as wasm_api;

mod api;
pub use api::fs::*;
pub use api::*;

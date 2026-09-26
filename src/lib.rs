#[cfg(target_arch = "wasm32")]
#[path = "wasm.rs"]
mod implementation;

#[cfg(not(target_arch = "wasm32"))]
#[path = "native.rs"]
mod implementation;

pub use implementation::*;

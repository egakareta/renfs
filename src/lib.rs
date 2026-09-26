#[cfg(target_arch = "wasm32")]
#[path = "wasm.rs"]
mod implementation;

#[cfg(not(target_arch = "wasm32"))]
#[path = "native.rs"]
mod implementation;

mod api;

pub use api::fs;
pub use api::fs::{
    create_dir, create_dir_all, exists, read_dir, read_file, read_text, remove_file, write_file,
    write_text,
};
pub use api::{Directory, Error, FileOps, Result, app_dir};

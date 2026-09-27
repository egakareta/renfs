use std::path::{Component, Path, PathBuf};

use super::implementation;

#[cfg(not(target_arch = "wasm32"))]
pub type Error = std::io::Error;

#[cfg(target_arch = "wasm32")]
#[derive(Debug)]
pub struct Error(wasm_bindgen::JsValue);

#[cfg(target_arch = "wasm32")]
impl From<wasm_bindgen::JsValue> for Error {
    fn from(value: wasm_bindgen::JsValue) -> Self {
        Self(value)
    }
}

#[cfg(target_arch = "wasm32")]
impl std::fmt::Display for Error {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(message) = self.0.as_string() {
            formatter.write_str(&message)
        } else {
            write!(formatter, "JavaScript filesystem error: {:?}", self.0)
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl std::error::Error for Error {}

pub type Result<T> = std::result::Result<T, Error>;

fn invalid_path_error(message: &str) -> Error {
    implementation::invalid_path_error(message).into()
}

/// A filesystem directory whose operations use paths relative to its root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Directory {
    path: String,
}

impl Directory {
    /// Wraps a path as a directory. This does not create or verify the path.
    pub fn new(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref().to_str().ok_or_else(|| {
            implementation::invalid_path_error("directory path is not valid UTF-8")
        })?;

        Ok(Self {
            path: path.to_owned(),
        })
    }

    /// Returns the directory's root path.
    pub fn as_path(&self) -> &str {
        &self.path
    }

    fn resolve(&self, relative_path: &str) -> Result<String> {
        let relative = Path::new(relative_path);
        if relative_path.contains('\\')
            || relative.is_absolute()
            || relative.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            return Err(invalid_path_error(
                "directory operations require a relative path that stays inside the directory",
            ));
        }

        let path = PathBuf::from(&self.path).join(relative);
        path.to_str()
            .map(str::to_owned)
            .ok_or_else(|| invalid_path_error("resolved path is not valid UTF-8"))
    }
}

struct RootFileSystem;

macro_rules! define_file_api {
    ($( $operation:ident([$($path:ident),+ $(,)?] $(, $arg:ident : $arg_type:ty)* $(,)?) -> $output:ty; )*) => {
        pub trait FileOps {
            fn resolve_path(&self, path: &str) -> Result<String>;

            $(
                fn $operation(&self, $($path: &str),* $(, $arg: $arg_type)*) -> Result<$output> {
                    $(let $path = self.resolve_path($path)?;)*
                    Ok(implementation::$operation($(&$path,)* $($arg),*)?)
                }
            )*
        }

        impl Directory {
            $(
                pub fn $operation(&self, $($path: &str),* $(, $arg: $arg_type)*) -> Result<$output> {
                    FileOps::$operation(self, $($path,)* $($arg),*)
                }
            )*
        }

        pub mod fs {
            use super::{FileOps, Result, RootFileSystem};

            $(
                pub fn $operation($($path: &str),* $(, $arg: $arg_type)*) -> Result<$output> {
                    FileOps::$operation(&RootFileSystem, $($path,)* $($arg),*)
                }
            )*
        }
    };
}

define_file_api! {
    append_text([path], contents: &str) -> ();
    exists([path]) -> bool;
    read_text([path]) -> String;
    write_text([path], contents: &str) -> ();
    read_file([path]) -> Vec<u8>;
    write_file([path], contents: &[u8]) -> ();
    create_dir([path]) -> ();
    create_dir_all([path]) -> ();
    read_dir([path]) -> Vec<String>;
    remove_file([path]) -> ();
    rename([from, to]) -> ();
}

impl FileOps for Directory {
    fn resolve_path(&self, path: &str) -> Result<String> {
        self.resolve(path)
    }
}

impl FileOps for RootFileSystem {
    fn resolve_path(&self, path: &str) -> Result<String> {
        Ok(path.to_owned())
    }
}

pub fn app_dir(name: &str) -> Result<Directory> {
    let path = implementation::app_dir(name)?;
    Ok(Directory { path })
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn directory_and_fs_share_file_operations() {
        let temp = tempdir().unwrap();
        let dir = Directory::new(temp.path()).unwrap();
        let full_path = temp.path().join("hello.txt");
        let full_path = full_path.to_str().unwrap();

        dir.write_text("hello.txt", "from directory").unwrap();
        assert_eq!(fs::read_text(full_path).unwrap(), "from directory");

        fs::write_text(full_path, "from fs").unwrap();
        assert_eq!(dir.read_text("hello.txt").unwrap(), "from fs");
    }

    #[test]
    fn append_text_creates_and_appends_to_files() {
        let temp = tempdir().unwrap();
        let dir = Directory::new(temp.path()).unwrap();
        let full_path = temp.path().join("hello.txt");
        let full_path = full_path.to_str().unwrap();

        dir.append_text("hello.txt", "from directory").unwrap();
        fs::append_text(full_path, " and fs").unwrap();

        assert_eq!(dir.read_text("hello.txt").unwrap(), "from directory and fs");
    }

    #[test]
    fn rename_is_available_on_fs_and_directory() {
        let temp = tempdir().unwrap();
        let dir = Directory::new(temp.path()).unwrap();
        dir.write_text("before.txt", "from directory").unwrap();
        dir.rename("before.txt", "after.txt").unwrap();
        assert!(!dir.exists("before.txt").unwrap());
        assert_eq!(dir.read_text("after.txt").unwrap(), "from directory");

        let root_before = temp.path().join("after.txt");
        let root_after = temp.path().join("root-renamed.txt");
        fs::rename(root_before.to_str().unwrap(), root_after.to_str().unwrap()).unwrap();
        assert!(!root_before.exists());
        assert!(root_after.exists());
    }

    #[test]
    fn directory_rejects_paths_that_escape_its_root() {
        let temp = tempdir().unwrap();
        let dir = Directory::new(temp.path()).unwrap();

        assert_eq!(
            dir.write_text("../outside.txt", "nope").unwrap_err().kind(),
            std::io::ErrorKind::InvalidInput
        );
        assert_eq!(
            dir.rename("inside.txt", "../outside.txt")
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::InvalidInput
        );
    }
}

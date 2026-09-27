use std::path::{Component, Path, PathBuf};

use super::implementation;

#[cfg(not(target_arch = "wasm32"))]
/// The error type returned by filesystem operations on native platforms.
pub type Error = std::io::Error;

#[cfg(target_arch = "wasm32")]
/// The error type returned by filesystem operations on WebAssembly.
///
/// This type wraps the JavaScript value thrown by the underlying filesystem.
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

/// The result type returned by filesystem operations.
pub type Result<T> = std::result::Result<T, Error>;

fn invalid_path_error(message: &str) -> Error {
    #[cfg(not(target_arch = "wasm32"))]
    {
        implementation::invalid_path_error(message)
    }

    #[cfg(target_arch = "wasm32")]
    {
        implementation::invalid_path_error(message).into()
    }
}

macro_rules! implementation_result {
    ($operation:expr) => {{
        #[cfg(not(target_arch = "wasm32"))]
        {
            $operation
        }

        #[cfg(target_arch = "wasm32")]
        {
            $operation.map_err(Into::into)
        }
    }};
}

macro_rules! define_file_handle_api {
    ($( $(#[$documentation:meta])* $operation:ident => $sync_operation:ident
        ($($argument:ident: $argument_type:ty),*) -> $output:ty {
            async { $async_body:expr }
            sync { $sync_body:expr }
        }; )*) => {
        $(
            $(#[$documentation])*
            pub async fn $operation($($argument: $argument_type),*) -> Result<$output> {
                $async_body.await
            }

            $(#[$documentation])*
            #[doc = ""]
            #[doc = concat!("Synchronously performs `", stringify!($operation), "`.")]
            pub fn $sync_operation($($argument: $argument_type),*) -> Result<$output> {
                $sync_body
            }
        )*
    };
}

macro_rules! define_file_methods {
    (
        async_mutable {
            $( $(#[$async_documentation:meta])* $async_method:ident($($async_argument:ident: $async_argument_type:ty),*) -> $async_output:ty = $async_implementation:ident; )*
        }
        sync_mutable {
            $( $(#[$sync_documentation:meta])* $sync_method:ident($($sync_argument:ident: $sync_argument_type:ty),*) -> $sync_output:ty = $sync_implementation:ident; )*
        }
        async_shared {
            $( $(#[$async_shared_documentation:meta])* $async_shared_method:ident($($async_shared_argument:ident: $async_shared_argument_type:ty),*) -> $async_shared_output:ty = $async_shared_implementation:ident; )*
        }
        sync_shared {
            $( $(#[$sync_shared_documentation:meta])* $sync_shared_method:ident($($sync_shared_argument:ident: $sync_shared_argument_type:ty),*) -> $sync_shared_output:ty = $sync_shared_implementation:ident; )*
        }
    ) => {
        $(
            $(#[$async_documentation])*
            pub async fn $async_method(&mut self, $($async_argument: $async_argument_type),*) -> Result<$async_output> {
                implementation_result!(implementation::$async_implementation(self.inner_mut()? $(, $async_argument)*).await)
            }
        )*

        $(
            #[doc = concat!("Synchronous filesystem operation `", stringify!($sync_method), "`.")]
            $(#[$sync_documentation])*
            pub fn $sync_method(&mut self, $($sync_argument: $sync_argument_type),*) -> Result<$sync_output> {
                implementation_result!(implementation::$sync_implementation(self.inner_mut()? $(, $sync_argument)*))
            }
        )*

        $(
            $(#[$async_shared_documentation])*
            pub async fn $async_shared_method(&self, $($async_shared_argument: $async_shared_argument_type),*) -> Result<$async_shared_output> {
                let inner = self.inner.as_ref().ok_or_else(|| {
                    invalid_path_error("operation attempted on a closed file")
                })?;
                implementation_result!(implementation::$async_shared_implementation(inner $(, $async_shared_argument)*).await)
            }
        )*

        $(
            #[doc = concat!("Synchronous filesystem operation `", stringify!($sync_shared_method), "`.")]
            $(#[$sync_shared_documentation])*
            pub fn $sync_shared_method(&self, $($sync_shared_argument: $sync_shared_argument_type),*) -> Result<$sync_shared_output> {
                let inner = self.inner.as_ref().ok_or_else(|| {
                    invalid_path_error("operation attempted on a closed file")
                })?;
                implementation_result!(implementation::$sync_shared_implementation(inner $(, $sync_shared_argument)*))
            }
        )*
    };
}

/// A filesystem directory whose operations use paths relative to its root.
///
/// Methods on this type reject absolute paths and paths that escape the root.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Directory {
    path: String,
}

impl Directory {
    /// Wraps a path as a directory.
    ///
    /// This does not create or verify the path.
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

/// Options and flags which can be used to configure how a file is opened.
///
/// This builder exposes the ability to configure how a file is opened and
/// what operations are permitted on the open file.
///
/// All options are initially set to `false`.
#[derive(Clone, Debug, Default)]
pub struct OpenOptions {
    pub(crate) read: bool,
    pub(crate) write: bool,
    pub(crate) append: bool,
    pub(crate) truncate: bool,
    pub(crate) create: bool,
    pub(crate) create_new: bool,
}

impl OpenOptions {
    /// Creates a blank new set of options ready for configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the option for read access.
    ///
    /// This option, when true, will indicate that the file should be
    /// readable if opened.
    pub fn read(&mut self, read: bool) -> &mut Self {
        self.read = read;
        self
    }

    /// Sets the option for write access.
    ///
    /// If the file already exists, any write calls on it will overwrite its
    /// contents, without truncating it.
    pub fn write(&mut self, write: bool) -> &mut Self {
        self.write = write;
        self
    }

    /// Sets the option for the append mode.
    ///
    /// This option, when true, means that writes will append to a file instead
    /// of overwriting previous contents.
    ///
    /// Note
    /// This function doesn’t create the file if it doesn’t exist. Use the
    /// `OpenOptions::create` method to do so.
    pub fn append(&mut self, append: bool) -> &mut Self {
        self.append = append;
        self
    }

    /// Sets the option for truncating a previous file.
    ///
    /// If a file is successfully opened with this option set to true, it will truncate
    /// the file to 0 length if it already exists.
    ///
    /// The file must be opened with write access for truncate to work.
    pub fn truncate(&mut self, truncate: bool) -> &mut Self {
        self.truncate = truncate;
        self
    }

    /// Sets the option to create a new file, or open it if it already exists.
    ///
    /// In order for the file to be created, `OpenOptions::write` or
    /// `OpenOptions::append` access must be used.
    ///
    /// If `.create(true)` is set without `.write(true)` or `.append(true)`, calling open will
    /// fail with an `InvalidInput` error.
    pub fn create(&mut self, create: bool) -> &mut Self {
        self.create = create;
        self
    }

    /// Sets the option to create a new file, failing if it already exists.
    ///
    /// No file is allowed to exist at the target location, also no (dangling) symlink. In this
    /// way, if the call succeeds, the file returned is guaranteed to be new.
    ///
    /// If `.create_new(true)` is set, `.create()` and `.truncate()` are ignored.
    ///
    /// The file must be opened with write or append access in order to create a new file.
    pub fn create_new(&mut self, create_new: bool) -> &mut Self {
        self.create_new = create_new;
        self
    }
}

/// An object providing access to an open file on the filesystem.
///
/// Files are automatically closed when they go out of scope. Errors detected
/// on closing are ignored by the implementation of `Drop`.
///
/// Use [`fs::close`] to close explicitly and observe close errors.
pub struct File {
    inner: Option<implementation::File>,
}

impl std::fmt::Debug for File {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("File").finish_non_exhaustive()
    }
}

impl File {
    fn from_inner(inner: implementation::File) -> Self {
        Self { inner: Some(inner) }
    }

    fn inner_mut(&mut self) -> Result<&mut implementation::File> {
        self.inner
            .as_mut()
            .ok_or_else(|| invalid_path_error("operation attempted on a closed file"))
    }

    fn take_inner(&mut self) -> Result<implementation::File> {
        self.inner
            .take()
            .ok_or_else(|| invalid_path_error("operation attempted on a closed file"))
    }

    define_file_methods! {
        async_mutable {
            /// Reads bytes from the file into the provided buffer.
            read(buffer: &mut [u8]) -> usize = read;
            /// Writes bytes from the provided buffer to the file.
            write(buffer: &[u8]) -> usize = write;
            /// Reads from multiple buffers.
            read_vectored(buffers: &mut [std::io::IoSliceMut<'_>]) -> usize = readv;
            /// Writes from multiple buffers.
            write_vectored(buffers: &[std::io::IoSlice<'_>]) -> usize = writev;
            /// Attempts to sync all OS-internal file content and metadata to disk.
            ///
            /// This function will attempt to ensure that all in-memory data reaches the
            /// filesystem before returning.
            sync_all() -> () = fsync;
            /// This function is similar to `sync_all`, except that it might not
            /// synchronize file metadata to the filesystem.
            ///
            /// This is intended for use cases that must synchronize content, but don’t
            /// need the metadata on disk. The goal of this method is to reduce disk
            /// operations.
            ///
            /// Note that some platforms may simply implement this in terms of
            /// `sync_all`.
            sync_data() -> () = fdatasync;
            /// Changes the size of the file.
            ///
            /// Truncates or extends the underlying file, updating the size of
            /// this file to become size.
            /// If the size is less than the current file’s size, then the file will
            /// be shrunk. If it is greater than the current file’s size, then the file
            /// will be extended to size and have all of the intermediate data filled
            /// in with 0s.
            set_len(size: u64) -> () = ftruncate;
        }
        sync_mutable {
            read_sync(buffer: &mut [u8]) -> usize = read_sync;
            write_sync(buffer: &[u8]) -> usize = write_sync;
            read_vectored_sync(buffers: &mut [std::io::IoSliceMut<'_>]) -> usize = readv_sync;
            write_vectored_sync(buffers: &[std::io::IoSlice<'_>]) -> usize = writev_sync;
            sync_all_sync() -> () = fsync_sync;
            sync_data_sync() -> () = fdatasync_sync;
            set_len_sync(size: u64) -> () = ftruncate_sync;
        }
        async_shared {
            /// Queries metadata about the underlying file.
            metadata() -> Metadata = fstat;
        }
        sync_shared {
            metadata_sync() -> Metadata = fstat_sync;
        }
    }
}

impl Drop for File {
    fn drop(&mut self) {
        if let Some(inner) = self.inner.take() {
            let _ = implementation::close_sync(inner);
        }
    }
}

/// Metadata about an open file.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Metadata {
    len: u64,
    is_file: bool,
    is_dir: bool,
    is_symlink: bool,
}

impl Metadata {
    pub(crate) fn from_parts(len: u64, is_file: bool, is_dir: bool, is_symlink: bool) -> Self {
        Self {
            len,
            is_file,
            is_dir,
            is_symlink,
        }
    }

    /// Returns the size of the file, in bytes, this metadata is for.
    pub fn len(&self) -> u64 {
        self.len
    }

    /// Returns `true` if the file size is zero bytes.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns `true` if this metadata is for a regular file.
    pub fn is_file(&self) -> bool {
        self.is_file
    }

    /// Returns `true` if this metadata is for a directory.
    pub fn is_dir(&self) -> bool {
        self.is_dir
    }

    /// Returns `true` if this metadata is for a symbolic link.
    pub fn is_symlink(&self) -> bool {
        self.is_symlink
    }
}

/// An in-memory snapshot of a file's contents returned by [`fs::open_as_blob`].
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Blob {
    bytes: Vec<u8>,
}

impl Blob {
    fn new(bytes: Vec<u8>) -> Self {
        Self { bytes }
    }

    /// Returns the size of the blob in bytes.
    pub fn size(&self) -> usize {
        self.bytes.len()
    }

    /// Returns `true` if the blob contains no bytes.
    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Returns the blob's bytes.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Consumes the blob and returns its bytes.
    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

macro_rules! define_file_api {
    (
        operations {
            $( $(#[$documentation:meta])* $operation:ident([$($path:ident),+ $(,)?] $(, $arg:ident : $arg_type:ty)* $(,)?) -> $output:ty; )*
        }
        async_operations {
            $( $(#[$async_documentation:meta])* $operation_async:ident([$($async_path:ident),+ $(,)?] $(, $async_arg:ident : $async_arg_type:ty)* $(,)?) -> $async_output:ty => $sync_operation:ident; )*
        }
    ) => {
        /// Filesystem operations that resolve paths before accessing the implementation.
        #[allow(async_fn_in_trait)]
        pub trait FileOps {
            /// Resolves a path for the filesystem implementation.
            fn resolve_path(&self, path: &str) -> Result<String>;

            /// Opens a file with the provided options.
            async fn open(&self, path: &str, options: &OpenOptions) -> Result<File> {
                let path = self.resolve_path(path)?;
                let inner = implementation::open(&path, options).await?;
                Ok(File::from_inner(inner))
            }

            /// Opens a file synchronously with the provided options.
            fn open_sync(&self, path: &str, options: &OpenOptions) -> Result<File> {
                let path = self.resolve_path(path)?;
                let inner = implementation::open_sync(&path, options)?;
                Ok(File::from_inner(inner))
            }

            $(
                $(#[$documentation])*
                fn $operation(&self, $($path: &str),* $(, $arg: $arg_type)*) -> Result<$output> {
                    $(let $path = self.resolve_path($path)?;)*
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        implementation::$operation($(&$path,)* $($arg),*)
                    }

                    #[cfg(target_arch = "wasm32")]
                    {
                        implementation::$operation($(&$path,)* $($arg),*).map_err(Into::into)
                    }
                }
            )*

            $(
                $(#[$async_documentation])*
                async fn $operation_async(&self, $($async_path: &str),* $(, $async_arg: $async_arg_type)*) -> Result<$async_output> {
                    $(let $async_path = self.resolve_path($async_path)?;)*
                    implementation_result!(implementation::$operation_async($(&$async_path,)* $($async_arg),*).await)
                }

                $(#[$async_documentation])*
                #[doc = ""]
                #[doc = concat!("Synchronously performs `", stringify!($operation_async), "`.")]
                fn $sync_operation(&self, $($async_path: &str),* $(, $async_arg: $async_arg_type)*) -> Result<$async_output> {
                    $(let $async_path = self.resolve_path($async_path)?;)*
                    implementation_result!(implementation::$sync_operation($(&$async_path,)* $($async_arg),*))
                }
            )*

        }

        impl Directory {
            /// Opens a file with the provided options.
            pub async fn open(&self, path: &str, options: &OpenOptions) -> Result<File> {
                FileOps::open(self, path, options).await
            }

            /// Opens a file synchronously with the provided options.
            pub fn open_sync(&self, path: &str, options: &OpenOptions) -> Result<File> {
                FileOps::open_sync(self, path, options)
            }

            /// Opens a file and returns a snapshot of its contents.
            pub async fn open_as_blob(&self, path: &str) -> Result<Blob> {
                let path = self.resolve(path)?;
                open_as_blob_path(&path).await
            }

            $(
                $(#[$documentation])*
                pub fn $operation(&self, $($path: &str),* $(, $arg: $arg_type)*) -> Result<$output> {
                    FileOps::$operation(self, $($path,)* $($arg),*)
                }
            )*

            $(
                $(#[$async_documentation])*
                pub async fn $operation_async(&self, $($async_path: &str),* $(, $async_arg: $async_arg_type)*) -> Result<$async_output> {
                    FileOps::$operation_async(self, $($async_path,)* $($async_arg),*).await
                }

                $(#[$async_documentation])*
                #[doc = ""]
                #[doc = concat!("Synchronously performs `", stringify!($operation_async), "`.")]
                pub fn $sync_operation(&self, $($async_path: &str),* $(, $async_arg: $async_arg_type)*) -> Result<$async_output> {
                    FileOps::$sync_operation(self, $($async_path,)* $($async_arg),*)
                }
            )*

        }

        /// Filesystem manipulation operations.
        pub mod fs {
            use super::{
                Blob, File, FileOps, Metadata, OpenOptions, Result, RootFileSystem,
            };

            /// Check whether a path exists, without checking permissions.
            pub const F_OK: u32 = 0;
            /// Check whether a path is readable.
            pub const R_OK: u32 = 4;
            /// Check whether a path is writable.
            pub const W_OK: u32 = 2;
            /// Check whether a path is executable.
            pub const X_OK: u32 = 1;

            /// Opens a file with the provided options.
            pub async fn open(path: &str, options: &OpenOptions) -> Result<File> {
                FileOps::open(&RootFileSystem, path, options).await
            }

            /// Opens a file synchronously with the provided options.
            pub fn open_sync(path: &str, options: &OpenOptions) -> Result<File> {
                FileOps::open_sync(&RootFileSystem, path, options)
            }

            /// Opens a file and returns a snapshot of its contents.
            pub async fn open_as_blob(path: &str) -> Result<Blob> {
                super::open_as_blob_path(path).await
            }

            $(
                $(#[$documentation])*
                pub fn $operation($($path: &str),* $(, $arg: $arg_type)*) -> Result<$output> {
                    FileOps::$operation(&RootFileSystem, $($path,)* $($arg),*)
                }

            )*

            $(
                $(#[$async_documentation])*
                pub async fn $operation_async($($async_path: &str),* $(, $async_arg: $async_arg_type)*) -> Result<$async_output> {
                    FileOps::$operation_async(&RootFileSystem, $($async_path,)* $($async_arg),*).await
                }

                $(#[$async_documentation])*
                #[doc = ""]
                #[doc = concat!("Synchronously performs `", stringify!($operation_async), "`.")]
                pub fn $sync_operation($($async_path: &str),* $(, $async_arg: $async_arg_type)*) -> Result<$async_output> {
                    FileOps::$sync_operation(&RootFileSystem, $($async_path,)* $($async_arg),*)
                }
            )*

            define_file_handle_api! {
                /// Closes an open file.
                close => close_sync(file: File) -> () {
                    async { super::close_file(file) }
                    sync { super::close_file_sync(file) }
                };
                /// Reads bytes from an open file into the provided buffer.
                read => read_sync(file: &mut File, buffer: &mut [u8]) -> usize {
                    async { file.read(buffer) }
                    sync { file.read_sync(buffer) }
                };
                /// Writes bytes from the provided buffer to an open file.
                write => write_sync(file: &mut File, buffer: &[u8]) -> usize {
                    async { file.write(buffer) }
                    sync { file.write_sync(buffer) }
                };
                /// Reads from multiple buffers.
                readv => readv_sync(file: &mut File, buffers: &mut [std::io::IoSliceMut<'_>]) -> usize {
                    async { file.read_vectored(buffers) }
                    sync { file.read_vectored_sync(buffers) }
                };
                /// Writes from multiple buffers.
                writev => writev_sync(file: &mut File, buffers: &[std::io::IoSlice<'_>]) -> usize {
                    async { file.write_vectored(buffers) }
                    sync { file.write_vectored_sync(buffers) }
                };
                /// Returns metadata for an open file.
                fstat => fstat_sync(file: &File) -> Metadata {
                    async { file.metadata() }
                    sync { file.metadata_sync() }
                };
                /// Synchronizes all file content and metadata to the filesystem.
                fsync => fsync_sync(file: &mut File) -> () {
                    async { file.sync_all() }
                    sync { file.sync_all_sync() }
                };
                /// Synchronizes file content to the filesystem.
                fdatasync => fdatasync_sync(file: &mut File) -> () {
                    async { file.sync_data() }
                    sync { file.sync_data_sync() }
                };
                /// Changes the size of an open file.
                ftruncate => ftruncate_sync(file: &mut File, size: u64) -> () {
                    async { file.set_len(size) }
                    sync { file.set_len_sync(size) }
                };
            }
        }
    };
}

define_file_api! {
  operations {
    /// Appends a string slice to a file.
    ///
    /// Creates the file if it does not exist.
    append_text([path], contents: &str) -> ();
    /// Reads the entire contents of a file into a string.
    read_text([path]) -> String;
    /// Writes a slice as the entire contents of a file.
    ///
    /// This function will create a file if it does not exist,
    /// and will entirely replace its contents if it does.
    write_text([path], contents: &str) -> ();
    /// Creates a new, empty directory at the provided path.
    create_dir([path]) -> ();
    /// Recursively create a directory and all of its parent components if they are missing.
    create_dir_all([path]) -> ();
    /// Returns the names of the entries within a directory.
    ///
    /// The entries are collected into a vector, and names that are not valid UTF-8 cause an error.
    read_dir([path]) -> Vec<String>;
    /// Removes a file from the filesystem.
    remove_file([path]) -> ();
    /// Renames a file or directory to a new name, replacing the original file if `to` already exists.
    rename([from, to]) -> ();
  }
  async_operations {
    /// Checks whether a path can be accessed with the requested mode.
    ///
    /// Use `F_OK`, `R_OK`, `W_OK`, and `X_OK` to select
    /// existence, read, write, and execute checks. These values can be combined.
    access([path], mode: u32) -> () => access_sync;
    /// Appends a byte slice to a file, creating it if it does not exist.
    append_file([path], contents: &[u8]) -> () => append_file_sync;
    /// Returns `Ok(true)` if the path points at an existing entity.
    ///
    /// An error is returned if the filesystem cannot determine whether the path exists.
    exists([path]) -> bool => exists_sync;
    /// Reads the entire contents of a file into a bytes vector.
    ///
    /// This is a convenience function for using `File::open` and `read_to_end`
    /// with fewer imports and without an intermediate variable.
    read_file([path]) -> Vec<u8> => read_file_sync;
    /// Writes a slice as the entire contents of a file.
    ///
    /// This function will create a file if it does not exist,
    /// and will entirely replace its contents if it does.
    write_file([path], contents: &[u8]) -> () => write_file_sync;
    /// Copies the contents of one file to another. This function will also
    /// copy the permission bits of the original file to the destination file.
    ///
    /// This function will overwrite the contents of the destination.
    copy_file([from, to]) -> () => copy_file_sync;
    /// Recursively copies a file or directory to a new path.
    cp([from, to]) -> () => cp_sync;
    /// Truncates a file to the specified length.
    truncate([path], len: u64) -> () => truncate_sync;
  }
}

fn close_file_sync(mut file: File) -> Result<()> {
    let inner = file.take_inner()?;
    implementation::close_sync(inner)?;
    Ok(())
}

async fn close_file(mut file: File) -> Result<()> {
    let inner = file
        .inner
        .as_mut()
        .ok_or_else(|| invalid_path_error("operation attempted on a closed file"))?;
    implementation::close(inner).await?;
    let _inner = file.inner.take();
    Ok(())
}

async fn open_as_blob_path(path: &str) -> Result<Blob> {
    let bytes = implementation::open_as_blob(path).await?;
    Ok(Blob::new(bytes))
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

/// Creates and returns a directory for the named application.
///
/// The application directory is created if it does not already exist. `name` must be a single
/// path component.
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
        assert!(!dir.exists_sync("before.txt").unwrap());
        assert_eq!(dir.read_text("after.txt").unwrap(), "from directory");

        let root_before = temp.path().join("after.txt");
        let root_after = temp.path().join("root-renamed.txt");
        fs::rename(root_before.to_str().unwrap(), root_after.to_str().unwrap()).unwrap();
        assert!(!root_before.exists());
        assert!(root_after.exists());
    }

    #[test]
    fn copy_file_is_available_on_fs_and_directory() {
        let temp = tempdir().unwrap();
        let dir = Directory::new(temp.path()).unwrap();
        dir.write_text("source.txt", "from directory").unwrap();
        dir.copy_file_sync("source.txt", "copy.txt").unwrap();
        assert_eq!(dir.read_text("copy.txt").unwrap(), "from directory");

        let root_source = temp.path().join("copy.txt");
        let root_copy = temp.path().join("root-copy.txt");
        fs::copy_file_sync(root_source.to_str().unwrap(), root_copy.to_str().unwrap()).unwrap();
        assert_eq!(
            fs::read_text(root_copy.to_str().unwrap()).unwrap(),
            "from directory"
        );
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
        assert_eq!(
            dir.copy_file_sync("inside.txt", "../outside.txt")
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::InvalidInput
        );
    }
}

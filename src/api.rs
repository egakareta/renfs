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

/// An opened directory, yielding its entries one at a time.
#[derive(Debug)]
pub struct Dir {
    entries: std::vec::IntoIter<String>,
}

impl Iterator for Dir {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        self.entries.next()
    }
}

impl Dir {
    pub(crate) fn from_names(names: Vec<String>) -> Self {
        Self {
            entries: names.into_iter(),
        }
    }
}

/// A temporary directory removed when this value is dropped.
#[derive(Debug)]
pub struct TempDir {
    pub(crate) path: String,
}

impl TempDir {
    /// Returns the path of the temporary directory.
    pub fn path(&self) -> &str {
        &self.path
    }

    /// Keeps the directory on disk and returns its path.
    pub fn keep(mut self) -> String {
        std::mem::take(&mut self.path)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        if !self.path.is_empty() {
            let _ = implementation::rm_sync(&self.path, true, true);
        }
    }
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
            /// Changes the permissions of the open file.
            fchmod(mode: u32) -> () = fchmod;
            /// Changes the owner and group of the open file.
            fchown(uid: u32, gid: u32) -> () = fchown;
            /// Changes the access and modification times of the open file.
            futimes(atime: std::time::SystemTime, mtime: std::time::SystemTime) -> () = futimes;
        }
        sync_shared {
            metadata_sync() -> Metadata = fstat_sync;
            fchmod_sync(mode: u32) -> () = fchmod_sync;
            fchown_sync(uid: u32, gid: u32) -> () = fchown_sync;
            futimes_sync(atime: std::time::SystemTime, mtime: std::time::SystemTime) -> () = futimes_sync;
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

/// Metadata about a filesystem entry.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Metadata {
    len: u64,
    is_file: bool,
    is_dir: bool,
    is_symlink: bool,
}

/// Information about the filesystem containing a path.
#[derive(Clone, Debug, PartialEq)]
pub struct StatFs {
    /// Fundamental filesystem block size in bytes.
    pub block_size: u64,
    /// Total blocks in the filesystem.
    pub blocks: f64,
    /// Free blocks in the filesystem.
    pub blocks_free: f64,
    /// Blocks available to unprivileged users.
    pub blocks_available: f64,
    /// Total file nodes in the filesystem.
    pub files: f64,
    /// Free file nodes in the filesystem.
    pub files_free: f64,
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
                Blob, Dir, File, FileOps, Metadata, OpenOptions, Result, RootFileSystem, StatFs, TempDir,
            };

            #[cfg(target_arch = "wasm32")]
            use wasm_bindgen::JsValue;
            use crate::implementation;

            /// Check whether a path exists, without checking permissions.
            pub const F_OK: u32 = 0;
            /// Check whether a path is readable.
            pub const R_OK: u32 = 4;
            /// Check whether a path is writable.
            pub const W_OK: u32 = 2;
            /// Check whether a path is executable.
            pub const X_OK: u32 = 1;

            /// Configures ZenFS with the supplied configuration, including asynchronous backends.
            #[cfg(target_arch = "wasm32")]
            pub async fn configure(configuration: &JsValue) -> Result<()> {
                implementation::configure(configuration).await?;
                Ok(())
            }

            /// Configures ZenFS synchronously. This fails if a configured backend requires async setup.
            #[cfg(target_arch = "wasm32")]
            pub fn configure_sync(configuration: &JsValue) -> Result<()> {
                implementation::configure_sync(configuration)?;
                Ok(())
            }

            /// Configures ZenFS with one backend mounted at `/`, including asynchronous backends.
            #[cfg(target_arch = "wasm32")]
            pub async fn configure_single(configuration: &JsValue) -> Result<()> {
                implementation::configure_single(configuration).await?;
                Ok(())
            }

            /// Configures ZenFS synchronously with one backend mounted at `/`.
            #[cfg(target_arch = "wasm32")]
            pub fn configure_single_sync(configuration: &JsValue) -> Result<()> {
                implementation::configure_single_sync(configuration)?;
                Ok(())
            }

            /// Applies shared configuration options to an already-created filesystem.
            #[cfg(target_arch = "wasm32")]
            pub fn configure_file_system(
                filesystem: &JsValue,
                configuration: &JsValue,
            ) -> Result<()> {
                implementation::configure_file_system(filesystem, configuration)?;
                Ok(())
            }

            /// Resolves a backend configuration asynchronously into a filesystem instance.
            #[cfg(target_arch = "wasm32")]
            pub async fn resolve_mount_config(configuration: &JsValue) -> Result<JsValue> {
                implementation::resolve_mount_config(configuration)
                    .await
                    .map_err(Into::into)
            }

            /// Resolves a backend configuration synchronously into a filesystem instance.
            #[cfg(target_arch = "wasm32")]
            pub fn resolve_mount_config_sync(configuration: &JsValue) -> Result<JsValue> {
                implementation::resolve_mount_config_sync(configuration).map_err(Into::into)
            }

            /// Resolves a remote filesystem asynchronously using a channel and mount configuration.
            #[cfg(target_arch = "wasm32")]
            pub async fn resolve_remote_mount(
                channel: &JsValue,
                configuration: &JsValue,
            ) -> Result<JsValue> {
                implementation::resolve_remote_mount(channel, configuration)
                    .await
                    .map_err(Into::into)
            }

            /// Mounts an existing filesystem at `mount_point`.
            #[cfg(target_arch = "wasm32")]
            pub fn mount(mount_point: &str, filesystem: &JsValue) -> Result<()> {
                implementation::mount(mount_point, filesystem)?;
                Ok(())
            }

            /// Unmounts the filesystem at `mount_point`.
            #[cfg(target_arch = "wasm32")]
            pub fn umount(mount_point: &str) -> Result<()> {
                implementation::umount(mount_point)?;
                Ok(())
            }

            /// Returns ZenFS's map of mount points to filesystem instances.
            #[cfg(target_arch = "wasm32")]
            pub fn mounts() -> JsValue {
                implementation::mounts()
            }

            /// Flushes all mounted filesystems.
            #[cfg(target_arch = "wasm32")]
            pub async fn sync() -> Result<()> {
                implementation::sync().await?;
                Ok(())
            }

            /// Waits for a worker to come online.
            #[cfg(target_arch = "wasm32")]
            pub async fn wait_online(worker: &JsValue) -> Result<()> {
                implementation::wait_online(worker).await?;
                Ok(())
            }

            /// Attaches a filesystem to a channel or RPC port.
            #[cfg(target_arch = "wasm32")]
            pub fn attach_fs(channel: &JsValue, filesystem: &JsValue) -> Result<()> {
                implementation::attach_fs(channel, filesystem)?;
                Ok(())
            }

            /// Detaches a filesystem from a channel or RPC port.
            #[cfg(target_arch = "wasm32")]
            pub fn detach_fs(channel: &JsValue, filesystem: &JsValue) -> Result<()> {
                implementation::detach_fs(channel, filesystem)?;
                Ok(())
            }

            /// Returns ZenFS's `fs` object.
            #[cfg(target_arch = "wasm32")]
            pub fn core_fs() -> JsValue {
                implementation::zenfs_fs()
            }

            /// Returns ZenFS's promise-based filesystem API.
            #[cfg(target_arch = "wasm32")]
            pub fn promises() -> JsValue {
                implementation::promises()
            }

            /// Returns ZenFS's default filesystem object.
            #[cfg(target_arch = "wasm32")]
            pub fn default_fs() -> JsValue {
                implementation::default_fs()
            }

            /// Returns ZenFS's virtual filesystem module.
            #[cfg(target_arch = "wasm32")]
            pub fn vfs() -> JsValue {
                implementation::vfs()
            }

            /// Returns the ZenFS version value exported by `@zenfs/core`.
            #[cfg(target_arch = "wasm32")]
            pub fn version() -> JsValue {
                implementation::version()
            }

            /// Returns ZenFS's filesystem constants object.
            #[cfg(target_arch = "wasm32")]
            pub fn constants() -> JsValue {
                implementation::constants()
            }

            /// Converts a glob pattern into a regular expression source.
            pub fn glob_to_regex(pattern: &str) -> Result<String> {
                implementation_result!(implementation::glob_to_regex(pattern))
            }

            /// Normalizes the components of a path.
            pub fn normalize_path(path: &str) -> Result<String> {
                implementation_result!(implementation::normalize_path(path))
            }

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
                /// Changes the permissions of an open file.
                fchmod => fchmod_sync(file: &File, mode: u32) -> () {
                    async { file.fchmod(mode) }
                    sync { file.fchmod_sync(mode) }
                };
                /// Changes the owner and group of an open file.
                fchown => fchown_sync(file: &File, uid: u32, gid: u32) -> () {
                    async { file.fchown(uid, gid) }
                    sync { file.fchown_sync(uid, gid) }
                };
                /// Changes the access and modification times of an open file.
                futimes => futimes_sync(file: &File, atime: std::time::SystemTime, mtime: std::time::SystemTime) -> () {
                    async { file.futimes(atime, mtime) }
                    sync { file.futimes_sync(atime, mtime) }
                };
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
    /// Creates a temporary directory removed when its guard is dropped.
    mkdtemp_disposable_sync([prefix]) -> TempDir;
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
    /// Creates a directory, creating missing parent directories when `recursive` is true.
    mkdir([path], recursive: bool) -> () => mkdir_sync;
    /// Creates a uniquely named temporary directory by appending to `prefix`.
    mkdtemp([prefix]) -> String => mkdtemp_sync;
    /// Opens a directory for iteration over its entry names.
    opendir([path]) -> Dir => opendir_sync;
    /// Reads the names of the entries in a directory.
    readdir([path]) -> Vec<String> => readdir_sync;
    /// Removes an empty directory.
    rmdir([path]) -> () => rmdir_sync;
    /// Removes a file or directory, optionally recursively or ignoring missing paths.
    rm([path], recursive: bool, force: bool) -> () => rm_sync;
    /// Returns the canonical, absolute form of a path with all intermediate
    /// components normalized and symbolic links resolved.
    realpath([path]) -> String => realpath_sync;
    /// Returns paths matching a glob pattern.
    glob([path]) -> Vec<String> => glob_sync;
    /// Given a path, queries the file system to get information about a file, directory, etc.
    stat([path]) -> Metadata => stat_sync;
    /// Queries the metadata about a file without following symlinks.
    lstat([path]) -> Metadata => lstat_sync;
    /// Returns information about the filesystem containing the specified path.
    statfs([path]) -> StatFs => statfs_sync;
    /// Changes the permissions found on a file or a directory.
    chmod([path], mode: u32) -> () => chmod_sync;
    /// Changes the permissions of a symbolic link itself (if supported).
    lchmod([path], mode: u32) -> () => lchmod_sync;
    /// Changes the owner and group of a file or directory.
    chown([path], uid: u32, gid: u32) -> () => chown_sync;
    /// Changes the owner and group of a symbolic link itself.
    lchown([path], uid: u32, gid: u32) -> () => lchown_sync;
    /// Changes the timestamps of the file or directory at the specified path.
    utimes([path], atime: std::time::SystemTime, mtime: std::time::SystemTime) -> () => utimes_sync;
    /// Changes the access and modification times of a symbolic link itself.
    lutimes([path], atime: std::time::SystemTime, mtime: std::time::SystemTime) -> () => lutimes_sync;
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

    #[cfg(unix)]
    #[test]
    fn metadata_stat_and_times_cover_path_and_descriptor_variants() {
        use std::time::{Duration, UNIX_EPOCH};
        let temp = tempdir().unwrap();
        let dir = Directory::new(temp.path()).unwrap();
        dir.write_text("file", "hello").unwrap();
        assert_eq!(dir.stat_sync("file").unwrap().len(), 5);
        assert!(dir.lstat_sync("file").unwrap().is_file());
        assert!(dir.statfs_sync("file").unwrap().block_size > 0);
        let atime = UNIX_EPOCH + Duration::from_secs(1_600_000_000);
        let mtime = atime + Duration::from_secs(100);
        dir.utimes_sync("file", atime, mtime).unwrap();
        let path = temp.path().join("file");
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), mtime);
        let mut options = OpenOptions::new();
        options.read(true);
        let file = dir.open_sync("file", &options).unwrap();
        fs::futimes_sync(&file, atime, atime).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().modified().unwrap(), atime);
        assert_eq!(
            dir.stat_sync("../file").unwrap_err().kind(),
            std::io::ErrorKind::InvalidInput
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::{PermissionsExt, symlink};
            let link = temp.path().join("link");
            symlink("file", &link).unwrap();
            assert!(dir.lstat_sync("link").unwrap().is_symlink());
            assert!(dir.stat_sync("link").unwrap().is_file());
            dir.lutimes_sync("link", atime, mtime).unwrap();
            assert_eq!(
                std::fs::symlink_metadata(&link)
                    .unwrap()
                    .modified()
                    .unwrap(),
                mtime
            );
            dir.chmod_sync("file", 0o640).unwrap();
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o640
            );
            fs::fchmod_sync(&file, 0o600).unwrap();
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
            let uid = unsafe { libc::geteuid() };
            let gid = unsafe { libc::getegid() };
            dir.chown_sync("file", uid, gid).unwrap();
            dir.lchown_sync("link", uid, gid).unwrap();
            fs::fchown_sync(&file, uid, gid).unwrap();
            // Some Unix filesystems cannot change a symlink's permissions.
            let _ = dir.lchmod_sync("link", 0o777);
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn async_metadata_operations_work() {
        futures_lite::future::block_on(async {
            let temp = tempdir().unwrap();
            let dir = Directory::new(temp.path()).unwrap();
            dir.write_text("file", "hi").unwrap();
            assert_eq!(dir.stat("file").await.unwrap().len(), 2);
            assert!(dir.lstat("file").await.unwrap().is_file());
            assert!(dir.statfs("file").await.unwrap().blocks > 0.0);
            let time = std::time::UNIX_EPOCH + std::time::Duration::from_secs(1_600_000_000);
            dir.utimes("file", time, time).await.unwrap();
            #[cfg(unix)]
            {
                std::os::unix::fs::symlink("file", temp.path().join("link")).unwrap();
                dir.lutimes("link", time, time).await.unwrap();
                dir.chmod("file", 0o600).await.unwrap();
                let uid = unsafe { libc::geteuid() };
                let gid = unsafe { libc::getegid() };
                dir.chown("file", uid, gid).await.unwrap();
                dir.lchown("link", uid, gid).await.unwrap();
                let _ = dir.lchmod("link", 0o777).await;
            }
            let mut options = OpenOptions::new();
            options.read(true);
            let file = dir.open("file", &options).await.unwrap();
            file.futimes(time, time).await.unwrap();
            #[cfg(unix)]
            {
                file.fchmod(0o600).await.unwrap();
                file.fchown(unsafe { libc::geteuid() }, unsafe { libc::getegid() })
                    .await
                    .unwrap();
            }
        });
    }

    #[test]
    fn directory_apis_cover_creation_listing_removal_and_paths() {
        let temp = tempdir().unwrap();
        let dir = Directory::new(temp.path()).unwrap();
        dir.mkdir_sync("nested/deep", true).unwrap();
        assert_eq!(dir.readdir_sync("nested").unwrap(), vec!["deep"]);
        assert_eq!(
            dir.opendir_sync("nested").unwrap().collect::<Vec<_>>(),
            vec!["deep"]
        );
        assert_eq!(
            dir.realpath_sync("nested/deep").unwrap(),
            temp.path().join("nested/deep").to_str().unwrap()
        );
        assert_eq!(
            dir.glob_sync("nested/*").unwrap(),
            vec![temp.path().join("nested/deep").to_str().unwrap()]
        );
        assert_eq!(
            dir.mkdir_sync("nested", false).unwrap_err().kind(),
            std::io::ErrorKind::AlreadyExists
        );
        dir.rmdir_sync("nested/deep").unwrap();
        dir.rm_sync("nested", true, false).unwrap();
        dir.rm_sync("nested", false, true).unwrap();
        assert_eq!(
            dir.rm_sync("missing", false, false).unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );
        assert_eq!(
            dir.rm_sync("../outside", true, true).unwrap_err().kind(),
            std::io::ErrorKind::InvalidInput
        );
        assert_eq!(fs::normalize_path("/a/./b/../c").unwrap(), "/a/c");
        assert_eq!(fs::glob_to_regex("*.txt").unwrap(), "^[^/]*\\.txt$");
    }

    #[test]
    fn temporary_directories_are_unique_and_disposable() {
        let temp = tempdir().unwrap();
        let prefix = temp.path().join("example-");
        let prefix = prefix.to_str().unwrap();
        let first = fs::mkdtemp_sync(prefix).unwrap();
        let second = fs::mkdtemp_sync(prefix).unwrap();
        assert_ne!(first, second);
        assert!(Path::new(&first).is_dir());
        let disposable = fs::mkdtemp_disposable_sync(prefix).unwrap();
        let disposable_path = disposable.path().to_owned();
        assert!(Path::new(&disposable_path).is_dir());
        drop(disposable);
        assert!(!Path::new(&disposable_path).exists());
    }

    #[test]
    fn async_directory_operations_work() {
        futures_lite::future::block_on(async {
            let temp = tempdir().unwrap();
            let dir = Directory::new(temp.path()).unwrap();
            dir.mkdir("nested", false).await.unwrap();
            assert_eq!(dir.readdir(".").await.unwrap(), vec!["nested"]);
            assert_eq!(
                dir.opendir(".").await.unwrap().collect::<Vec<_>>(),
                vec!["nested"]
            );
            assert_eq!(dir.glob("nest*").await.unwrap().len(), 1);
            assert!(dir.realpath("nested").await.unwrap().ends_with("nested"));
            let prefix = temp.path().join("tmp-");
            let created = fs::mkdtemp(prefix.to_str().unwrap()).await.unwrap();
            assert!(Path::new(&created).is_dir());
            dir.rmdir("nested").await.unwrap();
            fs::rm(&created, true, false).await.unwrap();
        });
    }

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

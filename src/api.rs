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

macro_rules! define_resource_handle {
    ($(#[$documentation:meta])* $name:ident($inner:ty) {
        from_inner $visibility:vis,
        drop $drop:expr;
    }) => {
        $(#[$documentation])*
        pub struct $name {
            inner: Option<$inner>,
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.debug_struct(stringify!($name)).finish_non_exhaustive()
            }
        }

        impl $name {
            $visibility fn from_inner(inner: $inner) -> Self {
                Self { inner: Some(inner) }
            }
        }

        impl Drop for $name {
            fn drop(&mut self) {
                if let Some(inner) = self.inner.take() {
                    let _ = ($drop)(inner);
                }
            }
        }
    };
}

macro_rules! define_bool_accessors {
    ($($(#[$documentation:meta])* $field:ident;)*) => {
        $(
            $(#[$documentation])*
            pub fn $field(&self) -> bool {
                self.$field
            }
        )*
    };
}

macro_rules! define_bool_option_setters {
    ($($(#[$documentation:meta])* $field:ident;)*) => {
        $(
            $(#[$documentation])*
            pub fn $field(&mut self, $field: bool) -> &mut Self {
                self.$field = $field;
                self
            }
        )*
    };
}

macro_rules! define_wasm_sync_wrappers {
    ($($(#[$documentation:meta])* $name:ident($($argument:ident: $argument_type:ty),*) -> $output:ty = $implementation:ident;)*) => {
        $(
            #[cfg(target_arch = "wasm32")]
            $(#[$documentation])*
            pub fn $name($($argument: $argument_type),*) -> Result<$output> {
                implementation_result!(implementation::$implementation($($argument),*))
            }
        )*
    };
}

macro_rules! define_wasm_async_wrappers {
    ($($(#[$documentation:meta])* $name:ident($($argument:ident: $argument_type:ty),*) -> $output:ty = $implementation:ident;)*) => {
        $(
            #[cfg(target_arch = "wasm32")]
            $(#[$documentation])*
            pub async fn $name($($argument: $argument_type),*) -> Result<$output> {
                implementation_result!(implementation::$implementation($($argument),*).await)
            }
        )*
    };
}

macro_rules! define_wasm_value_wrappers {
    ($($(#[$documentation:meta])* $name:ident() -> $output:ty = $implementation:ident;)*) => {
        $(
            #[cfg(target_arch = "wasm32")]
            $(#[$documentation])*
            pub fn $name() -> $output {
                implementation::$implementation()
            }
        )*
    };
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

define_resource_handle! {
    /// An open directory handle. Read entries one at a time and close it when finished.
    Dir(implementation::Dir) {
        from_inner pub(crate),
        drop |inner| implementation::dir_close_sync(&inner);
    }
}

impl Dir {
    /// Reads the next directory entry, or `None` when the directory is exhausted.
    pub fn read_sync(&mut self) -> Result<Option<Dirent>> {
        let inner = self
            .inner
            .as_mut()
            .ok_or_else(|| invalid_path_error("directory is closed"))?;
        implementation_result!(implementation::dir_read_sync(inner))
    }

    /// Reads the next directory entry, or `None` when the directory is exhausted.
    pub async fn read(&mut self) -> Result<Option<Dirent>> {
        let inner = self
            .inner
            .as_mut()
            .ok_or_else(|| invalid_path_error("directory is closed"))?;
        implementation_result!(implementation::dir_read(inner).await)
    }

    /// Closes the directory.
    pub fn close_sync(&mut self) -> Result<()> {
        let inner = self
            .inner
            .as_ref()
            .ok_or_else(|| invalid_path_error("directory is closed"))?;
        implementation::dir_close_sync(inner)?;
        self.inner.take();
        Ok(())
    }

    /// Closes the directory.
    pub async fn close(&mut self) -> Result<()> {
        let inner = self
            .inner
            .as_ref()
            .ok_or_else(|| invalid_path_error("directory is closed"))?;
        implementation::dir_close(inner).await?;
        self.inner.take();
        Ok(())
    }
}

/// A directory entry returned by [`Dir::read`] or [`Dir::read_sync`].
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Dirent {
    name: String,
    is_file: bool,
    is_dir: bool,
    is_symlink: bool,
}

impl Dirent {
    pub(crate) fn from_parts(name: String, is_file: bool, is_dir: bool, is_symlink: bool) -> Self {
        Self {
            name,
            is_file,
            is_dir,
            is_symlink,
        }
    }

    /// Returns the name of this entry.
    pub fn name(&self) -> &str {
        &self.name
    }
    define_bool_accessors! {
        /// Returns whether this entry is a regular file.
        is_file;
        /// Returns whether this entry is a directory.
        is_dir;
        /// Returns whether this entry is a symbolic link.
        is_symlink;
    }
}

define_resource_handle! {
    /// A filesystem watcher returned by [`fs::watch`].
    Watcher(implementation::Watcher) {
        from_inner,
        drop |inner| implementation::watch_close(&inner);
    }
}

impl Watcher {
    /// Stops watching for changes. Dropping the watcher also closes it.
    pub fn close(&mut self) -> Result<()> {
        let inner = self
            .inner
            .as_ref()
            .ok_or_else(|| invalid_path_error("watcher is closed"))?;
        implementation::watch_close(inner)?;
        self.inner.take();
        Ok(())
    }
}

/// Options for watching file or directory changes.
#[derive(Clone, Copy, Debug)]
pub struct WatchOptions {
    /// Whether to watch descendants of a directory as well.
    pub recursive: bool,
    /// Whether the watcher keeps the JavaScript event loop alive (wasm only).
    pub persistent: bool,
}

impl Default for WatchOptions {
    fn default() -> Self {
        Self {
            recursive: false,
            persistent: true,
        }
    }
}

/// Options for polling changes to a file's metadata.
#[derive(Clone, Copy, Debug)]
pub struct WatchFileOptions {
    /// Polling interval; ZenFS defaults to 5007 milliseconds.
    pub interval: std::time::Duration,
    /// Whether the watcher keeps the JavaScript event loop alive (wasm only).
    pub persistent: bool,
}

/// Options for a streaming read. `end` is an inclusive byte offset.
#[derive(Clone, Copy, Debug)]
pub struct ReadStreamOptions {
    /// First byte to read.
    pub start: u64,
    /// Last byte to read, inclusive. `None` reads to end of file.
    pub end: Option<u64>,
    /// Maximum bytes per chunk.
    pub chunk_size: usize,
}

impl Default for ReadStreamOptions {
    fn default() -> Self {
        Self {
            start: 0,
            end: None,
            chunk_size: 64 * 1024,
        }
    }
}

/// Options for a streaming write. ZenFS's `createWriteStream` always opens
/// with `w`, truncating the file before applying `start`.
#[derive(Clone, Copy, Debug, Default)]
pub struct WriteStreamOptions {
    /// Byte position where writing begins after truncation.
    pub start: Option<u64>,
}

define_resource_handle! {
    /// A readable byte stream. Dropping it destroys the stream.
    ReadStream(implementation::ReadStream) {
        from_inner,
        drop |inner| implementation::read_stream_destroy(&inner);
    }
}

impl ReadStream {
    /// Reads the next chunk, or `None` at end of stream.
    pub async fn read(&mut self) -> Result<Option<Vec<u8>>> {
        let inner = self
            .inner
            .as_mut()
            .ok_or_else(|| invalid_path_error("read stream is destroyed"))?;
        implementation_result!(implementation::stream_read(inner).await)
    }

    /// Destroys the stream and releases its resources.
    pub fn destroy(&mut self) -> Result<()> {
        let inner = self
            .inner
            .as_ref()
            .ok_or_else(|| invalid_path_error("read stream is destroyed"))?;
        implementation::read_stream_destroy(inner)?;
        self.inner.take();
        Ok(())
    }
}

define_resource_handle! {
    /// A writable byte stream. Call [`WriteStream::end`] to finish writing.
    WriteStream(implementation::WriteStream) {
        from_inner,
        drop |inner| implementation::write_stream_destroy(&inner);
    }
}

impl WriteStream {
    /// Writes all bytes, waiting until the stream has handled the chunk.
    pub async fn write(&mut self, bytes: &[u8]) -> Result<()> {
        let inner = self
            .inner
            .as_mut()
            .ok_or_else(|| invalid_path_error("write stream is closed"))?;
        implementation_result!(implementation::stream_write(inner, bytes).await)
    }

    /// Finishes writing and closes the stream.
    pub async fn end(&mut self) -> Result<()> {
        let inner = self
            .inner
            .as_mut()
            .ok_or_else(|| invalid_path_error("write stream is closed"))?;
        implementation::stream_end(inner).await?;
        self.inner.take();
        Ok(())
    }

    /// Destroys the stream without waiting for pending writes to finish.
    pub fn destroy(&mut self) -> Result<()> {
        let inner = self
            .inner
            .as_ref()
            .ok_or_else(|| invalid_path_error("write stream is closed"))?;
        implementation::write_stream_destroy(inner)?;
        self.inner.take();
        Ok(())
    }
}

impl Default for WatchFileOptions {
    fn default() -> Self {
        Self {
            interval: std::time::Duration::from_millis(5007),
            persistent: true,
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

    // The target is stored as given, relative to the link's parent. Reject
    // paths that would lexically escape this directory's root.
    fn symlink_paths(&self, target: &str, link: &str) -> Result<(String, String)> {
        let link = self.resolve(link)?;
        let relative_link = Path::new(link.as_str());
        let root = Path::new(&self.path);
        let mut depth = relative_link
            .strip_prefix(root)
            .map_err(|_| invalid_path_error("link is outside the directory"))?
            .parent()
            .map_or(0, |parent| {
                parent
                    .components()
                    .filter(|c| matches!(c, Component::Normal(_)))
                    .count()
            });
        if target.contains('\\') || Path::new(target).is_absolute() {
            return Err(invalid_path_error(
                "symbolic link target must stay inside the directory",
            ));
        }
        for component in Path::new(target).components() {
            match component {
                Component::Normal(_) => depth += 1,
                Component::ParentDir if depth > 0 => depth -= 1,
                Component::CurDir => (),
                _ => {
                    return Err(invalid_path_error(
                        "symbolic link target must stay inside the directory",
                    ));
                }
            }
        }
        Ok((target.to_owned(), link))
    }

    /// Creates a new symbolic link on the filesystem.
    ///
    /// `link` is relative to this directory's root. `target` is stored verbatim
    /// and interpreted relative to the link's parent, as with ZenFS.
    pub fn symlink_sync(&self, target: &str, link: &str) -> Result<()> {
        let (target, link) = self.symlink_paths(target, link)?;
        implementation_result!(implementation::symlink_sync(&target, &link))
    }

    /// Creates a new symbolic link on the filesystem.
    ///
    /// `link` is relative to this directory's root; `target` is relative to
    /// the link's parent and must stay inside this directory.
    pub async fn symlink(&self, target: &str, link: &str) -> Result<()> {
        let (target, link) = self.symlink_paths(target, link)?;
        implementation_result!(implementation::symlink(&target, &link).await)
    }

    /// Watches a file or directory for changes.
    pub fn watch(
        &self,
        path: &str,
        listener: impl FnMut(&str, &str) + Send + 'static,
    ) -> Result<Watcher> {
        self.watch_with_options(path, WatchOptions::default(), listener)
    }

    /// Watches a file or directory with the provided options.
    pub fn watch_with_options(
        &self,
        path: &str,
        options: WatchOptions,
        listener: impl FnMut(&str, &str) + Send + 'static,
    ) -> Result<Watcher> {
        fs::watch_with_options(&self.resolve(path)?, options, listener)
    }

    /// Watches changes to a file using polling. Returns an ID for removing this listener.
    pub fn watch_file(
        &self,
        path: &str,
        listener: impl FnMut(Metadata, Metadata) + Send + 'static,
    ) -> Result<u64> {
        self.watch_file_with_options(path, WatchFileOptions::default(), listener)
    }

    /// Watches changes to a file with the provided polling options.
    pub fn watch_file_with_options(
        &self,
        path: &str,
        options: WatchFileOptions,
        listener: impl FnMut(Metadata, Metadata) + Send + 'static,
    ) -> Result<u64> {
        fs::watch_file_with_options(&self.resolve(path)?, options, listener)
    }

    /// Stops all `watch_file` listeners for a path.
    pub fn unwatch_file(&self, path: &str) -> Result<()> {
        fs::unwatch_file(&self.resolve(path)?)
    }

    /// Stops the `watch_file` listener with the given ID for a path.
    pub fn unwatch_file_listener(&self, path: &str, id: u64) -> Result<()> {
        fs::unwatch_file_listener(&self.resolve(path)?, id)
    }

    /// Opens a file as a readable stream.
    pub fn create_read_stream(&self, path: &str) -> Result<ReadStream> {
        self.create_read_stream_with_options(path, ReadStreamOptions::default())
    }

    /// Opens a file as a readable stream with byte-range and chunk options.
    pub fn create_read_stream_with_options(
        &self,
        path: &str,
        options: ReadStreamOptions,
    ) -> Result<ReadStream> {
        fs::create_read_stream_with_options(&self.resolve(path)?, options)
    }

    /// Opens a file as a writable stream, replacing its contents.
    pub fn create_write_stream(&self, path: &str) -> Result<WriteStream> {
        self.create_write_stream_with_options(path, WriteStreamOptions::default())
    }

    /// Opens a file as a writable stream with the provided options.
    pub fn create_write_stream_with_options(
        &self,
        path: &str,
        options: WriteStreamOptions,
    ) -> Result<WriteStream> {
        fs::create_write_stream_with_options(&self.resolve(path)?, options)
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

    define_bool_option_setters! {
        /// Sets the option for read access.
        ///
        /// This option, when true, will indicate that the file should be
        /// readable if opened.
        read;
        /// Sets the option for write access.
        ///
        /// If the file already exists, any write calls on it will overwrite its
        /// contents, without truncating it.
        write;
        /// Sets the option for the append mode.
        ///
        /// This option, when true, means that writes will append to a file instead
        /// of overwriting previous contents.
        ///
        /// Note
        /// This function doesn’t create the file if it doesn’t exist. Use the
        /// `OpenOptions::create` method to do so.
        append;
        /// Sets the option for truncating a previous file.
        ///
        /// If a file is successfully opened with this option set to true, it will truncate
        /// the file to 0 length if it already exists.
        ///
        /// The file must be opened with write access for truncate to work.
        truncate;
        /// Sets the option to create a new file, or open it if it already exists.
        ///
        /// In order for the file to be created, `OpenOptions::write` or
        /// `OpenOptions::append` access must be used.
        ///
        /// If `.create(true)` is set without `.write(true)` or `.append(true)`, calling open will
        /// fail with an `InvalidInput` error.
        create;
        /// Sets the option to create a new file, failing if it already exists.
        ///
        /// No file is allowed to exist at the target location, also no (dangling) symlink. In this
        /// way, if the call succeeds, the file returned is guaranteed to be new.
        ///
        /// If `.create_new(true)` is set, `.create()` and `.truncate()` are ignored.
        ///
        /// The file must be opened with write or append access in order to create a new file.
        create_new;
    }
}

define_resource_handle! {
    /// An object providing access to an open file on the filesystem.
    ///
    /// Files are automatically closed when they go out of scope. Errors detected
    /// on closing are ignored by the implementation of `Drop`.
    ///
    /// Use [`fs::close`] to close explicitly and observe close errors.
    File(implementation::File) {
        from_inner,
        drop implementation::close_sync;
    }
}

impl File {
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

    define_bool_accessors! {
        /// Returns `true` if this metadata is for a regular file.
        is_file;
        /// Returns `true` if this metadata is for a directory.
        is_dir;
        /// Returns `true` if this metadata is for a symbolic link.
        is_symlink;
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
                Blob, Dir, File, FileOps, Metadata, OpenOptions, Result, RootFileSystem, StatFs, TempDir, Watcher, WatchOptions, WatchFileOptions, ReadStream, ReadStreamOptions, WriteStream, WriteStreamOptions,
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

            /// Watches a file or directory for changes.
            ///
            /// The listener receives a ZenFS-style event type (`change` or `rename`)
            /// and the name of the changed entry. Keep the returned watcher alive
            /// for as long as notifications are needed.
            pub fn watch(path: &str, listener: impl FnMut(&str, &str) + Send + 'static) -> Result<Watcher> {
                watch_with_options(path, WatchOptions::default(), listener)
            }

            /// Watches a file or directory with the provided options.
            pub fn watch_with_options(path: &str, options: WatchOptions, listener: impl FnMut(&str, &str) + Send + 'static) -> Result<Watcher> {
                let inner = implementation::watch(path, options, Box::new(listener)).map_err(super::Error::from)?;
                Ok(Watcher::from_inner(inner))
            }

            /// Watches changes to a file using polling and returns an ID for its listener.
            ///
            /// Unlike `watch`, this listener receives the current and previous metadata.
            /// Use [`unwatch_file`] or [`unwatch_file_listener`] to stop it.
            pub fn watch_file(path: &str, listener: impl FnMut(Metadata, Metadata) + Send + 'static) -> Result<u64> {
                watch_file_with_options(path, WatchFileOptions::default(), listener)
            }

            /// Watches changes to a file with the provided polling options.
            pub fn watch_file_with_options(path: &str, options: WatchFileOptions, listener: impl FnMut(Metadata, Metadata) + Send + 'static) -> Result<u64> {
                implementation::watch_file(path, options, Box::new(listener)).map_err(Into::into)
            }

            /// Stops all `watch_file` listeners for a path.
            pub fn unwatch_file(path: &str) -> Result<()> {
                implementation::unwatch_file(path, None).map_err(Into::into)
            }

            /// Stops one `watch_file` listener by the ID returned from [`watch_file`].
            pub fn unwatch_file_listener(path: &str, id: u64) -> Result<()> {
                implementation::unwatch_file(path, Some(id)).map_err(Into::into)
            }

            /// Opens a file as a readable stream.
            pub fn create_read_stream(path: &str) -> Result<ReadStream> {
                create_read_stream_with_options(path, ReadStreamOptions::default())
            }

            /// Opens a file as a readable stream with byte-range and chunk options.
            pub fn create_read_stream_with_options(path: &str, options: ReadStreamOptions) -> Result<ReadStream> {
                implementation::create_read_stream(path, options).map(ReadStream::from_inner).map_err(Into::into)
            }

            /// Opens a file as a writable stream, replacing its contents.
            pub fn create_write_stream(path: &str) -> Result<WriteStream> {
                create_write_stream_with_options(path, WriteStreamOptions::default())
            }

            /// Opens a file as a writable stream with the provided options.
            pub fn create_write_stream_with_options(path: &str, options: WriteStreamOptions) -> Result<WriteStream> {
                implementation::create_write_stream(path, options).map(WriteStream::from_inner).map_err(Into::into)
            }

            define_wasm_async_wrappers! {
                /// Configures ZenFS with the supplied configuration, including asynchronous backends.
                configure(configuration: &JsValue) -> () = configure;
                /// Configures ZenFS with one backend mounted at `/`, including asynchronous backends.
                configure_single(configuration: &JsValue) -> () = configure_single;
                /// Resolves a backend configuration asynchronously into a filesystem instance.
                resolve_mount_config(configuration: &JsValue) -> JsValue = resolve_mount_config;
                /// Resolves a remote filesystem asynchronously using a channel and mount configuration.
                resolve_remote_mount(channel: &JsValue, configuration: &JsValue) -> JsValue = resolve_remote_mount;
                /// Flushes all mounted filesystems.
                sync() -> () = sync;
                /// Waits for a worker to come online.
                wait_online(worker: &JsValue) -> () = wait_online;
            }

            define_wasm_sync_wrappers! {
                /// Configures ZenFS synchronously. This fails if a configured backend requires async setup.
                configure_sync(configuration: &JsValue) -> () = configure_sync;
                /// Configures ZenFS synchronously with one backend mounted at `/`.
                configure_single_sync(configuration: &JsValue) -> () = configure_single_sync;
                /// Applies shared configuration options to an already-created filesystem.
                configure_file_system(filesystem: &JsValue, configuration: &JsValue) -> () = configure_file_system;
                /// Resolves a backend configuration synchronously into a filesystem instance.
                resolve_mount_config_sync(configuration: &JsValue) -> JsValue = resolve_mount_config_sync;
                /// Mounts an existing filesystem at `mount_point`.
                mount(mount_point: &str, filesystem: &JsValue) -> () = mount;
                /// Unmounts the filesystem at `mount_point`.
                umount(mount_point: &str) -> () = umount;
                /// Attaches a filesystem to a channel or RPC port.
                attach_fs(channel: &JsValue, filesystem: &JsValue) -> () = attach_fs;
                /// Detaches a filesystem from a channel or RPC port.
                detach_fs(channel: &JsValue, filesystem: &JsValue) -> () = detach_fs;
            }

            define_wasm_value_wrappers! {
                /// Returns ZenFS's map of mount points to filesystem instances.
                mounts() -> JsValue = mounts;
                /// Returns ZenFS's `fs` object.
                core_fs() -> JsValue = zenfs_fs;
                /// Returns ZenFS's promise-based filesystem API.
                promises() -> JsValue = promises;
                /// Returns ZenFS's default filesystem object.
                default_fs() -> JsValue = default_fs;
                /// Returns ZenFS's virtual filesystem module.
                vfs() -> JsValue = vfs;
                /// Returns the ZenFS version value exported by `@zenfs/core`.
                version() -> JsValue = version;
                /// Returns ZenFS's filesystem constants object.
                constants() -> JsValue = constants;
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

            /// Creates a new symbolic link on the filesystem.
            pub async fn symlink(target: &str, link: &str) -> Result<()> {
                implementation_result!(implementation::symlink(target, link).await)
            }

            /// Creates a new symbolic link on the filesystem.
            pub fn symlink_sync(target: &str, link: &str) -> Result<()> {
                implementation_result!(implementation::symlink_sync(target, link))
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
    /// Renames a file or directory to a new name, replacing the original file if `to` already exists.
    rename([from, to]) -> () => rename_sync;
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
    ///
    /// Like ZenFS, matches for absolute patterns omit the leading slash.
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
    /// Creates a new hard link on the filesystem.
    link([original, link]) -> () => link_sync;
    /// Reads a symbolic link, returning the file that the link points to.
    readlink([path]) -> String => readlink_sync;
    /// Removes a file from the filesystem.
    unlink([path]) -> () => unlink_sync;
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
    fn streams_read_in_chunks_and_finish_or_destroy_writes() {
        futures_lite::future::block_on(async {
            let temp = tempdir().unwrap();
            let dir = Directory::new(temp.path()).unwrap();
            let mut output = dir.create_write_stream("data").unwrap();
            output.write(b"abcdef").await.unwrap();
            output.write(b"gh").await.unwrap();
            output.end().await.unwrap();
            assert!(output.write(b"no").await.is_err());
            assert_eq!(dir.read_text("data").unwrap(), "abcdefgh");

            let mut input = dir
                .create_read_stream_with_options(
                    "data",
                    ReadStreamOptions {
                        start: 2,
                        end: Some(6),
                        chunk_size: 2,
                    },
                )
                .unwrap();
            assert_eq!(input.read().await.unwrap(), Some(b"cd".to_vec()));
            assert_eq!(input.read().await.unwrap(), Some(b"ef".to_vec()));
            assert_eq!(input.read().await.unwrap(), Some(b"g".to_vec()));
            assert_eq!(input.read().await.unwrap(), None);
            input.destroy().unwrap();
            assert!(input.read().await.is_err());

            let mut replace = dir.create_write_stream("data").unwrap();
            replace.write(b"!").await.unwrap();
            replace.end().await.unwrap();
            assert_eq!(dir.read_text("data").unwrap(), "!");
            let mut positioned = dir
                .create_write_stream_with_options(
                    "positioned",
                    WriteStreamOptions { start: Some(2) },
                )
                .unwrap();
            positioned.write(b"x").await.unwrap();
            positioned.end().await.unwrap();
            assert_eq!(dir.read_file_sync("positioned").unwrap(), b"\0\0x");
            let mut abandoned = dir.create_write_stream("abandoned").unwrap();
            abandoned.destroy().unwrap();
            assert!(abandoned.write(b"no").await.is_err());
            assert!(
                dir.create_read_stream_with_options(
                    "data",
                    ReadStreamOptions {
                        chunk_size: 0,
                        ..Default::default()
                    }
                )
                .is_err()
            );
            assert!(
                dir.create_read_stream_with_options(
                    "data",
                    ReadStreamOptions {
                        start: 3,
                        end: Some(2),
                        ..Default::default()
                    }
                )
                .is_err()
            );
            assert!(dir.create_read_stream("../escape").is_err());

            let path = temp.path().join("data");
            let mut root_stream = fs::create_read_stream(path.to_str().unwrap()).unwrap();
            assert_eq!(root_stream.read().await.unwrap(), Some(b"!".to_vec()));
        });
    }

    #[test]
    fn watch_reports_events_and_closes() {
        let temp = tempdir().unwrap();
        let dir = Directory::new(temp.path()).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let mut watcher = dir
            .watch(".", move |event, name| {
                let _ = tx.send((event.to_owned(), name.to_owned()));
            })
            .unwrap();
        dir.write_text("created", "hello").unwrap();
        let (event, name) = rx.recv_timeout(std::time::Duration::from_secs(3)).unwrap();
        assert!(matches!(event.as_str(), "change" | "rename"));
        assert_eq!(name, "created");
        watcher.close().unwrap();
        assert!(watcher.close().is_err());
        assert!(dir.watch("../escape", |_, _| {}).is_err());
    }

    #[test]
    fn watch_file_listeners_can_be_removed_individually_or_together() {
        let temp = tempdir().unwrap();
        let dir = Directory::new(temp.path()).unwrap();
        dir.write_text("file", "a").unwrap();
        let options = WatchFileOptions {
            interval: std::time::Duration::from_millis(20),
            ..Default::default()
        };
        let (first_tx, first_rx) = std::sync::mpsc::channel();
        let first = dir
            .watch_file_with_options("file", options, move |curr, prev| {
                let _ = first_tx.send((curr.len(), prev.len()));
            })
            .unwrap();
        let (second_tx, second_rx) = std::sync::mpsc::channel();
        let second = dir
            .watch_file_with_options("file", options, move |curr, prev| {
                let _ = second_tx.send((curr.len(), prev.len()));
            })
            .unwrap();
        dir.write_text("file", "longer").unwrap();
        assert_eq!(
            first_rx
                .recv_timeout(std::time::Duration::from_secs(3))
                .unwrap(),
            (6, 1)
        );
        assert_eq!(
            second_rx
                .recv_timeout(std::time::Duration::from_secs(3))
                .unwrap(),
            (6, 1)
        );
        dir.unwatch_file_listener("file", first).unwrap();
        dir.write_text("file", "even longer").unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            let (current_len, _) = second_rx.recv_timeout(remaining).unwrap();
            if current_len == 11 {
                break;
            }
        }
        assert!(
            first_rx
                .recv_timeout(std::time::Duration::from_millis(100))
                .is_err()
        );
        dir.unwatch_file("file").unwrap();
        dir.unwatch_file_listener("file", second).unwrap();
        assert!(dir.unwatch_file("../escape").is_err());
    }

    #[test]
    fn hard_links_rename_and_unlink_work_on_fs_and_directory() {
        let temp = tempdir().unwrap();
        let dir = Directory::new(temp.path()).unwrap();
        dir.write_text("source", "original").unwrap();
        dir.link_sync("source", "hard").unwrap();
        dir.write_text("hard", "updated").unwrap();
        assert_eq!(dir.read_text("source").unwrap(), "updated");
        dir.rename_sync("hard", "moved").unwrap();
        dir.unlink_sync("source").unwrap();
        assert_eq!(dir.read_text("moved").unwrap(), "updated");
        dir.unlink_sync("moved").unwrap();
        assert_eq!(
            dir.unlink_sync("moved").unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );
        assert_eq!(
            dir.link_sync("../outside", "bad").unwrap_err().kind(),
            std::io::ErrorKind::InvalidInput
        );

        let original = temp.path().join("original");
        let copy = temp.path().join("copy");
        fs::write_text(original.to_str().unwrap(), "data").unwrap();
        fs::link_sync(original.to_str().unwrap(), copy.to_str().unwrap()).unwrap();
        fs::unlink_sync(original.to_str().unwrap()).unwrap();
        assert_eq!(fs::read_text(copy.to_str().unwrap()).unwrap(), "data");
    }

    #[cfg(unix)]
    #[test]
    fn symlinks_keep_relative_targets_and_unlink_does_not_remove_them() {
        let temp = tempdir().unwrap();
        let dir = Directory::new(temp.path()).unwrap();
        dir.create_dir("nested").unwrap();
        dir.write_text("source", "contents").unwrap();
        dir.symlink_sync("../source", "nested/link").unwrap();
        assert_eq!(dir.readlink_sync("nested/link").unwrap(), "../source");
        assert_eq!(dir.read_text("nested/link").unwrap(), "contents");
        dir.unlink_sync("nested/link").unwrap();
        assert_eq!(dir.read_text("source").unwrap(), "contents");
        dir.symlink_sync("../missing", "nested/dangling").unwrap();
        assert_eq!(dir.readlink_sync("nested/dangling").unwrap(), "../missing");
        assert_eq!(
            dir.symlink_sync("../../outside", "nested/escape")
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::InvalidInput
        );
        assert_eq!(
            dir.symlink_sync("source", "../outside").unwrap_err().kind(),
            std::io::ErrorKind::InvalidInput
        );

        let path = temp.path().join("raw-link");
        fs::symlink_sync("source", path.to_str().unwrap()).unwrap();
        assert_eq!(fs::readlink_sync(path.to_str().unwrap()).unwrap(), "source");
    }

    #[test]
    fn async_link_operations_work() {
        futures_lite::future::block_on(async {
            let temp = tempdir().unwrap();
            let dir = Directory::new(temp.path()).unwrap();
            dir.write_text("source", "value").unwrap();
            dir.link("source", "hard").await.unwrap();
            dir.rename("hard", "renamed").await.unwrap();
            dir.unlink("source").await.unwrap();
            assert_eq!(dir.read_text("renamed").unwrap(), "value");
            #[cfg(unix)]
            {
                dir.symlink("renamed", "symbolic").await.unwrap();
                assert_eq!(dir.readlink("symbolic").await.unwrap(), "renamed");
                dir.unlink("symbolic").await.unwrap();
            }
            dir.unlink("renamed").await.unwrap();
        });
    }

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
        let mut opened = dir.opendir_sync("nested").unwrap();
        let entry = opened.read_sync().unwrap().unwrap();
        assert_eq!(entry.name(), "deep");
        assert!(entry.is_dir());
        assert_eq!(opened.read_sync().unwrap(), None);
        opened.close_sync().unwrap();
        assert!(opened.read_sync().is_err());
        assert_eq!(
            dir.realpath_sync("nested/deep").unwrap(),
            temp.path().join("nested/deep").to_str().unwrap()
        );
        assert_eq!(
            dir.glob_sync("nested/*").unwrap(),
            vec![
                temp.path()
                    .join("nested/deep")
                    .to_str()
                    .unwrap()
                    .trim_start_matches('/')
            ]
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
            let mut opened = dir.opendir(".").await.unwrap();
            let entry = opened.read().await.unwrap().unwrap();
            assert_eq!(entry.name(), "nested");
            assert!(entry.is_dir());
            assert_eq!(opened.read().await.unwrap(), None);
            opened.close().await.unwrap();
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
        dir.rename_sync("before.txt", "after.txt").unwrap();
        assert!(!dir.exists_sync("before.txt").unwrap());
        assert_eq!(dir.read_text("after.txt").unwrap(), "from directory");

        let root_before = temp.path().join("after.txt");
        let root_after = temp.path().join("root-renamed.txt");
        fs::rename_sync(root_before.to_str().unwrap(), root_after.to_str().unwrap()).unwrap();
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
            dir.rename_sync("inside.txt", "../outside.txt")
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

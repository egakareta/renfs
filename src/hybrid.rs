//! Runtime dispatcher for builds that include both ZenFS and WebFS.

use std::{
    cell::Cell,
    io::{IoSlice, IoSliceMut},
    time::SystemTime,
};

use wasm_bindgen::JsValue;

use crate::api::{
    Dirent, Metadata, OpenOptions, ReadStreamOptions, StatFs, TempDir, WatchFileOptions,
    WatchOptions, WriteStreamOptions,
};

use super::{webfs_backend as webfs, zenfs};

type Result<T> = std::result::Result<T, JsValue>;

#[derive(Clone, Copy)]
enum Backend {
    ZenFs,
    WebFs,
}

thread_local! {
    static ACTIVE_BACKEND: Cell<Option<Backend>> = const { Cell::new(None) };
}

fn use_zenfs() -> bool {
    ACTIVE_BACKEND.with(|active| {
        let backend = active.get().unwrap_or_else(|| {
            let backend = if zenfs::is_configured() {
                Backend::ZenFs
            } else {
                Backend::WebFs
            };
            active.set(Some(backend));
            backend
        });
        matches!(backend, Backend::ZenFs)
    })
}

fn io_error(error: std::io::Error) -> JsValue {
    JsValue::from_str(&error.to_string())
}

pub fn invalid_path_error(message: &str) -> JsValue {
    io_error(webfs::invalid_path_error(message))
}

macro_rules! dispatch_sync {
    ($name:ident($($argument:ident: $type:ty),* $(,)?) -> $output:ty) => {
        pub fn $name($($argument: $type),*) -> Result<$output> {
            if use_zenfs() {
                zenfs::$name($($argument),*)
            } else {
                webfs::$name($($argument),*).map_err(io_error)
            }
        }
    };
}

macro_rules! dispatch_async {
    ($name:ident($($argument:ident: $type:ty),* $(,)?) -> $output:ty) => {
        pub async fn $name($($argument: $type),*) -> Result<$output> {
            if use_zenfs() {
                zenfs::$name($($argument),*).await
            } else {
                webfs::$name($($argument),*).await.map_err(io_error)
            }
        }
    };
}

dispatch_sync!(normalize_path(path: &str) -> String);
dispatch_sync!(glob_to_regex(pattern: &str) -> String);
dispatch_sync!(app_dir(name: &str) -> String);
dispatch_sync!(exists_sync(path: &str) -> bool);
dispatch_sync!(access_sync(path: &str, mode: u32) -> ());
dispatch_sync!(read_text(path: &str) -> String);
dispatch_sync!(write_text(path: &str, contents: &str) -> ());
dispatch_sync!(append_text(path: &str, contents: &str) -> ());
dispatch_sync!(append_file_sync(path: &str, contents: &[u8]) -> ());
dispatch_sync!(read_file_sync(path: &str) -> Vec<u8>);
dispatch_sync!(write_file_sync(path: &str, contents: &[u8]) -> ());
dispatch_sync!(copy_file_sync(from: &str, to: &str) -> ());
dispatch_sync!(cp_sync(from: &str, to: &str) -> ());
dispatch_sync!(remove_file(path: &str) -> ());
dispatch_sync!(create_dir(path: &str) -> ());
dispatch_sync!(create_dir_all(path: &str) -> ());
dispatch_sync!(read_dir(path: &str) -> Vec<String>);
dispatch_sync!(mkdir_sync(path: &str, recursive: bool) -> ());
dispatch_sync!(readdir_sync(path: &str) -> Vec<String>);
dispatch_sync!(rmdir_sync(path: &str) -> ());
dispatch_sync!(rm_sync(path: &str, recursive: bool, force: bool) -> ());
dispatch_sync!(rename_sync(from: &str, to: &str) -> ());
dispatch_sync!(realpath_sync(path: &str) -> String);
dispatch_sync!(stat_sync(path: &str) -> Metadata);
dispatch_sync!(lstat_sync(path: &str) -> Metadata);
dispatch_sync!(statfs_sync(path: &str) -> StatFs);
dispatch_sync!(chmod_sync(path: &str, mode: u32) -> ());
dispatch_sync!(lchmod_sync(path: &str, mode: u32) -> ());
dispatch_sync!(chown_sync(path: &str, uid: u32, gid: u32) -> ());
dispatch_sync!(lchown_sync(path: &str, uid: u32, gid: u32) -> ());
dispatch_sync!(utimes_sync(path: &str, atime: SystemTime, mtime: SystemTime) -> ());
dispatch_sync!(lutimes_sync(path: &str, atime: SystemTime, mtime: SystemTime) -> ());
dispatch_sync!(link_sync(original: &str, link: &str) -> ());
dispatch_sync!(readlink_sync(path: &str) -> String);
dispatch_sync!(unlink_sync(path: &str) -> ());
dispatch_sync!(truncate_sync(path: &str, len: u64) -> ());
dispatch_sync!(symlink_sync(target: &str, link: &str) -> ());
dispatch_sync!(mkdtemp_sync(prefix: &str) -> String);
dispatch_sync!(mkdtemp_disposable_sync(prefix: &str) -> TempDir);
dispatch_sync!(glob_sync(pattern: &str) -> Vec<String>);

dispatch_async!(exists(path: &str) -> bool);
dispatch_async!(access(path: &str, mode: u32) -> ());
dispatch_async!(append_file(path: &str, contents: &[u8]) -> ());
dispatch_async!(read_file(path: &str) -> Vec<u8>);
dispatch_async!(write_file(path: &str, contents: &[u8]) -> ());
dispatch_async!(copy_file(from: &str, to: &str) -> ());
dispatch_async!(cp(from: &str, to: &str) -> ());
dispatch_async!(rename(from: &str, to: &str) -> ());
dispatch_async!(mkdir(path: &str, recursive: bool) -> ());
dispatch_async!(mkdtemp(prefix: &str) -> String);
dispatch_async!(readdir(path: &str) -> Vec<String>);
dispatch_async!(rmdir(path: &str) -> ());
dispatch_async!(rm(path: &str, recursive: bool, force: bool) -> ());
dispatch_async!(realpath(path: &str) -> String);
dispatch_async!(glob(pattern: &str) -> Vec<String>);
dispatch_async!(stat(path: &str) -> Metadata);
dispatch_async!(lstat(path: &str) -> Metadata);
dispatch_async!(statfs(path: &str) -> StatFs);
dispatch_async!(chmod(path: &str, mode: u32) -> ());
dispatch_async!(lchmod(path: &str, mode: u32) -> ());
dispatch_async!(chown(path: &str, uid: u32, gid: u32) -> ());
dispatch_async!(lchown(path: &str, uid: u32, gid: u32) -> ());
dispatch_async!(utimes(path: &str, atime: SystemTime, mtime: SystemTime) -> ());
dispatch_async!(lutimes(path: &str, atime: SystemTime, mtime: SystemTime) -> ());
dispatch_async!(link(original: &str, link: &str) -> ());
dispatch_async!(readlink(path: &str) -> String);
dispatch_async!(unlink(path: &str) -> ());
dispatch_async!(truncate(path: &str, len: u64) -> ());
dispatch_async!(symlink(target: &str, link: &str) -> ());

pub enum File {
    ZenFs(zenfs::File),
    WebFs(webfs::File),
}

pub enum Dir {
    ZenFs(zenfs::Dir),
    WebFs(webfs::Dir),
}

pub enum ReadStream {
    ZenFs(zenfs::ReadStream),
    WebFs(webfs::ReadStream),
}

pub enum WriteStream {
    ZenFs(zenfs::WriteStream),
    WebFs(webfs::WriteStream),
}

pub enum Watcher {
    ZenFs(zenfs::Watcher),
    WebFs(webfs::Watcher),
}

pub fn open_sync(path: &str, options: &OpenOptions) -> Result<File> {
    if use_zenfs() {
        zenfs::open_sync(path, options).map(File::ZenFs)
    } else {
        webfs::open_sync(path, options)
            .map(File::WebFs)
            .map_err(io_error)
    }
}

pub async fn open(path: &str, options: &OpenOptions) -> Result<File> {
    if use_zenfs() {
        zenfs::open(path, options).await.map(File::ZenFs)
    } else {
        webfs::open(path, options)
            .await
            .map(File::WebFs)
            .map_err(io_error)
    }
}

pub async fn open_as_blob(path: &str) -> Result<Vec<u8>> {
    if use_zenfs() {
        zenfs::open_as_blob(path).await
    } else {
        webfs::open_as_blob(path).await.map_err(io_error)
    }
}

pub fn close_sync(file: File) -> Result<()> {
    match file {
        File::ZenFs(file) => zenfs::close_sync(file),
        File::WebFs(file) => webfs::close_sync(file).map_err(io_error),
    }
}

pub async fn close(file: &mut File) -> Result<()> {
    match file {
        File::ZenFs(file) => zenfs::close(file).await,
        File::WebFs(file) => webfs::close(file).await.map_err(io_error),
    }
}

macro_rules! file_sync {
    ($name:ident($file:ident $(, $argument:ident : $type:ty)*) -> $output:ty) => {
        pub fn $name($file: &mut File $(, $argument: $type)*) -> Result<$output> {
            match $file {
                File::ZenFs(inner) => zenfs::$name(inner $(, $argument)*),
                File::WebFs(inner) => webfs::$name(inner $(, $argument)*).map_err(io_error),
            }
        }
    };
}

macro_rules! file_async {
    ($name:ident($file:ident $(, $argument:ident : $type:ty)*) -> $output:ty) => {
        pub async fn $name($file: &mut File $(, $argument: $type)*) -> Result<$output> {
            match $file {
                File::ZenFs(inner) => zenfs::$name(inner $(, $argument)*).await,
                File::WebFs(inner) => webfs::$name(inner $(, $argument)*).await.map_err(io_error),
            }
        }
    };
}

file_sync!(read_sync(file, buffer: &mut [u8]) -> usize);
file_async!(read(file, buffer: &mut [u8]) -> usize);
file_sync!(write_sync(file, buffer: &[u8]) -> usize);
file_async!(write(file, buffer: &[u8]) -> usize);
file_sync!(readv_sync(file, buffers: &mut [IoSliceMut<'_>]) -> usize);
file_async!(readv(file, buffers: &mut [IoSliceMut<'_>]) -> usize);
file_sync!(writev_sync(file, buffers: &[IoSlice<'_>]) -> usize);
file_async!(writev(file, buffers: &[IoSlice<'_>]) -> usize);

macro_rules! file_ref_sync {
    ($name:ident($file:ident $(, $argument:ident : $type:ty)*) -> $output:ty) => {
        pub fn $name($file: &File $(, $argument: $type)*) -> Result<$output> {
            match $file {
                File::ZenFs(inner) => zenfs::$name(inner $(, $argument)*),
                File::WebFs(inner) => webfs::$name(inner $(, $argument)*).map_err(io_error),
            }
        }
    };
}

macro_rules! file_ref_async {
    ($name:ident($file:ident $(, $argument:ident : $type:ty)*) -> $output:ty) => {
        pub async fn $name($file: &File $(, $argument: $type)*) -> Result<$output> {
            match $file {
                File::ZenFs(inner) => zenfs::$name(inner $(, $argument)*).await,
                File::WebFs(inner) => webfs::$name(inner $(, $argument)*).await.map_err(io_error),
            }
        }
    };
}

file_ref_sync!(fstat_sync(file) -> Metadata);
file_ref_async!(fstat(file) -> Metadata);
file_ref_sync!(fchmod_sync(file, mode: u32) -> ());
file_ref_async!(fchmod(file, mode: u32) -> ());
file_ref_sync!(fchown_sync(file, uid: u32, gid: u32) -> ());
file_ref_async!(fchown(file, uid: u32, gid: u32) -> ());
file_ref_sync!(futimes_sync(file, atime: SystemTime, mtime: SystemTime) -> ());
file_ref_async!(futimes(file, atime: SystemTime, mtime: SystemTime) -> ());
file_ref_sync!(fsync_sync(file) -> ());
file_ref_async!(fsync(file) -> ());
file_ref_sync!(fdatasync_sync(file) -> ());
file_ref_async!(fdatasync(file) -> ());
file_ref_sync!(ftruncate_sync(file, size: u64) -> ());
file_ref_async!(ftruncate(file, size: u64) -> ());

pub fn create_read_stream(path: &str, options: ReadStreamOptions) -> Result<ReadStream> {
    if use_zenfs() {
        zenfs::create_read_stream(path, options).map(ReadStream::ZenFs)
    } else {
        webfs::create_read_stream(path, options)
            .map(ReadStream::WebFs)
            .map_err(io_error)
    }
}

pub async fn stream_read(stream: &mut ReadStream) -> Result<Option<Vec<u8>>> {
    match stream {
        ReadStream::ZenFs(inner) => zenfs::stream_read(inner).await,
        ReadStream::WebFs(inner) => webfs::stream_read(inner).await.map_err(io_error),
    }
}

pub fn read_stream_destroy(stream: &ReadStream) -> Result<()> {
    match stream {
        ReadStream::ZenFs(inner) => zenfs::read_stream_destroy(inner),
        ReadStream::WebFs(inner) => webfs::read_stream_destroy(inner).map_err(io_error),
    }
}

pub fn create_write_stream(path: &str, options: WriteStreamOptions) -> Result<WriteStream> {
    if use_zenfs() {
        zenfs::create_write_stream(path, options).map(WriteStream::ZenFs)
    } else {
        webfs::create_write_stream(path, options)
            .map(WriteStream::WebFs)
            .map_err(io_error)
    }
}

pub async fn stream_write(stream: &mut WriteStream, bytes: &[u8]) -> Result<()> {
    match stream {
        WriteStream::ZenFs(inner) => zenfs::stream_write(inner, bytes).await,
        WriteStream::WebFs(inner) => webfs::stream_write(inner, bytes).await.map_err(io_error),
    }
}

pub async fn stream_end(stream: &mut WriteStream) -> Result<()> {
    match stream {
        WriteStream::ZenFs(inner) => zenfs::stream_end(inner).await,
        WriteStream::WebFs(inner) => webfs::stream_end(inner).await.map_err(io_error),
    }
}

pub fn write_stream_destroy(stream: &WriteStream) -> Result<()> {
    match stream {
        WriteStream::ZenFs(inner) => zenfs::write_stream_destroy(inner),
        WriteStream::WebFs(inner) => webfs::write_stream_destroy(inner).map_err(io_error),
    }
}

pub fn watch(
    path: &str,
    options: WatchOptions,
    listener: Box<dyn FnMut(&str, &str) + Send>,
) -> Result<Watcher> {
    if use_zenfs() {
        zenfs::watch(path, options, listener).map(Watcher::ZenFs)
    } else {
        webfs::watch(path, options, listener)
            .map(Watcher::WebFs)
            .map_err(io_error)
    }
}

pub fn watch_close(watcher: &Watcher) -> Result<()> {
    match watcher {
        Watcher::ZenFs(inner) => zenfs::watch_close(inner),
        Watcher::WebFs(inner) => webfs::watch_close(inner).map_err(io_error),
    }
}

pub fn watch_file(
    path: &str,
    options: WatchFileOptions,
    listener: Box<dyn FnMut(Metadata, Metadata) + Send>,
) -> Result<u64> {
    if use_zenfs() {
        zenfs::watch_file(path, options, listener)
    } else {
        webfs::watch_file(path, options, listener).map_err(io_error)
    }
}

pub fn unwatch_file(path: &str, id: Option<u64>) -> Result<()> {
    if use_zenfs() {
        zenfs::unwatch_file(path, id)
    } else {
        webfs::unwatch_file(path, id).map_err(io_error)
    }
}

pub fn opendir_sync(path: &str) -> Result<crate::api::Dir> {
    let dir = if use_zenfs() {
        Dir::ZenFs(zenfs::open_dir_sync(path)?)
    } else {
        Dir::WebFs(webfs::open_dir_sync(path).map_err(io_error)?)
    };
    Ok(crate::api::Dir::from_inner(dir))
}

pub async fn opendir(path: &str) -> Result<crate::api::Dir> {
    let dir = if use_zenfs() {
        Dir::ZenFs(zenfs::open_dir(path).await?)
    } else {
        Dir::WebFs(webfs::open_dir_sync(path).map_err(io_error)?)
    };
    Ok(crate::api::Dir::from_inner(dir))
}

pub fn dir_read_sync(dir: &mut Dir) -> Result<Option<Dirent>> {
    match dir {
        Dir::ZenFs(inner) => zenfs::dir_read_sync(inner),
        Dir::WebFs(inner) => webfs::dir_read_sync(inner).map_err(io_error),
    }
}

pub async fn dir_read(dir: &mut Dir) -> Result<Option<Dirent>> {
    match dir {
        Dir::ZenFs(inner) => zenfs::dir_read(inner).await,
        Dir::WebFs(inner) => webfs::dir_read(inner).await.map_err(io_error),
    }
}

pub fn dir_close_sync(dir: &Dir) -> Result<()> {
    match dir {
        Dir::ZenFs(inner) => zenfs::dir_close_sync(inner),
        Dir::WebFs(inner) => webfs::dir_close_sync(inner).map_err(io_error),
    }
}

pub async fn dir_close(dir: &Dir) -> Result<()> {
    match dir {
        Dir::ZenFs(inner) => zenfs::dir_close(inner).await,
        Dir::WebFs(inner) => webfs::dir_close(inner).await.map_err(io_error),
    }
}

#[cfg(test)]
mod tests {
    use super::{ACTIVE_BACKEND, Backend};
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    async fn webfs_fallback_handles_directories_files_and_streams() {
        ACTIVE_BACKEND.with(|active| active.set(Some(Backend::WebFs)));

        let name = format!("renfs-hybrid-{:x}", js_sys::Math::random().to_bits());
        let dir = crate::app_dir(&name).unwrap();
        dir.write_text("counter.txt", "1").unwrap();
        assert_eq!(dir.read_text("counter.txt").unwrap(), "1");

        let mut stream = dir.create_write_stream("stream.txt").unwrap();
        stream.write(b"fallback").await.unwrap();
        stream.end().await.unwrap();
        assert_eq!(dir.read_file_sync("stream.txt").unwrap(), b"fallback");

        crate::fs::rm_sync(dir.as_path(), true, false).unwrap();
        ACTIVE_BACKEND.with(|active| active.set(None));
    }
}

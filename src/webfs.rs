//! Implementation of the public filesystem API over webfs's localStorage store.

use std::{
    io::{self, IoSlice, IoSliceMut, Read, Seek, SeekFrom, Write},
    path::Path,
    time::SystemTime,
};

use super::{
    access_sync, append_file_sync, copy_file_sync, cp_sync, exists_sync, link_sync, lstat_sync,
    mkdir_sync, mkdtemp_sync, normalize_path, platform_fs as fs, read_file_sync, readdir_sync,
    readlink_sync, realpath_sync, rename_sync, rm_sync, rmdir_sync, stat_sync, symlink_sync,
    truncate_sync, unlink_sync, write_file_sync,
};
use crate::api::{
    Dirent, Metadata, OpenOptions, ReadStreamOptions, StatFs, WatchFileOptions, WatchOptions,
    WriteStreamOptions,
};

pub type File = fs::File;
pub type Dir = fs::ReadDir;

pub struct ReadStream {
    file: File,
    chunk_size: usize,
    remaining: Option<u64>,
}

pub fn create_read_stream(path: &str, options: ReadStreamOptions) -> io::Result<ReadStream> {
    validate_stream_options(options)?;
    let mut file = File::open(path)?;
    if options.start != 0 {
        file.seek(SeekFrom::Start(options.start))?;
    }
    Ok(ReadStream {
        file,
        chunk_size: options.chunk_size,
        remaining: options
            .end
            .map(|end| end.saturating_sub(options.start).saturating_add(1)),
    })
}

fn validate_stream_options(options: ReadStreamOptions) -> io::Result<()> {
    if options.chunk_size == 0 || options.chunk_size > u32::MAX as usize {
        return Err(super::invalid_path_error(
            "stream chunk size must be between 1 and u32::MAX",
        ));
    }
    if options.end.is_some_and(|end| end < options.start) {
        return Err(super::invalid_path_error(
            "stream end must not be before start",
        ));
    }
    Ok(())
}

pub async fn stream_read(stream: &mut ReadStream) -> io::Result<Option<Vec<u8>>> {
    let size = stream.remaining.map_or(stream.chunk_size, |remaining| {
        stream
            .chunk_size
            .min(usize::try_from(remaining).unwrap_or(usize::MAX))
    });
    if size == 0 {
        return Ok(None);
    }
    let mut bytes = vec![0; size];
    let count = stream.file.read(&mut bytes)?;
    if count == 0 {
        return Ok(None);
    }
    bytes.truncate(count);
    if let Some(remaining) = &mut stream.remaining {
        *remaining -= count as u64;
    }
    Ok(Some(bytes))
}

pub fn read_stream_destroy(_stream: &ReadStream) -> io::Result<()> {
    Ok(())
}

pub struct WriteStream {
    file: File,
}

pub fn create_write_stream(path: &str, options: WriteStreamOptions) -> io::Result<WriteStream> {
    let mut open = fs::OpenOptions::new();
    open.write(true).create(true).truncate(true);
    let mut file = open.open(path)?;
    if let Some(start) = options.start {
        file.seek(SeekFrom::Start(start))?;
    }
    Ok(WriteStream { file })
}

pub async fn stream_write(stream: &mut WriteStream, bytes: &[u8]) -> io::Result<()> {
    stream.file.write_all(bytes)
}

pub async fn stream_end(stream: &mut WriteStream) -> io::Result<()> {
    stream.file.sync_all()
}

pub fn write_stream_destroy(_stream: &WriteStream) -> io::Result<()> {
    Ok(())
}

pub struct Watcher;

pub fn watch(
    _path: &str,
    _options: WatchOptions,
    _listener: Box<dyn FnMut(&str, &str) + Send>,
) -> io::Result<Watcher> {
    Err(unsupported("file watching is not supported by webfs"))
}

pub fn watch_close(_watcher: &Watcher) -> io::Result<()> {
    Ok(())
}

pub fn watch_file(
    _path: &str,
    _options: WatchFileOptions,
    _listener: Box<dyn FnMut(Metadata, Metadata) + Send>,
) -> io::Result<u64> {
    Err(unsupported("file watching is not supported by webfs"))
}

pub fn unwatch_file(_path: &str, _id: Option<u64>) -> io::Result<()> {
    Ok(())
}

fn unsupported(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::Unsupported, message)
}

pub fn open_sync(path: &str, options: &OpenOptions) -> io::Result<File> {
    let mut open = fs::OpenOptions::new();
    open.read(options.read)
        .write(options.write)
        .append(options.append)
        .truncate(options.truncate)
        .create(options.create)
        .create_new(options.create_new);
    open.open(path)
}

pub async fn open(path: &str, options: &OpenOptions) -> io::Result<File> {
    open_sync(path, options)
}

pub async fn open_as_blob(path: &str) -> io::Result<Vec<u8>> {
    super::read_file_sync(path)
}

pub fn close_sync(file: File) -> io::Result<()> {
    drop(file);
    Ok(())
}

pub async fn close(_file: &mut File) -> io::Result<()> {
    Ok(())
}

pub fn read_sync(file: &mut File, buffer: &mut [u8]) -> io::Result<usize> {
    file.read(buffer)
}

pub async fn read(file: &mut File, buffer: &mut [u8]) -> io::Result<usize> {
    read_sync(file, buffer)
}

pub fn write_sync(file: &mut File, buffer: &[u8]) -> io::Result<usize> {
    file.write(buffer)
}

pub async fn write(file: &mut File, buffer: &[u8]) -> io::Result<usize> {
    write_sync(file, buffer)
}

pub fn readv_sync(file: &mut File, buffers: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
    let mut total = 0;
    for buffer in buffers {
        if buffer.is_empty() {
            continue;
        }
        let count = file.read(buffer)?;
        total += count;
        if count < buffer.len() {
            break;
        }
    }
    Ok(total)
}

pub async fn readv(file: &mut File, buffers: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
    let mut read = 0;
    for buffer in buffers {
        if buffer.is_empty() {
            continue;
        }
        let count = file.read(buffer)?;
        read += count;
        if count < buffer.len() {
            break;
        }
    }
    Ok(read)
}

pub fn writev_sync(file: &mut File, buffers: &[IoSlice<'_>]) -> io::Result<usize> {
    let mut total = 0;
    for buffer in buffers {
        if buffer.is_empty() {
            continue;
        }
        let count = file.write(buffer)?;
        total += count;
        if count < buffer.len() {
            break;
        }
    }
    Ok(total)
}

pub async fn writev(file: &mut File, buffers: &[IoSlice<'_>]) -> io::Result<usize> {
    let mut written = 0;
    for buffer in buffers {
        if buffer.is_empty() {
            continue;
        }
        let count = file.write(buffer)?;
        written += count;
        if count < buffer.len() {
            break;
        }
    }
    Ok(written)
}

pub fn fstat_sync(file: &File) -> io::Result<Metadata> {
    file.metadata().map(super::metadata_parts)
}

pub async fn fstat(file: &File) -> io::Result<Metadata> {
    fstat_sync(file)
}

pub async fn stat(path: &str) -> io::Result<Metadata> {
    stat_sync(path)
}

pub async fn lstat(path: &str) -> io::Result<Metadata> {
    lstat_sync(path)
}

macro_rules! async_sync_aliases {
    ($(fn $async_name:ident($($argument:ident: $type:ty),*) -> $output:ty = $sync_name:ident;)*) => {
        $(
            pub async fn $async_name($($argument: $type),*) -> io::Result<$output> {
                $sync_name($($argument),*)
            }
        )*
    };
}

async_sync_aliases! {
    fn exists(path: &str) -> bool = exists_sync;
    fn access(path: &str, mode: u32) -> () = access_sync;
    fn append_file(path: &str, contents: &[u8]) -> () = append_file_sync;
    fn read_file(path: &str) -> Vec<u8> = read_file_sync;
    fn write_file(path: &str, contents: &[u8]) -> () = write_file_sync;
    fn copy_file(from: &str, to: &str) -> () = copy_file_sync;
    fn cp(from: &str, to: &str) -> () = cp_sync;
    fn rename(from: &str, to: &str) -> () = rename_sync;
    fn mkdir(path: &str, recursive: bool) -> () = mkdir_sync;
    fn mkdtemp(prefix: &str) -> String = mkdtemp_sync;
    fn readdir(path: &str) -> Vec<String> = readdir_sync;
    fn rmdir(path: &str) -> () = rmdir_sync;
    fn rm(path: &str, recursive: bool, force: bool) -> () = rm_sync;
    fn realpath(path: &str) -> String = realpath_sync;
    fn glob(pattern: &str) -> Vec<String> = glob_sync;
    fn chmod(path: &str, mode: u32) -> () = chmod_sync;
    fn lchmod(path: &str, mode: u32) -> () = lchmod_sync;
    fn chown(path: &str, uid: u32, gid: u32) -> () = chown_sync;
    fn lchown(path: &str, uid: u32, gid: u32) -> () = lchown_sync;
    fn utimes(path: &str, atime: SystemTime, mtime: SystemTime) -> () = utimes_sync;
    fn lutimes(path: &str, atime: SystemTime, mtime: SystemTime) -> () = lutimes_sync;
    fn link(original: &str, link: &str) -> () = link_sync;
    fn readlink(path: &str) -> String = readlink_sync;
    fn unlink(path: &str) -> () = unlink_sync;
    fn truncate(path: &str, len: u64) -> () = truncate_sync;
    fn symlink(target: &str, link: &str) -> () = symlink_sync;
}

pub fn open_dir_sync(path: &str) -> io::Result<Dir> {
    fs::read_dir(path)
}

#[cfg(not(all(feature = "webfs", feature = "zenfs")))]
pub fn opendir_sync(path: &str) -> io::Result<crate::api::Dir> {
    open_dir_sync(path).map(crate::api::Dir::from_inner)
}

#[cfg(not(all(feature = "webfs", feature = "zenfs")))]
pub async fn opendir(path: &str) -> io::Result<crate::api::Dir> {
    opendir_sync(path)
}

fn dirent(entry: fs::DirEntry) -> io::Result<Dirent> {
    let name = entry.file_name().into_string().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "directory entry name is not valid UTF-8",
        )
    })?;
    let kind = entry.file_type()?;
    Ok(Dirent::from_parts(
        name,
        kind.is_file(),
        kind.is_dir(),
        kind.is_symlink(),
    ))
}

pub fn dir_read_sync(dir: &mut Dir) -> io::Result<Option<Dirent>> {
    dir.next().transpose()?.map(dirent).transpose()
}

pub async fn dir_read(dir: &mut Dir) -> io::Result<Option<Dirent>> {
    dir_read_sync(dir)
}

pub fn dir_close_sync(_dir: &Dir) -> io::Result<()> {
    Ok(())
}

pub async fn dir_close(_dir: &Dir) -> io::Result<()> {
    Ok(())
}

pub fn statfs_sync(_path: &str) -> io::Result<StatFs> {
    Err(unsupported("statfs is not supported by webfs"))
}

pub async fn statfs(path: &str) -> io::Result<StatFs> {
    statfs_sync(path)
}

fn permissions(mode: u32) -> fs::Permissions {
    let mut permissions = fs::Permissions::default();
    permissions.set_readonly(mode & 0o222 == 0);
    permissions
}

pub fn chmod_sync(path: &str, mode: u32) -> io::Result<()> {
    fs::set_permissions(path, permissions(mode))
}

pub fn fchmod_sync(file: &File, mode: u32) -> io::Result<()> {
    file.set_permissions(permissions(mode))
}

pub async fn fchmod(file: &File, mode: u32) -> io::Result<()> {
    fchmod_sync(file, mode)
}

pub fn lchmod_sync(_path: &str, _mode: u32) -> io::Result<()> {
    Err(unsupported("lchmod is not supported by webfs"))
}

pub fn chown_sync(_path: &str, _uid: u32, _gid: u32) -> io::Result<()> {
    Err(unsupported("ownership is not supported by webfs"))
}

pub fn lchown_sync(_path: &str, _uid: u32, _gid: u32) -> io::Result<()> {
    Err(unsupported("ownership is not supported by webfs"))
}

pub fn fchown_sync(_file: &File, _uid: u32, _gid: u32) -> io::Result<()> {
    Err(unsupported("ownership is not supported by webfs"))
}

pub async fn fchown(file: &File, uid: u32, gid: u32) -> io::Result<()> {
    fchown_sync(file, uid, gid)
}

pub fn utimes_sync(path: &str, _atime: SystemTime, _mtime: SystemTime) -> io::Result<()> {
    fs::metadata(path).map(|_| ())
}

pub fn lutimes_sync(path: &str, atime: SystemTime, mtime: SystemTime) -> io::Result<()> {
    utimes_sync(path, atime, mtime)
}

pub fn futimes_sync(file: &File, atime: SystemTime, mtime: SystemTime) -> io::Result<()> {
    file.set_times(fs::FileTimes::new().set_accessed(atime).set_modified(mtime))
}

pub async fn futimes(file: &File, atime: SystemTime, mtime: SystemTime) -> io::Result<()> {
    futimes_sync(file, atime, mtime)
}

pub fn fsync_sync(file: &File) -> io::Result<()> {
    file.sync_all()
}

pub async fn fsync(file: &File) -> io::Result<()> {
    fsync_sync(file)
}

pub fn fdatasync_sync(file: &File) -> io::Result<()> {
    file.sync_data()
}

pub async fn fdatasync(file: &File) -> io::Result<()> {
    fdatasync_sync(file)
}

pub fn ftruncate_sync(file: &File, size: u64) -> io::Result<()> {
    file.set_len(size)
}

pub async fn ftruncate(file: &File, size: u64) -> io::Result<()> {
    ftruncate_sync(file, size)
}

pub fn glob_sync(pattern: &str) -> io::Result<Vec<String>> {
    let pattern = normalize_path(pattern)?;
    let absolute = Path::new(&pattern).is_absolute();
    let normalized = pattern.trim_start_matches('/');
    let mut pending = vec![String::new()];
    let mut paths = Vec::new();
    while let Some(parent) = pending.pop() {
        let parent_path = if parent.is_empty() {
            "/".to_owned()
        } else {
            format!("/{parent}")
        };
        for entry in fs::read_dir(&parent_path)? {
            let entry = entry?;
            let name = entry.file_name().into_string().map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "directory entry name is not valid UTF-8",
                )
            })?;
            let candidate = if parent.is_empty() {
                name
            } else {
                format!("{parent}/{name}")
            };
            if glob_matches(normalized, &candidate) {
                paths.push(if absolute {
                    candidate.clone()
                } else {
                    format!("/{candidate}")
                });
            }
            if entry.file_type()?.is_dir() {
                pending.push(candidate);
            }
        }
    }
    paths.sort();
    if absolute {
        Ok(paths)
    } else {
        Ok(paths
            .into_iter()
            .map(|path| path.trim_start_matches('/').to_owned())
            .collect())
    }
}

fn glob_matches(pattern: &str, value: &str) -> bool {
    fn go(pattern: &[char], value: &[char]) -> bool {
        if pattern.is_empty() {
            return value.is_empty();
        }
        match pattern[0] {
            '*' if pattern.get(1) == Some(&'*') => {
                let rest = if pattern.get(2) == Some(&'/') {
                    &pattern[3..]
                } else {
                    &pattern[2..]
                };
                if pattern.get(2) == Some(&'/') && go(rest, value) {
                    return true;
                }
                (0..=value.len()).any(|index| go(rest, &value[index..]))
            }
            '*' => {
                let end = value
                    .iter()
                    .position(|character| *character == '/')
                    .unwrap_or(value.len());
                (0..=end).any(|index| go(&pattern[1..], &value[index..]))
            }
            '?' => !value.is_empty() && value[0] != '/' && go(&pattern[1..], &value[1..]),
            character => {
                !value.is_empty() && character == value[0] && go(&pattern[1..], &value[1..])
            }
        }
    }
    go(
        &pattern.chars().collect::<Vec<_>>(),
        &value.chars().collect::<Vec<_>>(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test;

    #[wasm_bindgen_test]
    async fn public_filesystem_uses_local_storage_without_zenfs_setup() {
        let root = format!("/renfs-webfs-tests-{:x}", js_sys::Math::random().to_bits());
        let file = format!("{root}/nested/data.bin");

        crate::mkdir_sync(&format!("{root}/nested"), true).unwrap();
        crate::write_file_sync(&file, b"web storage").unwrap();
        assert_eq!(crate::read_file_sync(&file).unwrap(), b"web storage");
        assert_eq!(crate::stat_sync(&file).unwrap().len(), 11);

        crate::append_file(&file, b" persists").await.unwrap();
        assert_eq!(crate::read_text(&file).unwrap(), "web storage persists");

        let mut options = OpenOptions::new();
        options.read(true);
        let mut handle = crate::open_sync(&file, &options).unwrap();
        let mut contents = [0; 32];
        let count = handle.read_sync(&mut contents).unwrap();
        assert_eq!(&contents[..count], b"web storage persists");
        crate::close_sync(handle).unwrap();

        let stream_path = format!("{root}/nested/stream.txt");
        let mut stream = crate::create_write_stream(&stream_path).unwrap();
        stream.write(b"stream data").await.unwrap();
        stream.end().await.unwrap();
        assert_eq!(crate::read_text(&stream_path).unwrap(), "stream data");

        crate::rm_sync(&root, true, false).unwrap();
        assert!(!crate::exists_sync(&root).unwrap());
    }
}

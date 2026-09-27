use std::{
    fs,
    io::{self, IoSlice, IoSliceMut, Write as StdWrite},
    path::{Path, PathBuf},
    time::SystemTime,
};

use futures_lite::{
    future::block_on,
    io::{AsyncReadExt, AsyncWriteExt},
    stream::StreamExt,
};

use super::api::{Dirent, Metadata, OpenOptions, StatFs, TempDir};

pub type File = async_fs::File;
pub type Dir = async_fs::ReadDir;

pub fn invalid_path_error(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.to_owned())
}

pub fn app_dir(name: &str) -> io::Result<String> {
    let base = dirs::data_local_dir().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "could not determine the application data directory",
        )
    })?;

    app_dir_in(&base, name)
}

fn app_dir_in(base: &Path, name: &str) -> io::Result<String> {
    let name_path = Path::new(name);
    if name.contains('/')
        || name.contains('\\')
        || !matches!(
            name_path.components().next(),
            Some(std::path::Component::Normal(_))
        )
        || name_path.components().count() != 1
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "application name must be a single path component",
        ));
    }

    let path = base.join(name_path);
    fs::create_dir_all(&path)?;
    path.into_os_string().into_string().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "application data directory path is not valid UTF-8",
        )
    })
}

pub fn exists_sync(path: &str) -> io::Result<bool> {
    Path::new(path).try_exists()
}

fn check_access(metadata: &fs::Metadata, mode: u32) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let permissions = metadata.permissions().mode();
        for (requested, available) in [(4, 0o444), (2, 0o222), (1, 0o111)] {
            if mode & requested != 0 && permissions & available == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "requested access is not permitted",
                ));
            }
        }
    }
    #[cfg(not(unix))]
    {
        if mode & 2 != 0 && metadata.permissions().readonly() {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "write access is not permitted",
            ));
        }
    }
    Ok(())
}

pub async fn exists(path: &str) -> io::Result<bool> {
    match async_fs::metadata(path).await {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

pub fn access_sync(path: &str, mode: u32) -> io::Result<()> {
    check_access(&fs::metadata(path)?, mode)
}

pub async fn access(path: &str, mode: u32) -> io::Result<()> {
    check_access(&async_fs::metadata(path).await?, mode)
}

pub fn read_text(path: &str) -> io::Result<String> {
    fs::read_to_string(path)
}

pub fn write_text(path: &str, contents: &str) -> io::Result<()> {
    fs::write(path, contents)
}

pub fn append_text(path: &str, contents: &str) -> io::Result<()> {
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?
        .write_all(contents.as_bytes())
}

pub fn append_file_sync(path: &str, contents: &[u8]) -> io::Result<()> {
    fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?
        .write_all(contents)
}

pub async fn append_file(path: &str, contents: &[u8]) -> io::Result<()> {
    let mut options = async_fs::OpenOptions::new();
    options.create(true).append(true);
    options.open(path).await?.write_all(contents).await
}

pub fn read_file_sync(path: &str) -> io::Result<Vec<u8>> {
    fs::read(path)
}

pub async fn read_file(path: &str) -> io::Result<Vec<u8>> {
    async_fs::read(path).await
}

pub fn write_file_sync(path: &str, contents: &[u8]) -> io::Result<()> {
    fs::write(path, contents)
}

pub async fn write_file(path: &str, contents: &[u8]) -> io::Result<()> {
    async_fs::write(path, contents).await
}

pub fn copy_file_sync(from: &str, to: &str) -> io::Result<()> {
    fs::copy(from, to).map(|_| ())
}

pub async fn copy_file(from: &str, to: &str) -> io::Result<()> {
    async_fs::copy(from, to).await.map(|_| ())
}

fn create_symlink(from: &Path, to: &Path) -> io::Result<()> {
    let target = fs::read_link(from)?;
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, to)
    }
    #[cfg(windows)]
    {
        if fs::metadata(from)?.is_dir() {
            std::os::windows::fs::symlink_dir(target, to)
        } else {
            std::os::windows::fs::symlink_file(target, to)
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = target;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "copying symbolic links is unsupported on this platform",
        ))
    }
}

pub fn cp_sync(from: &str, to: &str) -> io::Result<()> {
    fn copy_path(from: &Path, to: &Path) -> io::Result<()> {
        let metadata = fs::symlink_metadata(from)?;
        if metadata.is_dir() {
            fs::create_dir_all(to)?;
            for entry in fs::read_dir(from)? {
                let entry = entry?;
                copy_path(&entry.path(), &to.join(entry.file_name()))?;
            }
            fs::set_permissions(to, metadata.permissions())?;
            Ok(())
        } else if metadata.is_file() {
            fs::copy(from, to).map(|_| ())
        } else if metadata.file_type().is_symlink() {
            create_symlink(from, to)
        } else {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "copying this filesystem entry type is unsupported",
            ))
        }
    }

    copy_path(Path::new(from), Path::new(to))
}

pub async fn cp(from: &str, to: &str) -> io::Result<()> {
    let mut pending = vec![(PathBuf::from(from), PathBuf::from(to))];
    let mut directory_permissions = Vec::new();

    while let Some((source, destination)) = pending.pop() {
        let metadata = async_fs::symlink_metadata(&source).await?;
        if metadata.is_dir() {
            async_fs::create_dir_all(&destination).await?;
            let mut entries = async_fs::read_dir(&source).await?;
            while let Some(entry) = entries.next().await {
                let entry = entry?;
                pending.push((entry.path(), destination.join(entry.file_name())));
            }
            directory_permissions.push((destination, metadata.permissions()));
        } else if metadata.file_type().is_symlink() {
            create_symlink(&source, &destination)?;
        } else if metadata.is_file() {
            async_fs::copy(&source, &destination).await?;
        } else {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "copying this filesystem entry type is unsupported",
            ));
        }
    }

    for (path, permissions) in directory_permissions.into_iter().rev() {
        async_fs::set_permissions(path, permissions).await?;
    }
    Ok(())
}

pub fn open_sync(path: &str, options: &OpenOptions) -> io::Result<File> {
    let mut open_options = fs::OpenOptions::new();
    open_options
        .read(options.read)
        .write(options.write)
        .append(options.append)
        .truncate(options.truncate)
        .create(options.create)
        .create_new(options.create_new);
    open_options.open(path).map(File::from)
}

pub async fn open(path: &str, options: &OpenOptions) -> io::Result<File> {
    let mut open_options = async_fs::OpenOptions::new();
    open_options
        .read(options.read)
        .write(options.write)
        .append(options.append)
        .truncate(options.truncate)
        .create(options.create)
        .create_new(options.create_new);
    open_options.open(path).await
}

pub async fn open_as_blob(path: &str) -> io::Result<Vec<u8>> {
    async_fs::read(path).await
}

pub fn close_sync(file: File) -> io::Result<()> {
    drop(file);
    Ok(())
}

pub async fn close(file: &mut File) -> io::Result<()> {
    file.close().await
}

pub fn read_sync(file: &mut File, buffer: &mut [u8]) -> io::Result<usize> {
    block_on(AsyncReadExt::read(file, buffer))
}

pub async fn read(file: &mut File, buffer: &mut [u8]) -> io::Result<usize> {
    file.read(buffer).await
}

pub fn write_sync(file: &mut File, buffer: &[u8]) -> io::Result<usize> {
    block_on(AsyncWriteExt::write(file, buffer))
}

pub async fn write(file: &mut File, buffer: &[u8]) -> io::Result<usize> {
    file.write(buffer).await
}

pub fn readv_sync(file: &mut File, buffers: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
    block_on(async {
        let mut read = 0;
        for buffer in buffers {
            if buffer.is_empty() {
                continue;
            }
            let count = file.read(&mut buffer[..]).await?;
            read += count;
            if count < buffer.len() {
                break;
            }
        }
        Ok(read)
    })
}

pub async fn readv(file: &mut File, buffers: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
    let mut read = 0;
    for buffer in buffers {
        if buffer.is_empty() {
            continue;
        }
        let count = file.read(&mut buffer[..]).await?;
        read += count;
        if count < buffer.len() {
            break;
        }
    }
    Ok(read)
}

pub fn writev_sync(file: &mut File, buffers: &[IoSlice<'_>]) -> io::Result<usize> {
    block_on(async {
        let mut written = 0;
        for buffer in buffers {
            if buffer.is_empty() {
                continue;
            }
            let count = file.write(buffer).await?;
            written += count;
            if count < buffer.len() {
                break;
            }
        }
        Ok(written)
    })
}

pub async fn writev(file: &mut File, buffers: &[IoSlice<'_>]) -> io::Result<usize> {
    let mut written = 0;
    for buffer in buffers {
        if buffer.is_empty() {
            continue;
        }
        let count = file.write(buffer).await?;
        written += count;
        if count < buffer.len() {
            break;
        }
    }
    Ok(written)
}

pub fn fstat_sync(file: &File) -> io::Result<Metadata> {
    let metadata = block_on(file.metadata())?;
    Ok(metadata_parts(metadata))
}

pub async fn fstat(file: &File) -> io::Result<Metadata> {
    Ok(metadata_parts(file.metadata().await?))
}

fn metadata_parts(metadata: fs::Metadata) -> Metadata {
    Metadata::from_parts(
        metadata.len(),
        metadata.is_file(),
        metadata.is_dir(),
        metadata.file_type().is_symlink(),
    )
}

pub fn stat_sync(path: &str) -> io::Result<Metadata> {
    fs::metadata(path).map(metadata_parts)
}
pub async fn stat(path: &str) -> io::Result<Metadata> {
    async_fs::metadata(path).await.map(metadata_parts)
}
pub fn lstat_sync(path: &str) -> io::Result<Metadata> {
    fs::symlink_metadata(path).map(metadata_parts)
}
pub async fn lstat(path: &str) -> io::Result<Metadata> {
    async_fs::symlink_metadata(path).await.map(metadata_parts)
}

#[cfg(unix)]
fn c_path(path: &str) -> io::Result<std::ffi::CString> {
    std::ffi::CString::new(path.as_bytes())
        .map_err(|_| invalid_path_error("path contains a NUL byte"))
}

#[cfg(unix)]
pub fn statfs_sync(path: &str) -> io::Result<StatFs> {
    let path = c_path(path)?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(path.as_ptr(), &mut stat) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(StatFs {
        block_size: stat.f_bsize as u64,
        blocks: stat.f_blocks as f64,
        blocks_free: stat.f_bfree as f64,
        blocks_available: stat.f_bavail as f64,
        files: stat.f_files as f64,
        files_free: stat.f_ffree as f64,
    })
}

#[cfg(not(unix))]
pub fn statfs_sync(_path: &str) -> io::Result<StatFs> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "statfs is unsupported on this platform",
    ))
}
pub async fn statfs(path: &str) -> io::Result<StatFs> {
    statfs_sync(path)
}

#[cfg(unix)]
fn permissions(mode: u32) -> fs::Permissions {
    use std::os::unix::fs::PermissionsExt;
    fs::Permissions::from_mode(mode)
}
#[cfg(not(unix))]
fn permissions(mode: u32) -> fs::Permissions {
    fs::Permissions::from_readonly(mode & 0o222 == 0)
}

pub fn chmod_sync(path: &str, mode: u32) -> io::Result<()> {
    fs::set_permissions(path, permissions(mode))
}
pub async fn chmod(path: &str, mode: u32) -> io::Result<()> {
    async_fs::set_permissions(path, permissions(mode)).await
}
pub fn fchmod_sync(file: &File, mode: u32) -> io::Result<()> {
    block_on(file.set_permissions(permissions(mode)))
}
pub async fn fchmod(file: &File, mode: u32) -> io::Result<()> {
    file.set_permissions(permissions(mode)).await
}

#[cfg(unix)]
pub fn lchmod_sync(path: &str, mode: u32) -> io::Result<()> {
    let path = c_path(path)?;
    if unsafe {
        libc::fchmodat(
            libc::AT_FDCWD,
            path.as_ptr(),
            mode as libc::mode_t,
            libc::AT_SYMLINK_NOFOLLOW,
        )
    } == 0
    {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
#[cfg(not(unix))]
pub fn lchmod_sync(_path: &str, _mode: u32) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "lchmod is unsupported on this platform",
    ))
}
pub async fn lchmod(path: &str, mode: u32) -> io::Result<()> {
    lchmod_sync(path, mode)
}

#[cfg(unix)]
fn set_owner(path: &str, uid: u32, gid: u32, flags: libc::c_int) -> io::Result<()> {
    let path = c_path(path)?;
    if unsafe { libc::fchownat(libc::AT_FDCWD, path.as_ptr(), uid, gid, flags) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
#[cfg(unix)]
pub fn chown_sync(path: &str, uid: u32, gid: u32) -> io::Result<()> {
    set_owner(path, uid, gid, 0)
}
#[cfg(unix)]
pub fn lchown_sync(path: &str, uid: u32, gid: u32) -> io::Result<()> {
    set_owner(path, uid, gid, libc::AT_SYMLINK_NOFOLLOW)
}
#[cfg(not(unix))]
pub fn chown_sync(_path: &str, _uid: u32, _gid: u32) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "chown is unsupported on this platform",
    ))
}
#[cfg(not(unix))]
pub fn lchown_sync(_path: &str, _uid: u32, _gid: u32) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "lchown is unsupported on this platform",
    ))
}
pub async fn chown(path: &str, uid: u32, gid: u32) -> io::Result<()> {
    chown_sync(path, uid, gid)
}
pub async fn lchown(path: &str, uid: u32, gid: u32) -> io::Result<()> {
    lchown_sync(path, uid, gid)
}

#[cfg(unix)]
pub fn fchown_sync(file: &File, uid: u32, gid: u32) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    if unsafe { libc::fchown(file.as_raw_fd(), uid, gid) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
#[cfg(not(unix))]
pub fn fchown_sync(_file: &File, _uid: u32, _gid: u32) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "fchown is unsupported on this platform",
    ))
}
pub async fn fchown(file: &File, uid: u32, gid: u32) -> io::Result<()> {
    fchown_sync(file, uid, gid)
}

fn file_times(atime: SystemTime, mtime: SystemTime) -> (filetime::FileTime, filetime::FileTime) {
    (
        filetime::FileTime::from_system_time(atime),
        filetime::FileTime::from_system_time(mtime),
    )
}
pub fn utimes_sync(path: &str, atime: SystemTime, mtime: SystemTime) -> io::Result<()> {
    let (atime, mtime) = file_times(atime, mtime);
    filetime::set_file_times(path, atime, mtime)
}
pub async fn utimes(path: &str, atime: SystemTime, mtime: SystemTime) -> io::Result<()> {
    utimes_sync(path, atime, mtime)
}
pub fn lutimes_sync(path: &str, atime: SystemTime, mtime: SystemTime) -> io::Result<()> {
    let (atime, mtime) = file_times(atime, mtime);
    filetime::set_symlink_file_times(path, atime, mtime)
}
pub async fn lutimes(path: &str, atime: SystemTime, mtime: SystemTime) -> io::Result<()> {
    lutimes_sync(path, atime, mtime)
}

#[cfg(unix)]
pub fn futimes_sync(file: &File, atime: SystemTime, mtime: SystemTime) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    let (atime, mtime) = file_times(atime, mtime);
    let times = [
        libc::timespec {
            tv_sec: atime.unix_seconds() as libc::time_t,
            tv_nsec: atime.nanoseconds() as _,
        },
        libc::timespec {
            tv_sec: mtime.unix_seconds() as libc::time_t,
            tv_nsec: mtime.nanoseconds() as _,
        },
    ];
    if unsafe { libc::futimens(file.as_raw_fd(), times.as_ptr()) } == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}
#[cfg(not(unix))]
pub fn futimes_sync(_file: &File, _atime: SystemTime, _mtime: SystemTime) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "futimes is unsupported on this platform",
    ))
}
pub async fn futimes(file: &File, atime: SystemTime, mtime: SystemTime) -> io::Result<()> {
    futimes_sync(file, atime, mtime)
}

pub fn fsync_sync(file: &File) -> io::Result<()> {
    block_on(file.sync_all())
}

pub async fn fsync(file: &File) -> io::Result<()> {
    file.sync_all().await
}

pub fn fdatasync_sync(file: &File) -> io::Result<()> {
    block_on(file.sync_data())
}

pub async fn fdatasync(file: &File) -> io::Result<()> {
    file.sync_data().await
}

pub fn ftruncate_sync(file: &File, size: u64) -> io::Result<()> {
    block_on(file.set_len(size))
}

pub async fn ftruncate(file: &File, size: u64) -> io::Result<()> {
    file.set_len(size).await
}

pub fn truncate_sync(path: &str, len: u64) -> io::Result<()> {
    fs::OpenOptions::new().write(true).open(path)?.set_len(len)
}

pub async fn truncate(path: &str, len: u64) -> io::Result<()> {
    let mut options = async_fs::OpenOptions::new();
    options.write(true);
    options.open(path).await?.set_len(len).await
}

pub fn create_dir(path: &str) -> io::Result<()> {
    fs::create_dir(path)
}

pub fn create_dir_all(path: &str) -> io::Result<()> {
    fs::create_dir_all(path)
}

pub fn read_dir(path: &str) -> io::Result<Vec<String>> {
    fs::read_dir(path)?
        .map(|entry| {
            let name = entry?.file_name();
            name.into_string().map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "directory entry name is not valid UTF-8",
                )
            })
        })
        .collect()
}

fn path_string(path: PathBuf) -> io::Result<String> {
    path.into_os_string()
        .into_string()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "path is not valid UTF-8"))
}

pub fn mkdir_sync(path: &str, recursive: bool) -> io::Result<()> {
    if recursive {
        fs::create_dir_all(path)
    } else {
        fs::create_dir(path)
    }
}

pub async fn mkdir(path: &str, recursive: bool) -> io::Result<()> {
    if recursive {
        async_fs::create_dir_all(path).await
    } else {
        async_fs::create_dir(path).await
    }
}

pub fn mkdtemp_sync(prefix: &str) -> io::Result<String> {
    let path = Path::new(prefix);
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .ok_or_else(|| invalid_path_error("temporary directory prefix must include a name"))?;
    path_string(
        tempfile::Builder::new()
            .prefix(name)
            .tempdir_in(parent)?
            .keep(),
    )
}

pub async fn mkdtemp(prefix: &str) -> io::Result<String> {
    mkdtemp_sync(prefix)
}

pub fn mkdtemp_disposable_sync(prefix: &str) -> io::Result<TempDir> {
    Ok(TempDir {
        path: mkdtemp_sync(prefix)?,
    })
}

pub fn readdir_sync(path: &str) -> io::Result<Vec<String>> {
    read_dir(path)
}

pub async fn readdir(path: &str) -> io::Result<Vec<String>> {
    let mut entries = async_fs::read_dir(path).await?;
    let mut names = Vec::new();
    while let Some(entry) = entries.next().await {
        names.push(entry?.file_name().into_string().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "directory entry name is not valid UTF-8",
            )
        })?);
    }
    Ok(names)
}

pub fn opendir_sync(path: &str) -> io::Result<super::api::Dir> {
    block_on(async_fs::read_dir(path)).map(super::api::Dir::from_inner)
}
pub async fn opendir(path: &str) -> io::Result<super::api::Dir> {
    async_fs::read_dir(path)
        .await
        .map(super::api::Dir::from_inner)
}

async fn dirent(entry: async_fs::DirEntry) -> io::Result<Dirent> {
    let name = entry.file_name().into_string().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "directory entry name is not valid UTF-8",
        )
    })?;
    let kind = entry.file_type().await?;
    Ok(Dirent::from_parts(
        name,
        kind.is_file(),
        kind.is_dir(),
        kind.is_symlink(),
    ))
}

pub fn dir_read_sync(dir: &mut Dir) -> io::Result<Option<Dirent>> {
    block_on(dir_read(dir))
}

pub async fn dir_read(dir: &mut Dir) -> io::Result<Option<Dirent>> {
    match dir.next().await.transpose()? {
        Some(entry) => dirent(entry).await.map(Some),
        None => Ok(None),
    }
}

pub fn dir_close_sync(_dir: &Dir) -> io::Result<()> {
    Ok(())
}

pub async fn dir_close(_dir: &Dir) -> io::Result<()> {
    Ok(())
}

pub fn rmdir_sync(path: &str) -> io::Result<()> {
    fs::remove_dir(path)
}
pub async fn rmdir(path: &str) -> io::Result<()> {
    async_fs::remove_dir(path).await
}

pub fn rm_sync(path: &str, recursive: bool, force: bool) -> io::Result<()> {
    let result = match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => {
            if recursive {
                fs::remove_dir_all(path)
            } else {
                fs::remove_dir(path)
            }
        }
        Ok(_) => fs::remove_file(path),
        Err(error) => Err(error),
    };
    if force
        && result
            .as_ref()
            .is_err_and(|error| error.kind() == io::ErrorKind::NotFound)
    {
        Ok(())
    } else {
        result
    }
}

pub async fn rm(path: &str, recursive: bool, force: bool) -> io::Result<()> {
    let result = match async_fs::symlink_metadata(path).await {
        Ok(metadata) if metadata.is_dir() => {
            if recursive {
                async_fs::remove_dir_all(path).await
            } else {
                async_fs::remove_dir(path).await
            }
        }
        Ok(_) => async_fs::remove_file(path).await,
        Err(error) => Err(error),
    };
    if force
        && result
            .as_ref()
            .is_err_and(|error| error.kind() == io::ErrorKind::NotFound)
    {
        Ok(())
    } else {
        result
    }
}

pub fn realpath_sync(path: &str) -> io::Result<String> {
    path_string(fs::canonicalize(path)?)
}
pub async fn realpath(path: &str) -> io::Result<String> {
    path_string(async_fs::canonicalize(path).await?)
}

pub fn glob_sync(pattern: &str) -> io::Result<Vec<String>> {
    glob::glob(pattern)
        .map_err(|error| invalid_path_error(&error.to_string()))?
        .map(|entry| {
            let path = path_string(entry.map_err(io::Error::other)?)?;
            Ok(if Path::new(pattern).is_absolute() {
                path.trim_start_matches('/').to_owned()
            } else {
                path
            })
        })
        .collect()
}

pub async fn glob(pattern: &str) -> io::Result<Vec<String>> {
    glob_sync(pattern)
}

pub fn glob_to_regex(pattern: &str) -> io::Result<String> {
    glob::Pattern::new(pattern).map_err(|error| invalid_path_error(&error.to_string()))?;
    let mut result = String::from("^");
    let mut chars = pattern.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '*' if chars.peek() == Some(&'*') => {
                chars.next();
                if chars.peek() == Some(&'/') {
                    chars.next();
                    result.push_str("(?:.*/)?");
                } else {
                    result.push_str(".*");
                }
            }
            '*' => result.push_str("[^/]*"),
            '?' => result.push_str("[^/]"),
            '[' => {
                result.push('[');
                if chars.peek() == Some(&'!') {
                    chars.next();
                    result.push('^');
                }
                for item in chars.by_ref() {
                    result.push(item);
                    if item == ']' {
                        break;
                    }
                }
            }
            '.' | '+' | '(' | ')' | '$' | '^' | '|' | '{' | '}' | '\\' => {
                result.push('\\');
                result.push(character);
            }
            _ => result.push(character),
        }
    }
    result.push('$');
    Ok(result)
}

pub fn normalize_path(path: &str) -> io::Result<String> {
    let mut parts = Vec::new();
    let absolute = Path::new(path).is_absolute();
    for component in Path::new(path).components() {
        match component {
            std::path::Component::CurDir | std::path::Component::RootDir => {}
            std::path::Component::ParentDir if parts.last().is_some_and(|part| *part != "..") => {
                parts.pop();
            }
            std::path::Component::ParentDir if !absolute => parts.push(".."),
            std::path::Component::Normal(part) => parts.push(
                part.to_str()
                    .ok_or_else(|| invalid_path_error("path is not valid UTF-8"))?,
            ),
            _ => {}
        }
    }
    let joined = parts.join("/");
    Ok(if absolute {
        format!("/{joined}")
    } else if joined.is_empty() {
        ".".to_owned()
    } else {
        joined
    })
}

pub fn remove_file(path: &str) -> io::Result<()> {
    fs::remove_file(path)
}

pub fn rename_sync(from: &str, to: &str) -> io::Result<()> {
    fs::rename(from, to)
}

pub async fn rename(from: &str, to: &str) -> io::Result<()> {
    async_fs::rename(from, to).await
}

pub fn link_sync(original: &str, link: &str) -> io::Result<()> {
    fs::hard_link(original, link)
}

pub async fn link(original: &str, link: &str) -> io::Result<()> {
    async_fs::hard_link(original, link).await
}

pub fn symlink_sync(target: &str, link: &str) -> io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(windows)]
    {
        let target_at_link = Path::new(link)
            .parent()
            .unwrap_or(Path::new("."))
            .join(target);
        if fs::metadata(target_at_link).is_ok_and(|metadata| metadata.is_dir()) {
            std::os::windows::fs::symlink_dir(target, link)
        } else {
            std::os::windows::fs::symlink_file(target, link)
        }
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = (target, link);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "symlinks are unsupported on this platform",
        ))
    }
}

pub async fn symlink(target: &str, link: &str) -> io::Result<()> {
    symlink_sync(target, link)
}

pub fn readlink_sync(path: &str) -> io::Result<String> {
    path_string(fs::read_link(path)?)
}

pub async fn readlink(path: &str) -> io::Result<String> {
    path_string(async_fs::read_link(path).await?)
}

pub fn unlink_sync(path: &str) -> io::Result<()> {
    fs::remove_file(path)
}

pub async fn unlink(path: &str) -> io::Result<()> {
    async_fs::remove_file(path).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn app_dir_creates_a_named_directory_under_the_data_directory() {
        let temp = tempdir().unwrap();

        let path = app_dir_in(temp.path(), "example-app").unwrap();

        assert_eq!(path, temp.path().join("example-app").to_str().unwrap());
        assert!(Path::new(&path).is_dir());
    }

    #[test]
    fn app_dir_rejects_path_components() {
        let temp = tempdir().unwrap();

        assert_eq!(
            app_dir_in(temp.path(), "../outside").unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(
            app_dir_in(temp.path(), "").unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }
}

#[cfg(test)]
#[path = "implementation_tests.rs"]
mod shared_tests;

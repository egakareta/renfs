use std::{
    fs,
    io::{self, IoSlice, IoSliceMut, Write as StdWrite},
    path::{Path, PathBuf},
};

use futures_lite::{
    future::block_on,
    io::{AsyncReadExt, AsyncWriteExt},
    stream::StreamExt,
};

use super::api::{Metadata, OpenOptions};

pub type File = async_fs::File;

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
    Ok(Metadata::from_parts(
        metadata.len(),
        metadata.is_file(),
        metadata.is_dir(),
        metadata.file_type().is_symlink(),
    ))
}

pub async fn fstat(file: &File) -> io::Result<Metadata> {
    let metadata = file.metadata().await?;
    Ok(Metadata::from_parts(
        metadata.len(),
        metadata.is_file(),
        metadata.is_dir(),
        false,
    ))
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

pub fn remove_file(path: &str) -> io::Result<()> {
    fs::remove_file(path)
}

pub fn rename(from: &str, to: &str) -> io::Result<()> {
    fs::rename(from, to)
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

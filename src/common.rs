//! Filesystem behavior shared by the native and localStorage backends.

use std::{
    io::{self, Write},
    path::{Path, PathBuf},
};

use super::api::{Metadata, TempDir};

#[cfg(not(target_arch = "wasm32"))]
use std::fs as platform_fs;
#[cfg(target_arch = "wasm32")]
use webfs::fs as platform_fs;

/// Constructs an invalid-input error used by filesystem operations.
pub fn invalid_path_error(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.to_owned())
}

/// Converts platform metadata into the public, platform-independent metadata type.
pub(crate) fn metadata_parts(metadata: platform_fs::Metadata) -> Metadata {
    Metadata::from_parts(
        metadata.len(),
        metadata.is_file(),
        metadata.is_dir(),
        metadata.file_type().is_symlink(),
    )
}

/// Converts a path to a UTF-8 string, reporting invalid platform paths.
pub(crate) fn path_string(path: PathBuf) -> io::Result<String> {
    path.into_os_string()
        .into_string()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "path is not valid UTF-8"))
}

/// Normalizes path components while preserving whether the path is absolute.
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

/// Converts a glob pattern into a regular-expression source string.
pub fn glob_to_regex(pattern: &str) -> io::Result<String> {
    #[cfg(not(target_arch = "wasm32"))]
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

/// Returns whether a path exists, preserving errors from the filesystem.
pub fn exists_sync(path: &str) -> io::Result<bool> {
    #[cfg(target_arch = "wasm32")]
    {
        platform_fs::try_exists(path)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Path::new(path).try_exists()
    }
}

pub(crate) fn check_access(metadata: &platform_fs::Metadata, mode: u32) -> io::Result<()> {
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

/// Checks whether a path can be accessed with the requested mode.
pub fn access_sync(path: &str, mode: u32) -> io::Result<()> {
    check_access(&platform_fs::metadata(path)?, mode)
}

/// Reads a file as UTF-8 text.
pub fn read_text(path: &str) -> io::Result<String> {
    platform_fs::read_to_string(path)
}

/// Replaces a file's contents with UTF-8 text.
pub fn write_text(path: &str, contents: &str) -> io::Result<()> {
    platform_fs::write(path, contents)
}

/// Appends UTF-8 text, creating the file when needed.
pub fn append_text(path: &str, contents: &str) -> io::Result<()> {
    platform_fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?
        .write_all(contents.as_bytes())
}

/// Appends bytes, creating the file when needed.
pub fn append_file_sync(path: &str, contents: &[u8]) -> io::Result<()> {
    platform_fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?
        .write_all(contents)
}

/// Reads an entire file into memory.
pub fn read_file_sync(path: &str) -> io::Result<Vec<u8>> {
    platform_fs::read(path)
}

/// Replaces a file's contents with bytes.
pub fn write_file_sync(path: &str, contents: &[u8]) -> io::Result<()> {
    platform_fs::write(path, contents)
}

/// Copies a file to another path.
pub fn copy_file_sync(from: &str, to: &str) -> io::Result<()> {
    platform_fs::copy(from, to).map(|_| ())
}

#[cfg(unix)]
fn create_symlink(from: &Path, to: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(platform_fs::read_link(from)?, to)
}

#[cfg(windows)]
fn create_symlink(from: &Path, to: &Path) -> io::Result<()> {
    let target = platform_fs::read_link(from)?;
    if platform_fs::metadata(from)?.is_dir() {
        std::os::windows::fs::symlink_dir(target, to)
    } else {
        std::os::windows::fs::symlink_file(target, to)
    }
}

#[cfg(target_arch = "wasm32")]
fn create_symlink(_from: &Path, _to: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "symbolic links are not supported by webfs",
    ))
}

#[cfg(not(any(unix, windows, target_arch = "wasm32")))]
fn create_symlink(_from: &Path, _to: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "copying symbolic links is unsupported on this platform",
    ))
}

/// Recursively copies files and directories, preserving directory permissions.
pub fn cp_sync(from: &str, to: &str) -> io::Result<()> {
    fn copy_path(from: &Path, to: &Path) -> io::Result<()> {
        let metadata = platform_fs::symlink_metadata(from)?;
        if metadata.is_dir() {
            platform_fs::create_dir_all(to)?;
            for entry in platform_fs::read_dir(from)? {
                let entry = entry?;
                copy_path(&entry.path(), &to.join(entry.file_name()))?;
            }
            platform_fs::set_permissions(to, metadata.permissions())
        } else if metadata.is_file() {
            platform_fs::copy(from, to).map(|_| ())
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

/// Creates a directory.
pub fn create_dir(path: &str) -> io::Result<()> {
    platform_fs::create_dir(path)
}

/// Recursively creates a directory.
pub fn create_dir_all(path: &str) -> io::Result<()> {
    platform_fs::create_dir_all(path)
}

/// Reads directory entry names, requiring UTF-8 names.
pub fn read_dir(path: &str) -> io::Result<Vec<String>> {
    platform_fs::read_dir(path)?
        .map(|entry| {
            entry?.file_name().into_string().map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "directory entry name is not valid UTF-8",
                )
            })
        })
        .collect()
}

/// Creates a directory, optionally creating missing parents.
pub fn mkdir_sync(path: &str, recursive: bool) -> io::Result<()> {
    if recursive {
        create_dir_all(path)
    } else {
        create_dir(path)
    }
}

/// Opens a directory and collects its entry names.
pub fn readdir_sync(path: &str) -> io::Result<Vec<String>> {
    read_dir(path)
}

/// Removes an empty directory.
pub fn rmdir_sync(path: &str) -> io::Result<()> {
    platform_fs::remove_dir(path)
}

/// Removes a file or directory, optionally recursively or ignoring missing paths.
pub fn rm_sync(path: &str, recursive: bool, force: bool) -> io::Result<()> {
    let result = match platform_fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_dir() => {
            if recursive {
                platform_fs::remove_dir_all(path)
            } else {
                platform_fs::remove_dir(path)
            }
        }
        Ok(_) => platform_fs::remove_file(path),
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

/// Resolves a path to its canonical absolute form.
pub fn realpath_sync(path: &str) -> io::Result<String> {
    path_string(platform_fs::canonicalize(path)?)
}

/// Removes a file.
pub fn remove_file(path: &str) -> io::Result<()> {
    platform_fs::remove_file(path)
}

/// Renames a file or directory.
pub fn rename_sync(from: &str, to: &str) -> io::Result<()> {
    platform_fs::rename(from, to)
}

/// Creates a hard link.
pub fn link_sync(original: &str, link: &str) -> io::Result<()> {
    platform_fs::hard_link(original, link)
}

/// Creates a symbolic link where supported.
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
        if platform_fs::metadata(target_at_link).is_ok_and(|metadata| metadata.is_dir()) {
            std::os::windows::fs::symlink_dir(target, link)
        } else {
            std::os::windows::fs::symlink_file(target, link)
        }
    }
    #[cfg(target_arch = "wasm32")]
    {
        let _ = (target, link);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "symbolic links are not supported by webfs",
        ))
    }
    #[cfg(not(any(unix, windows, target_arch = "wasm32")))]
    {
        let _ = (target, link);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "symbolic links are unsupported on this platform",
        ))
    }
}

/// Reads the target of a symbolic link.
pub fn readlink_sync(path: &str) -> io::Result<String> {
    path_string(platform_fs::read_link(path)?)
}

/// Removes a file.
pub fn unlink_sync(path: &str) -> io::Result<()> {
    platform_fs::remove_file(path)
}

/// Truncates a file to the specified length.
pub fn truncate_sync(path: &str, len: u64) -> io::Result<()> {
    platform_fs::OpenOptions::new()
        .write(true)
        .open(path)?
        .set_len(len)
}

/// Reads metadata for a filesystem path, following symbolic links.
pub fn stat_sync(path: &str) -> io::Result<Metadata> {
    platform_fs::metadata(path).map(metadata_parts)
}

/// Reads metadata for a filesystem path without following symbolic links.
pub fn lstat_sync(path: &str) -> io::Result<Metadata> {
    platform_fs::symlink_metadata(path).map(metadata_parts)
}

/// Creates the named application directory.
pub fn app_dir(name: &str) -> io::Result<String> {
    validate_app_name(name)?;
    #[cfg(not(target_arch = "wasm32"))]
    let base = dirs::data_local_dir().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "could not determine the application data directory",
        )
    })?;
    #[cfg(target_arch = "wasm32")]
    let base = PathBuf::from("/");

    let path = base.join(name);
    platform_fs::create_dir_all(&path)?;
    path_string(path)
}

fn validate_app_name(name: &str) -> io::Result<()> {
    let name_path = Path::new(name);
    if name.contains('/')
        || name.contains('\\')
        || !matches!(
            name_path.components().next(),
            Some(std::path::Component::Normal(_))
        )
        || name_path.components().count() != 1
    {
        return Err(invalid_path_error(
            "application name must be a single path component",
        ));
    }
    Ok(())
}

/// Creates a temporary directory that is removed when dropped.
pub fn mkdtemp_disposable_sync(prefix: &str) -> io::Result<TempDir> {
    Ok(TempDir {
        path: mkdtemp_sync(prefix)?,
    })
}

/// Creates a uniquely named temporary directory by appending a suffix to `prefix`.
pub fn mkdtemp_sync(prefix: &str) -> io::Result<String> {
    #[cfg(not(target_arch = "wasm32"))]
    {
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
    #[cfg(target_arch = "wasm32")]
    {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let base = Path::new(prefix);
        let name = base
            .file_name()
            .ok_or_else(|| invalid_path_error("temporary directory prefix must include a name"))?
            .to_string_lossy();
        let parent = base.parent().unwrap_or(Path::new("/"));
        for _ in 0..100 {
            let suffix = NEXT_ID.fetch_add(1, Ordering::Relaxed);
            let candidate = parent.join(format!("{name}{suffix:x}"));
            match platform_fs::create_dir(&candidate) {
                Ok(()) => return path_string(candidate),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(error),
            }
        }
        Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "could not allocate a unique temporary directory",
        ))
    }
}

#[cfg(target_arch = "wasm32")]
#[path = "webfs.rs"]
mod webfs_impl;

#[cfg(target_arch = "wasm32")]
pub use webfs_impl::*;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn path_normalization_and_glob_conversion_are_shared() {
        assert_eq!(normalize_path("/a/./b/../c").unwrap(), "/a/c");
        assert_eq!(normalize_path("a/../../b").unwrap(), "../b");
        assert_eq!(glob_to_regex("**/*.rs").unwrap(), "^(?:.*/)?[^/]*\\.rs$");
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn app_directory_name_validation_is_shared() {
        assert!(validate_app_name("example-app").is_ok());
        assert_eq!(
            validate_app_name("../outside").unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }
}

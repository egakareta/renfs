//! `std::fs`-style synchronous filesystem operations for browser WebAssembly.
//!
//! Files are kept under a private key prefix in `localStorage`. Paths use a
//! virtual POSIX-style root: both `file.txt` and `/file.txt` name the same
//! file, and parent traversal above the root is rejected. Directory entries
//! and file contents are scoped to this crate's key prefix.

use base64::{Engine as _, engine::general_purpose::STANDARD};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    ffi::OsString,
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
    rc::Rc,
    time::SystemTime,
};
use wasm_bindgen::JsValue;
use web_sys::{Storage, Window};

const PREFIX: &str = "__webfs__/";
const FILE_PREFIX: &str = "__webfs__/file/";
const DIR_PREFIX: &str = "__webfs__/dir/";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Kind {
    File,
    Directory,
}

fn js_error(error: JsValue) -> io::Error {
    let message = error
        .as_string()
        .unwrap_or_else(|| format!("localStorage error: {error:?}"));
    io::Error::other(message)
}

fn storage() -> io::Result<Storage> {
    let window: Window = web_sys::window()
        .ok_or_else(|| io::Error::new(io::ErrorKind::Unsupported, "window is unavailable"))?;
    window
        .local_storage()
        .map_err(js_error)?
        .ok_or_else(|| io::Error::new(io::ErrorKind::Unsupported, "localStorage is unavailable"))
}

fn normalize_path(path: &Path) -> io::Result<String> {
    let path = path
        .to_str()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path is not valid UTF-8"))?;
    if path.contains('\0') {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path contains a NUL byte",
        ));
    }

    let mut parts = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => (),
            ".." if parts.pop().is_some() => (),
            ".." => {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "path escapes the virtual filesystem root",
                ));
            }
            part => parts.push(part),
        }
    }
    Ok(parts.join("/"))
}

fn file_key(path: &str) -> String {
    format!("{FILE_PREFIX}{path}")
}

fn dir_key(path: &str) -> String {
    format!("{DIR_PREFIX}{path}")
}

fn file_bytes(store: &Storage, path: &str) -> io::Result<Option<Vec<u8>>> {
    store
        .get_item(&file_key(path))
        .map_err(js_error)?
        .map(|value| {
            STANDARD.decode(value).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("invalid stored file contents: {error}"),
                )
            })
        })
        .transpose()
}

fn store_file(store: &Storage, path: &str, contents: &[u8]) -> io::Result<()> {
    store
        .set_item(&file_key(path), &STANDARD.encode(contents))
        .map_err(js_error)
}

fn read_open_file(path: &str) -> io::Result<Vec<u8>> {
    let store = storage()?;
    match kind(&store, path)? {
        Some(Kind::File) => {
            file_bytes(&store, path)?.ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
        }
        Some(Kind::Directory) => Err(io::Error::from(io::ErrorKind::IsADirectory)),
        None => Err(io::Error::from(io::ErrorKind::NotFound)),
    }
}

fn store_open_file(path: &str, contents: &[u8]) -> io::Result<()> {
    let store = storage()?;
    match kind(&store, path)? {
        Some(Kind::File) => store_file(&store, path, contents),
        Some(Kind::Directory) => Err(io::Error::from(io::ErrorKind::IsADirectory)),
        None => Err(io::Error::from(io::ErrorKind::NotFound)),
    }
}

fn kind(store: &Storage, path: &str) -> io::Result<Option<Kind>> {
    if path.is_empty() {
        return Ok(Some(Kind::Directory));
    }
    if store.get_item(&file_key(path)).map_err(js_error)?.is_some() {
        return Ok(Some(Kind::File));
    }
    if store.get_item(&dir_key(path)).map_err(js_error)?.is_some() {
        return Ok(Some(Kind::Directory));
    }
    Ok(None)
}

fn path_kind(path: &str) -> io::Result<Kind> {
    kind(&storage()?, path)?.ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))
}

fn ensure_parent(store: &Storage, path: &str) -> io::Result<()> {
    let parent = path.rsplit_once('/').map_or("", |(parent, _)| parent);
    match kind(store, parent)? {
        Some(Kind::Directory) => Ok(()),
        Some(Kind::File) => Err(io::Error::from(io::ErrorKind::NotADirectory)),
        None => Err(io::Error::from(io::ErrorKind::NotFound)),
    }
}

fn storage_keys(store: &Storage) -> io::Result<Vec<String>> {
    let mut keys = Vec::new();
    for index in 0..store.length().map_err(js_error)? {
        if let Some(key) = store.key(index).map_err(js_error)?
            && key.starts_with(PREFIX)
        {
            keys.push(key);
        }
    }
    Ok(keys)
}

/// Reads the entire contents of a file into a bytes vector.
pub fn read<P: AsRef<Path>>(path: P) -> io::Result<Vec<u8>> {
    let path = normalize_path(path.as_ref())?;
    let store = storage()?;
    match file_bytes(&store, &path)? {
        Some(contents) => Ok(contents),
        None if kind(&store, &path)? == Some(Kind::Directory) => {
            Err(io::Error::from(io::ErrorKind::IsADirectory))
        }
        None => Err(io::Error::from(io::ErrorKind::NotFound)),
    }
}

/// Reads the entire contents of a file into a string.
pub fn read_to_string<P: AsRef<Path>>(path: P) -> io::Result<String> {
    String::from_utf8(read(path)?)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

/// Writes a slice as the entire contents of a file.
pub fn write<P: AsRef<Path>, C: AsRef<[u8]>>(path: P, contents: C) -> io::Result<()> {
    let path = normalize_path(path.as_ref())?;
    let store = storage()?;
    ensure_parent(&store, &path)?;
    match kind(&store, &path)? {
        Some(Kind::Directory) => Err(io::Error::from(io::ErrorKind::IsADirectory)),
        _ => store_file(&store, &path, contents.as_ref()),
    }
}

/// Creates a new, empty directory at the provided path.
pub fn create_dir<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let path = normalize_path(path.as_ref())?;
    if path.is_empty() {
        return Err(io::Error::from(io::ErrorKind::AlreadyExists));
    }
    let store = storage()?;
    ensure_parent(&store, &path)?;
    if kind(&store, &path)?.is_some() {
        return Err(io::Error::from(io::ErrorKind::AlreadyExists));
    }
    store.set_item(&dir_key(&path), "1").map_err(js_error)
}

/// Recursively create a directory and all of its parent components if they are missing.
pub fn create_dir_all<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let path = normalize_path(path.as_ref())?;
    let store = storage()?;
    let mut current = String::new();
    for component in path.split('/').filter(|component| !component.is_empty()) {
        if !current.is_empty() {
            current.push('/');
        }
        current.push_str(component);
        match kind(&store, &current)? {
            Some(Kind::Directory) => (),
            Some(Kind::File) => return Err(io::Error::from(io::ErrorKind::AlreadyExists)),
            None => {
                store.set_item(&dir_key(&current), "1").map_err(js_error)?;
            }
        }
    }
    Ok(())
}

/// Removes an empty directory.
pub fn remove_dir<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let path = normalize_path(path.as_ref())?;
    if path.is_empty() {
        return Err(io::Error::from(io::ErrorKind::PermissionDenied));
    }
    let store = storage()?;
    match kind(&store, &path)? {
        Some(Kind::File) => return Err(io::Error::from(io::ErrorKind::NotADirectory)),
        Some(Kind::Directory) => (),
        None => return Err(io::Error::from(io::ErrorKind::NotFound)),
    }
    if !read_dir_names(&store, &path)?.is_empty() {
        return Err(io::Error::from(io::ErrorKind::DirectoryNotEmpty));
    }
    store.remove_item(&dir_key(&path)).map_err(js_error)
}

/// Removes a file from the filesystem.
pub fn remove_file<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let path = normalize_path(path.as_ref())?;
    let store = storage()?;
    match kind(&store, &path)? {
        Some(Kind::File) => store.remove_item(&file_key(&path)).map_err(js_error),
        Some(Kind::Directory) => Err(io::Error::from(io::ErrorKind::IsADirectory)),
        None => Err(io::Error::from(io::ErrorKind::NotFound)),
    }
}

/// Removes a directory at this path, after removing all its contents. Use carefully!
pub fn remove_dir_all<P: AsRef<Path>>(path: P) -> io::Result<()> {
    let path = normalize_path(path.as_ref())?;
    if path.is_empty() {
        return Err(io::Error::from(io::ErrorKind::PermissionDenied));
    }
    let store = storage()?;
    match kind(&store, &path)? {
        Some(Kind::Directory) => (),
        Some(Kind::File) => return Err(io::Error::from(io::ErrorKind::NotADirectory)),
        None => return Err(io::Error::from(io::ErrorKind::NotFound)),
    }
    let boundary = format!("{path}/");
    for key in storage_keys(&store)? {
        let relative = key
            .strip_prefix(FILE_PREFIX)
            .or_else(|| key.strip_prefix(DIR_PREFIX))
            .unwrap_or_default();
        if relative == path || relative.starts_with(&boundary) {
            store.remove_item(&key).map_err(js_error)?;
        }
    }
    Ok(())
}

fn read_dir_names(store: &Storage, path: &str) -> io::Result<Vec<(String, Kind)>> {
    let base = if path.is_empty() {
        String::new()
    } else {
        format!("{path}/")
    };
    let mut names = BTreeMap::<String, Kind>::new();
    for key in storage_keys(store)? {
        let (prefix, key_kind) = if key.starts_with(FILE_PREFIX) {
            (FILE_PREFIX, Kind::File)
        } else if key.starts_with(DIR_PREFIX) {
            (DIR_PREFIX, Kind::Directory)
        } else {
            continue;
        };
        let Some(relative) = key
            .strip_prefix(prefix)
            .and_then(|key| key.strip_prefix(&base))
        else {
            continue;
        };
        if relative.is_empty() {
            continue;
        }
        let (name, nested) = relative
            .split_once('/')
            .map_or((relative, false), |(name, _)| (name, true));
        if name.is_empty() {
            continue;
        }
        let child_kind = if nested { Kind::Directory } else { key_kind };
        names
            .entry(name.to_owned())
            .and_modify(|existing| {
                if child_kind == Kind::Directory {
                    *existing = Kind::Directory;
                }
            })
            .or_insert(child_kind);
    }
    Ok(names.into_iter().collect())
}

/// Returns an iterator over the entries within a directory.
pub fn read_dir<P: AsRef<Path>>(path: P) -> io::Result<ReadDir> {
    let original = path.as_ref();
    let path = normalize_path(original)?;
    let store = storage()?;
    match kind(&store, &path)? {
        Some(Kind::Directory) => (),
        Some(Kind::File) => return Err(io::Error::from(io::ErrorKind::NotADirectory)),
        None => return Err(io::Error::from(io::ErrorKind::NotFound)),
    }
    let entries = read_dir_names(&store, &path)?
        .into_iter()
        .map(|(name, kind)| {
            let entry_path = original.join(&name);
            let entry_name = OsString::from(&name);
            let virtual_path = if path.is_empty() {
                name
            } else {
                format!("{path}/{name}")
            };
            Ok(DirEntry {
                path: entry_path,
                virtual_path,
                name: entry_name,
                kind,
            })
        })
        .collect::<Vec<_>>()
        .into_iter();
    Ok(ReadDir { entries })
}

/// Given a path, queries the file system to get information about a file, directory, etc.
pub fn metadata<P: AsRef<Path>>(path: P) -> io::Result<Metadata> {
    metadata_impl(path.as_ref())
}

/// Returns metadata without following symbolic links.
///
/// This filesystem does not support symbolic links, so this is equivalent to
/// [`metadata`].
pub fn symlink_metadata<P: AsRef<Path>>(path: P) -> io::Result<Metadata> {
    metadata_impl(path.as_ref())
}

fn metadata_impl(path: &Path) -> io::Result<Metadata> {
    let path = normalize_path(path)?;
    let store = storage()?;
    match kind(&store, &path)? {
        Some(Kind::File) => Ok(Metadata::new(
            Kind::File,
            file_bytes(&store, &path)?.unwrap_or_default().len() as u64,
        )),
        Some(Kind::Directory) => Ok(Metadata::new(Kind::Directory, 0)),
        None => Err(io::Error::from(io::ErrorKind::NotFound)),
    }
}

/// Resolves a path to an absolute path in the virtual filesystem.
pub fn canonicalize<P: AsRef<Path>>(path: P) -> io::Result<PathBuf> {
    let path = normalize_path(path.as_ref())?;
    path_kind(&path)?;
    Ok(PathBuf::from(format!("/{path}")))
}

/// Returns whether a path currently exists, preserving errors from the store.
pub fn try_exists<P: AsRef<Path>>(path: P) -> io::Result<bool> {
    let path = normalize_path(path.as_ref())?;
    kind(&storage()?, &path).map(|kind| kind.is_some())
}

/// Returns `true` if the path points at an existing entity.
pub fn exists<P: AsRef<Path>>(path: P) -> bool {
    try_exists(path).unwrap_or(false)
}

/// Copies the contents of one file to another.
pub fn copy<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> io::Result<u64> {
    let contents = read(from)?;
    let length = contents.len() as u64;
    write(to, contents)?;
    Ok(length)
}

/// Renames a file or directory to a new name, replacing the original file if `to` already exists.
pub fn rename<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> io::Result<()> {
    let from = normalize_path(from.as_ref())?;
    let to = normalize_path(to.as_ref())?;
    if from.is_empty() || to.is_empty() {
        return Err(io::Error::from(io::ErrorKind::PermissionDenied));
    }
    let store = storage()?;
    let source_kind =
        kind(&store, &from)?.ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
    if from == to {
        return Ok(());
    }
    ensure_parent(&store, &to)?;
    if source_kind == Kind::Directory && to.starts_with(&format!("{from}/")) {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    match kind(&store, &to)? {
        Some(Kind::Directory) if source_kind == Kind::File => {
            return Err(io::Error::from(io::ErrorKind::IsADirectory));
        }
        Some(Kind::File) if source_kind == Kind::Directory => {
            return Err(io::Error::from(io::ErrorKind::NotADirectory));
        }
        Some(Kind::Directory) => {
            if !read_dir_names(&store, &to)?.is_empty() {
                return Err(io::Error::from(io::ErrorKind::DirectoryNotEmpty));
            }
            store.remove_item(&dir_key(&to)).map_err(js_error)?;
        }
        Some(Kind::File) => {
            // Replaced atomically with the source value below.
        }
        None => (),
    }

    if source_kind == Kind::File {
        let value = store
            .get_item(&file_key(&from))
            .map_err(js_error)?
            .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
        store.set_item(&file_key(&to), &value).map_err(js_error)?;
        store.remove_item(&file_key(&from)).map_err(js_error)
    } else {
        let boundary = format!("{from}/");
        let mut moves = Vec::new();
        for key in storage_keys(&store)? {
            let translated = if let Some(path) = key.strip_prefix(FILE_PREFIX) {
                (path == from || path.starts_with(&boundary))
                    .then(|| format!("{FILE_PREFIX}{to}{}", &path[from.len()..]))
            } else if let Some(path) = key.strip_prefix(DIR_PREFIX) {
                (path == from || path.starts_with(&boundary))
                    .then(|| format!("{DIR_PREFIX}{to}{}", &path[from.len()..]))
            } else {
                None
            };
            if let Some(new_key) = translated {
                let value = store
                    .get_item(&key)
                    .map_err(js_error)?
                    .ok_or_else(|| io::Error::from(io::ErrorKind::NotFound))?;
                moves.push((key, new_key, value));
            }
        }
        for (_, new_key, value) in &moves {
            store.set_item(new_key, value).map_err(js_error)?;
        }
        for (old_key, _, _) in moves {
            store.remove_item(&old_key).map_err(js_error)?;
        }
        Ok(())
    }
}

/// Creates a new hard link on the filesystem.
///
/// Hard links are not supported by the localStorage-backed filesystem.
pub fn hard_link<P: AsRef<Path>, Q: AsRef<Path>>(_original: P, _link: Q) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "hard links are not supported",
    ))
}

/// Reads a symbolic link, returning the file that the link points to.
///
/// Symbolic links are not supported by the localStorage-backed filesystem.
pub fn read_link<P: AsRef<Path>>(_path: P) -> io::Result<PathBuf> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "symbolic links are not supported",
    ))
}

/// Changes the permissions found on a file or a directory.
///
/// Browser localStorage does not expose filesystem permissions, so this only
/// verifies that the path exists.
pub fn set_permissions<P: AsRef<Path>>(path: P, _permissions: Permissions) -> io::Result<()> {
    path_kind(&normalize_path(path.as_ref())?).map(|_| ())
}

/// Changes the timestamps of a file or directory.
///
/// `localStorage` does not retain timestamps, so this verifies that `path`
/// exists without changing metadata.
pub fn set_times<P: AsRef<Path>>(path: P, _times: FileTimes) -> io::Result<()> {
    path_kind(&normalize_path(path.as_ref())?).map(|_| ())
}

/// Options and flags which can be used to configure how a file is opened.
#[derive(Clone, Debug, Default)]
pub struct OpenOptions {
    read: bool,
    write: bool,
    append: bool,
    truncate: bool,
    create: bool,
    create_new: bool,
}

impl OpenOptions {
    /// Creates a blank new set of options ready for configuration.
    ///
    /// All options are initially set to `false`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the option for read access.
    pub fn read(&mut self, read: bool) -> &mut Self {
        self.read = read;
        self
    }

    /// Sets the option for write access.
    pub fn write(&mut self, write: bool) -> &mut Self {
        self.write = write;
        self
    }

    /// Sets the option for the append mode.
    pub fn append(&mut self, append: bool) -> &mut Self {
        self.append = append;
        self
    }

    /// Sets the option for truncating a previous file.
    pub fn truncate(&mut self, truncate: bool) -> &mut Self {
        self.truncate = truncate;
        self
    }

    /// Sets the option to create a new file, or open it if it already exists.
    pub fn create(&mut self, create: bool) -> &mut Self {
        self.create = create;
        self
    }

    /// Sets the option to create a new file, failing if it already exists.
    pub fn create_new(&mut self, create_new: bool) -> &mut Self {
        self.create_new = create_new;
        self
    }

    /// Opens a file at `path` with the options specified by `self`.
    pub fn open<P: AsRef<Path>>(&self, path: P) -> io::Result<File> {
        if !(self.read || self.write || self.append) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "at least one of read, write, or append must be enabled",
            ));
        }
        if (self.truncate || self.create || self.create_new) && !(self.write || self.append) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "create and truncate require write or append access",
            ));
        }

        let path = normalize_path(path.as_ref())?;
        let store = storage()?;
        ensure_parent(&store, &path)?;
        let existing_kind = kind(&store, &path)?;
        if existing_kind == Some(Kind::Directory) {
            return Err(io::Error::from(io::ErrorKind::IsADirectory));
        }
        if self.create_new && existing_kind.is_some() {
            return Err(io::Error::from(io::ErrorKind::AlreadyExists));
        }
        if existing_kind.is_none() && !(self.create || self.create_new) {
            return Err(io::Error::from(io::ErrorKind::NotFound));
        }

        let mut contents = file_bytes(&store, &path)?.unwrap_or_default();
        if self.truncate {
            contents.clear();
            store_file(&store, &path, &contents)?;
        } else if existing_kind.is_none() {
            store_file(&store, &path, &contents)?;
        }
        let cursor = if self.append { contents.len() } else { 0 };
        Ok(File {
            path,
            contents: Rc::new(RefCell::new(contents)),
            cursor: Rc::new(Cell::new(cursor)),
            readable: self.read,
            writable: self.write || self.append,
            append: self.append,
        })
    }
}

/// An object providing access to an open file on the filesystem.
#[derive(Debug)]
pub struct File {
    path: String,
    contents: Rc<RefCell<Vec<u8>>>,
    cursor: Rc<Cell<usize>>,
    readable: bool,
    writable: bool,
    append: bool,
}

impl File {
    /// Attempts to open a file in read-only mode.
    pub fn open<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut options = OpenOptions::new();
        options.read(true).open(path)
    }

    /// Opens a file in write-only mode, creating it if it does not exist and truncating it if it does.
    pub fn create<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut options = OpenOptions::new();
        options.write(true).create(true).truncate(true).open(path)
    }

    /// Creates a new file in read-write mode; error if the file exists.
    pub fn create_new<P: AsRef<Path>>(path: P) -> io::Result<Self> {
        let mut options = OpenOptions::new();
        options.read(true).write(true).create_new(true).open(path)
    }

    /// Returns a new `OpenOptions` object.
    pub fn options() -> OpenOptions {
        OpenOptions::new()
    }

    /// Queries metadata about the underlying file.
    pub fn metadata(&self) -> io::Result<Metadata> {
        Ok(Metadata::new(
            Kind::File,
            read_open_file(&self.path)?.len() as u64,
        ))
    }

    /// Creates a new `File` instance that shares the same underlying file handle as the existing `File` instance.
    pub fn try_clone(&self) -> io::Result<Self> {
        Ok(Self {
            path: self.path.clone(),
            contents: Rc::clone(&self.contents),
            cursor: Rc::clone(&self.cursor),
            readable: self.readable,
            writable: self.writable,
            append: self.append,
        })
    }

    /// Truncates or extends the underlying file, updating the size of this file to become `size`.
    pub fn set_len(&self, size: u64) -> io::Result<()> {
        if !self.writable {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        let size = usize::try_from(size)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "file is too large"))?;
        let mut contents = read_open_file(&self.path)?;
        contents.resize(size, 0);
        store_open_file(&self.path, &contents)?;
        *self.contents.borrow_mut() = contents;
        Ok(())
    }

    /// Confirms that the file is still available.
    ///
    /// `localStorage` writes are synchronous, so this checks that the file
    /// still exists; there is no buffered data to flush.
    pub fn sync_all(&self) -> io::Result<()> {
        read_open_file(&self.path).map(|_| ())
    }

    /// Confirms that the file is still available.
    ///
    /// `localStorage` writes are synchronous, so there is no buffered data to
    /// flush. This is equivalent to [`File::sync_all`].
    pub fn sync_data(&self) -> io::Result<()> {
        self.sync_all()
    }

    /// Changes the permissions of the file.
    pub fn set_permissions(&self, permissions: Permissions) -> io::Result<()> {
        set_permissions(&self.path, permissions)
    }

    /// Changes the timestamps of the underlying file.
    ///
    /// `localStorage` does not retain timestamps, so this does not change metadata.
    pub fn set_times(&self, _times: FileTimes) -> io::Result<()> {
        path_kind(&self.path).map(|_| ())
    }

    /// Changes the modification time of the underlying file.
    ///
    /// `localStorage` does not retain timestamps, so this does not change metadata.
    pub fn set_modified(&self, time: SystemTime) -> io::Result<()> {
        self.set_times(FileTimes::new().set_modified(time))
    }
}

impl Read for File {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if !self.readable {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        let current = read_open_file(&self.path)?;
        *self.contents.borrow_mut() = current.clone();
        let contents = self.contents.borrow();
        let cursor = self.cursor.get();
        let count = buffer.len().min(contents.len().saturating_sub(cursor));
        buffer[..count].copy_from_slice(&contents[cursor..cursor + count]);
        self.cursor.set(cursor + count);
        Ok(count)
    }
}

impl Write for File {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if !self.writable {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        let current = read_open_file(&self.path)?;
        let start = if self.append {
            current.len()
        } else {
            self.cursor.get()
        };
        let end = start
            .checked_add(buffer.len())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "file is too large"))?;
        let mut contents = current;
        if end > contents.len() {
            contents.resize(end, 0);
        }
        contents[start..end].copy_from_slice(buffer);
        store_open_file(&self.path, &contents)?;
        *self.contents.borrow_mut() = contents;
        self.cursor.set(end);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.sync_all()
    }
}

impl Seek for File {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        let contents = self.contents.borrow();
        let cursor = self.cursor.get();
        let next = match position {
            SeekFrom::Start(position) => i128::from(position),
            SeekFrom::End(offset) => contents.len() as i128 + i128::from(offset),
            SeekFrom::Current(offset) => cursor as i128 + i128::from(offset),
        };
        if next < 0 || next > usize::MAX as i128 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "invalid seek position",
            ));
        }
        self.cursor.set(next as usize);
        Ok(next as u64)
    }
}

/// Metadata information about a file.
#[derive(Clone, Debug)]
pub struct Metadata {
    kind: Kind,
    len: u64,
}

impl Metadata {
    fn new(kind: Kind, len: u64) -> Self {
        Self { kind, len }
    }

    /// Returns `true` if this metadata is for a regular file.
    pub fn is_file(&self) -> bool {
        self.kind == Kind::File
    }

    /// Returns `true` if this metadata is for a directory.
    pub fn is_dir(&self) -> bool {
        self.kind == Kind::Directory
    }

    /// Returns the size of the file, in bytes.
    pub fn len(&self) -> u64 {
        self.len
    }

    /// Returns `true` if the file size is zero bytes.
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the type of this file.
    pub fn file_type(&self) -> FileType {
        FileType { kind: self.kind }
    }

    /// Returns the last modification time.
    ///
    /// localStorage does not retain timestamps; this returns the Unix epoch.
    pub fn modified(&self) -> io::Result<SystemTime> {
        Ok(SystemTime::UNIX_EPOCH)
    }

    /// Returns the last access time.
    ///
    /// localStorage does not retain timestamps; this returns the Unix epoch.
    pub fn accessed(&self) -> io::Result<SystemTime> {
        Ok(SystemTime::UNIX_EPOCH)
    }

    /// Returns the creation time.
    ///
    /// localStorage does not retain timestamps; this returns the Unix epoch.
    pub fn created(&self) -> io::Result<SystemTime> {
        Ok(SystemTime::UNIX_EPOCH)
    }

    /// Returns the permissions of this file.
    pub fn permissions(&self) -> Permissions {
        Permissions { readonly: false }
    }
}

/// Representation of the various timestamps on a file.
#[derive(Clone, Copy, Debug, Default)]
pub struct FileTimes {
    _accessed: Option<SystemTime>,
    _modified: Option<SystemTime>,
}

impl FileTimes {
    /// Creates a new `FileTimes` with no times set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the last access time of a file.
    pub fn set_accessed(mut self, time: SystemTime) -> Self {
        self._accessed = Some(time);
        self
    }

    /// Sets the last modified time of a file.
    pub fn set_modified(mut self, time: SystemTime) -> Self {
        self._modified = Some(time);
        self
    }
}

/// A structure representing a type of file with accessors for each file type.
///
/// It is returned by [`Metadata::file_type`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FileType {
    kind: Kind,
}

impl FileType {
    /// Returns `true` if this file type is a directory.
    pub fn is_dir(&self) -> bool {
        self.kind == Kind::Directory
    }

    /// Returns `true` if this file type is a regular file.
    pub fn is_file(&self) -> bool {
        self.kind == Kind::File
    }

    /// Returns `true` if this file type is a symbolic link.
    pub fn is_symlink(&self) -> bool {
        false
    }
}

/// Representation of the various permissions on a file.
#[derive(Clone, Debug, Default)]
pub struct Permissions {
    readonly: bool,
}

impl Permissions {
    /// Returns `true` if these permissions describe a read-only file.
    pub fn readonly(&self) -> bool {
        self.readonly
    }

    /// Sets the read-only flag.
    ///
    /// localStorage has no per-entry permission controls, so the flag is
    /// accepted for API compatibility but does not affect stored data.
    pub fn set_readonly(&mut self, readonly: bool) {
        self.readonly = readonly;
    }
}

/// Iterator over the entries in a directory.
#[derive(Debug)]
pub struct ReadDir {
    entries: std::vec::IntoIter<io::Result<DirEntry>>,
}

impl Iterator for ReadDir {
    type Item = io::Result<DirEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        self.entries.next()
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.entries.size_hint()
    }
}

impl ExactSizeIterator for ReadDir {}

/// Entries returned by the [`ReadDir`] iterator.
#[derive(Clone, Debug)]
pub struct DirEntry {
    path: PathBuf,
    virtual_path: String,
    name: OsString,
    kind: Kind,
}

impl DirEntry {
    /// Returns the full path to this entry.
    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }

    /// Returns the name of this directory entry.
    pub fn file_name(&self) -> OsString {
        self.name.clone()
    }

    /// Returns the type for this directory entry.
    pub fn file_type(&self) -> io::Result<FileType> {
        Ok(FileType { kind: self.kind })
    }

    /// Returns the metadata for this directory entry.
    pub fn metadata(&self) -> io::Result<Metadata> {
        metadata(&self.virtual_path)
    }
}

/// A builder used to create directories in various manners.
#[derive(Clone, Debug, Default)]
pub struct DirBuilder {
    recursive: bool,
}

impl DirBuilder {
    /// Creates a directory builder with default options.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets whether to create missing parent directories.
    pub fn recursive(&mut self, recursive: bool) -> &mut Self {
        self.recursive = recursive;
        self
    }

    /// Creates the specified directory.
    pub fn create<P: AsRef<Path>>(&self, path: P) -> io::Result<()> {
        if self.recursive {
            create_dir_all(path)
        } else {
            create_dir(path)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test;

    fn test_path(name: &str) -> String {
        format!("/webfs-tests/{name}")
    }

    #[wasm_bindgen_test]
    fn file_and_directory_operations_round_trip() {
        let root = test_path("round-trip");
        let _ = remove_dir_all(&root);
        create_dir_all(format!("{root}/nested")).unwrap();
        write(format!("{root}/nested/file.txt"), "hello").unwrap();
        assert_eq!(
            read_to_string(format!("{root}/nested/file.txt")).unwrap(),
            "hello"
        );
        assert_eq!(
            metadata(format!("{root}/nested/file.txt")).unwrap().len(),
            5
        );
        assert_eq!(read_dir(format!("{root}/nested")).unwrap().count(), 1);
        remove_dir_all(&root).unwrap();
        assert!(!try_exists(&root).unwrap());
    }

    #[wasm_bindgen_test]
    fn file_handles_support_read_write_seek_and_append() {
        let path = test_path("file-handle.bin");
        let _ = remove_file(&path);
        let mut file = File::create(&path).unwrap();
        file.write_all(b"abcd").unwrap();
        file.seek(SeekFrom::Start(1)).unwrap();
        file.write_all(b"Z").unwrap();
        file.sync_all().unwrap();
        assert_eq!(read(&path).unwrap(), b"aZcd");

        let mut options = OpenOptions::new();
        let mut file = options.append(true).open(&path).unwrap();
        file.write_all(b"!").unwrap();
        assert_eq!(read(&path).unwrap(), b"aZcd!");
        remove_file(path).unwrap();
    }

    #[wasm_bindgen_test]
    fn rename_moves_directory_trees() {
        let source = test_path("rename-source");
        let destination = test_path("rename-destination");
        let _ = remove_dir_all(&source);
        let _ = remove_dir_all(&destination);
        create_dir_all(format!("{source}/nested")).unwrap();
        write(format!("{source}/nested/file"), [0, 1, 2]).unwrap();
        rename(&source, &destination).unwrap();
        assert_eq!(
            read(format!("{destination}/nested/file")).unwrap(),
            [0, 1, 2]
        );
        assert!(!try_exists(source).unwrap());
        remove_dir_all(destination).unwrap();
    }
}

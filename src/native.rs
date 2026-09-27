use std::{fs, io, io::Write, path::Path};

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

pub fn exists(path: &str) -> io::Result<bool> {
    Path::new(path).try_exists()
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

pub fn read_file(path: &str) -> io::Result<Vec<u8>> {
    fs::read(path)
}

pub fn write_file(path: &str, contents: &[u8]) -> io::Result<()> {
    fs::write(path, contents)
}

pub fn copy_file(from: &str, to: &str) -> io::Result<()> {
    fs::copy(from, to).map(|_| ())
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

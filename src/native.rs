use std::{fs, io, path::Path};

pub fn exists(path: &str) -> io::Result<bool> {
    Path::new(path).try_exists()
}

pub fn read_text(path: &str) -> io::Result<String> {
    fs::read_to_string(path)
}

pub fn write_text(path: &str, contents: &str) -> io::Result<()> {
    fs::write(path, contents)
}

pub fn read_file(path: &str) -> io::Result<Vec<u8>> {
    fs::read(path)
}

pub fn write_file(path: &str, contents: &[u8]) -> io::Result<()> {
    fs::write(path, contents)
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn filesystem_operations_use_the_native_filesystem() {
        let temp = tempdir().unwrap();
        let nested_dir = temp.path().join("nested").join("directory");
        let nested_path = nested_dir.to_str().unwrap();
        let file = nested_dir.join("hello.txt");
        let file_path = file.to_str().unwrap();

        assert!(!exists(file_path).unwrap());
        create_dir_all(nested_path).unwrap();
        write_text(file_path, "hello from std::fs").unwrap();
        assert!(exists(file_path).unwrap());
        assert_eq!(read_text(file_path).unwrap(), "hello from std::fs");
        assert_eq!(read_file(file_path).unwrap(), b"hello from std::fs");

        write_file(file_path, b"bytes").unwrap();
        assert_eq!(read_file(file_path).unwrap(), b"bytes");
        assert!(
            read_dir(nested_path)
                .unwrap()
                .contains(&"hello.txt".to_owned())
        );

        remove_file(file_path).unwrap();
        assert!(!exists(file_path).unwrap());
    }

    #[test]
    fn create_dir_requires_existing_parent_and_rejects_existing_path() {
        let temp = tempdir().unwrap();
        let missing_parent = temp.path().join("missing").join("child");
        assert!(create_dir(missing_parent.to_str().unwrap()).is_err());

        let child = temp.path().join("child");

        create_dir(child.to_str().unwrap()).unwrap();
        assert!(child.is_dir());
        assert!(create_dir(child.to_str().unwrap()).is_err());
    }
}

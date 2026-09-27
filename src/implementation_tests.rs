use super::*;
use std::path::PathBuf;

struct TestDirectory {
    path: PathBuf,
    #[cfg(not(target_arch = "wasm32"))]
    _directory: tempfile::TempDir,
}

impl TestDirectory {
    fn new(_name: &str) -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let directory = tempfile::tempdir().unwrap();
            Self {
                path: directory.path().to_owned(),
                _directory: directory,
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            let path = PathBuf::from(format!(
                "/app/renfs-{_name}-{:x}",
                js_sys::Math::random().to_bits()
            ));
            create_dir_all(path.to_str().unwrap()).unwrap();
            Self { path }
        }
    }

    fn join(&self, name: &str) -> String {
        self.path.join(name).to_str().unwrap().to_owned()
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn filesystem_operations_round_trip_text_binary_and_directories() {
    let directory = TestDirectory::new("filesystem");
    let nested = directory.join("nested/directory");
    create_dir_all(&nested).unwrap();

    let text_file = PathBuf::from(&nested).join("message.txt");
    let text_path = text_file.to_str().unwrap();
    assert!(!exists(text_path).unwrap());

    write_text(text_path, "hello").unwrap();
    assert!(exists(text_path).unwrap());
    assert_eq!(read_text(text_path).unwrap(), "hello");
    assert_eq!(read_file(text_path).unwrap(), b"hello");

    append_text(text_path, " world").unwrap();
    assert_eq!(read_text(text_path).unwrap(), "hello world");

    let bytes = [0, 1, 127, 128, 255];
    let binary_file = PathBuf::from(&nested).join("bytes.bin");
    let binary_path = binary_file.to_str().unwrap();
    write_file(binary_path, &bytes).unwrap();
    assert_eq!(read_file(binary_path).unwrap(), bytes);
    assert!(read_dir(&nested).unwrap().contains(&"bytes.bin".to_owned()));

    let copy = PathBuf::from(&nested).join("copy.txt");
    let copy_path = copy.to_str().unwrap();
    copy_file(text_path, copy_path).unwrap();
    assert_eq!(read_text(copy_path).unwrap(), "hello world");

    let renamed = PathBuf::from(&nested).join("renamed.txt");
    let renamed_path = renamed.to_str().unwrap();
    rename(copy_path, renamed_path).unwrap();
    assert!(!exists(copy_path).unwrap());
    assert_eq!(read_text(renamed_path).unwrap(), "hello world");

    remove_file(renamed_path).unwrap();
    assert!(!exists(renamed_path).unwrap());
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn create_dir_only_creates_one_level_and_rejects_existing_paths() {
    let directory = TestDirectory::new("create-dir");
    let missing_parent = directory.join("missing/child");
    assert!(create_dir(&missing_parent).is_err());

    let child = directory.join("child");
    create_dir(&child).unwrap();
    assert!(exists(&child).unwrap());
    assert!(create_dir(&child).is_err());
}

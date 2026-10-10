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
    assert!(!exists_sync(text_path).unwrap());

    write_text(text_path, "hello").unwrap();
    assert!(exists_sync(text_path).unwrap());
    assert_eq!(read_text(text_path).unwrap(), "hello");
    assert_eq!(read_file_sync(text_path).unwrap(), b"hello");

    append_text(text_path, " world").unwrap();
    assert_eq!(read_text(text_path).unwrap(), "hello world");

    let bytes = [0, 1, 127, 128, 255];
    let binary_file = PathBuf::from(&nested).join("bytes.bin");
    let binary_path = binary_file.to_str().unwrap();
    write_file_sync(binary_path, &bytes).unwrap();
    assert_eq!(read_file_sync(binary_path).unwrap(), bytes);
    assert!(read_dir(&nested).unwrap().contains(&"bytes.bin".to_owned()));

    let copy = PathBuf::from(&nested).join("copy.txt");
    let copy_path = copy.to_str().unwrap();
    copy_file_sync(text_path, copy_path).unwrap();
    assert_eq!(read_text(copy_path).unwrap(), "hello world");

    let renamed = PathBuf::from(&nested).join("renamed.txt");
    let renamed_path = renamed.to_str().unwrap();
    rename_sync(copy_path, renamed_path).unwrap();
    assert!(!exists_sync(copy_path).unwrap());
    assert_eq!(read_text(renamed_path).unwrap(), "hello world");

    remove_file(renamed_path).unwrap();
    assert!(!exists_sync(renamed_path).unwrap());
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn create_dir_only_creates_one_level_and_rejects_existing_paths() {
    let directory = TestDirectory::new("create-dir");
    let missing_parent = directory.join("missing/child");
    assert!(create_dir(&missing_parent).is_err());

    let child = directory.join("child");
    create_dir(&child).unwrap();
    assert!(exists_sync(&child).unwrap());
    assert!(create_dir(&child).is_err());
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn file_access_copy_and_truncate_apis_work() {
    let directory = TestDirectory::new("file-apis");
    let source = directory.join("source.txt");
    let copy = directory.join("copy.txt");

    write_file_sync(&source, b"hello").unwrap();
    crate::fs::append_file_sync(&source, b" world").unwrap();
    crate::fs::access_sync(&source, 0).unwrap();
    assert!(crate::fs::exists_sync(&source).unwrap());

    crate::fs::cp_sync(&source, &copy).unwrap();
    assert_eq!(crate::fs::read_file_sync(&copy).unwrap(), b"hello world");
    crate::fs::truncate_sync(&copy, 5).unwrap();
    assert_eq!(read_file_sync(&copy).unwrap(), b"hello");

    let source_dir = directory.join("source-dir");
    let nested_dir = PathBuf::from(&source_dir).join("nested");
    create_dir_all(nested_dir.to_str().unwrap()).unwrap();
    let nested_file = nested_dir.join("child.txt");
    write_file_sync(nested_file.to_str().unwrap(), b"nested").unwrap();
    let copied_dir = directory.join("copied-dir");
    crate::fs::cp_sync(&source_dir, &copied_dir).unwrap();
    let copied_file = PathBuf::from(&copied_dir).join("nested/child.txt");
    assert_eq!(
        read_file_sync(copied_file.to_str().unwrap()).unwrap(),
        b"nested"
    );
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn open_descriptor_read_write_and_metadata_apis_work() {
    let directory = TestDirectory::new("file-descriptor");
    let path = directory.join("opened.bin");

    let mut write_options = crate::OpenOptions::new();
    write_options.write(true).create_new(true);
    let mut file = crate::fs::open_sync(&path, &write_options).unwrap();
    let write_buffers = [
        std::io::IoSlice::new(b"descriptor "),
        std::io::IoSlice::new(b"data"),
    ];
    assert_eq!(
        crate::fs::writev_sync(&mut file, &write_buffers).unwrap(),
        15
    );
    crate::fs::fdatasync_sync(&mut file).unwrap();
    assert_eq!(crate::fs::fstat_sync(&file).unwrap().len(), 15);
    crate::fs::ftruncate_sync(&mut file, 4).unwrap();
    crate::fs::fsync_sync(&mut file).unwrap();
    crate::fs::close_sync(file).unwrap();
    assert_eq!(read_file_sync(&path).unwrap(), b"desc");

    let mut read_options = crate::OpenOptions::new();
    read_options.read(true);
    let mut file = crate::fs::open_sync(&path, &read_options).unwrap();
    let mut first = [0; 2];
    let mut second = [0; 2];
    let mut read_buffers = [
        std::io::IoSliceMut::new(&mut first),
        std::io::IoSliceMut::new(&mut second),
    ];
    assert_eq!(
        crate::fs::readv_sync(&mut file, &mut read_buffers).unwrap(),
        4
    );
    assert_eq!(&first, b"de");
    assert_eq!(&second, b"sc");
    crate::fs::close_sync(file).unwrap();
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn asynchronous_filesystem_apis_complete_file_operations() {
    futures_lite::future::block_on(async {
        let directory = TestDirectory::new("async-file-apis");
        let source = directory.join("source.txt");
        let copy = directory.join("copy.txt");

        assert!(!exists(&source).await.unwrap());
        write_file(&source, b"hello").await.unwrap();
        append_file(&source, b" world").await.unwrap();
        access(&source, 0).await.unwrap();
        assert!(exists(&source).await.unwrap());
        copy_file(&source, &copy).await.unwrap();
        truncate(&copy, 5).await.unwrap();
        assert_eq!(read_file(&copy).await.unwrap(), b"hello");

        let source_dir = directory.join("source-dir");
        create_dir_all(&source_dir).unwrap();
        let nested_file = PathBuf::from(&source_dir).join("nested.txt");
        write_file_sync(nested_file.to_str().unwrap(), b"nested").unwrap();
        let copied_dir = directory.join("copied-dir");
        cp(&source_dir, &copied_dir).await.unwrap();
        let copied_file = PathBuf::from(copied_dir).join("nested.txt");
        assert_eq!(
            read_file(copied_file.to_str().unwrap()).await.unwrap(),
            b"nested"
        );

        let mut options = OpenOptions::new();
        options.read(true);
        let mut file = crate::fs::open(&copy, &options).await.unwrap();
        let mut bytes = [0; 5];
        assert_eq!(file.read(&mut bytes).await.unwrap(), bytes.len());
        assert_eq!(&bytes, b"hello");
        assert_eq!(file.metadata().await.unwrap().len(), 5);
        crate::fs::close(file).await.unwrap();

        let mut options = OpenOptions::new();
        options.write(true);
        let mut file = crate::fs::open(&copy, &options).await.unwrap();
        let buffers = [
            std::io::IoSlice::new(b"async "),
            std::io::IoSlice::new(b"write"),
        ];
        assert_eq!(file.write_vectored(&buffers).await.unwrap(), 11);
        file.sync_all().await.unwrap();
        assert_eq!(file.metadata().await.unwrap().len(), 11);
        file.set_len(5).await.unwrap();
        crate::fs::close(file).await.unwrap();
        assert_eq!(read_file(&copy).await.unwrap(), b"async");
    });
}

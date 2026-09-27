use js_sys::{Array, Function, Object, Reflect, Uint8Array};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

pub fn invalid_path_error(message: &str) -> JsValue {
    JsValue::from_str(message)
}

#[cfg(not(test))]
#[wasm_bindgen(module = "@zenfs/core")]
extern "C" {
    #[wasm_bindgen(thread_local_v2, js_name = fs)]
    static ZEN_FS: JsValue;
}

#[cfg(test)]
#[wasm_bindgen(module = "https://esm.sh/@zenfs/core@2.7.6?bundle&target=es2022")]
extern "C" {
    #[wasm_bindgen(thread_local_v2, js_name = fs)]
    static ZEN_FS: JsValue;
}

fn call_fs(method: &str, arguments: &[JsValue]) -> Result<JsValue, JsValue> {
    let fs = ZEN_FS.with(JsValue::clone);
    let method = Reflect::get(&fs, &JsValue::from_str(method))?;
    let method = method
        .dyn_into::<Function>()
        .map_err(|_| JsValue::from_str("ZenFS method is not a function"))?;
    let arguments = Array::from_iter(arguments.iter().cloned());

    // `Function::apply` captures exceptions thrown by ZenFS and returns them as
    // `Err`, which wasm-bindgen exposes as a normal JavaScript exception.
    method.apply(&fs, &arguments)
}

fn mkdir_options(recursive: bool) -> Result<JsValue, JsValue> {
    let options = Object::new();
    Reflect::set(
        &options,
        &JsValue::from_str("recursive"),
        &JsValue::from_bool(recursive),
    )?;
    Ok(options.into())
}

#[wasm_bindgen(js_name = appDir)]
pub fn app_dir(name: &str) -> Result<String, JsValue> {
    if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\\') {
        return Err(JsValue::from_str(
            "application name must be a single path component",
        ));
    }

    let path = format!("/app/{name}");
    let options = mkdir_options(true)?;
    call_fs("mkdirSync", &[JsValue::from_str(&path), options])?;
    Ok(path)
}

#[wasm_bindgen(js_name = exists)]
pub fn exists(path: &str) -> Result<bool, JsValue> {
    call_fs("existsSync", &[JsValue::from_str(path)])?
        .as_bool()
        .ok_or_else(|| JsValue::from_str("ZenFS existsSync did not return a boolean"))
}

#[wasm_bindgen(js_name = readText)]
pub fn read_text(path: &str) -> Result<String, JsValue> {
    call_fs(
        "readFileSync",
        &[JsValue::from_str(path), JsValue::from_str("utf8")],
    )?
    .as_string()
    .ok_or_else(|| JsValue::from_str("ZenFS did not return a UTF-8 string"))
}

#[wasm_bindgen(js_name = writeText)]
pub fn write_text(path: &str, contents: &str) -> Result<(), JsValue> {
    call_fs(
        "writeFileSync",
        &[JsValue::from_str(path), JsValue::from_str(contents)],
    )?;
    Ok(())
}

#[wasm_bindgen(js_name = appendText)]
pub fn append_text(path: &str, contents: &str) -> Result<(), JsValue> {
    call_fs(
        "appendFileSync",
        &[JsValue::from_str(path), JsValue::from_str(contents)],
    )?;
    Ok(())
}

#[wasm_bindgen(js_name = readFile)]
pub fn read_file(path: &str) -> Result<Vec<u8>, JsValue> {
    let contents = call_fs("readFileSync", &[JsValue::from_str(path)])?;
    Ok(Uint8Array::new(&contents).to_vec())
}

#[wasm_bindgen(js_name = writeFile)]
pub fn write_file(path: &str, contents: &[u8]) -> Result<(), JsValue> {
    let contents = Uint8Array::from(contents);
    call_fs("writeFileSync", &[JsValue::from_str(path), contents.into()])?;
    Ok(())
}

#[wasm_bindgen(js_name = createDir)]
pub fn create_dir(path: &str) -> Result<(), JsValue> {
    let options = mkdir_options(false)?;
    call_fs("mkdirSync", &[JsValue::from_str(path), options])?;
    Ok(())
}

#[wasm_bindgen(js_name = createDirAll)]
pub fn create_dir_all(path: &str) -> Result<(), JsValue> {
    let options = mkdir_options(true)?;
    call_fs("mkdirSync", &[JsValue::from_str(path), options])?;
    Ok(())
}

#[wasm_bindgen(js_name = readDir)]
pub fn read_dir(path: &str) -> Result<Vec<String>, JsValue> {
    let entries = call_fs("readdirSync", &[JsValue::from_str(path)])?;
    Array::from(&entries)
        .iter()
        .map(|entry| {
            entry
                .as_string()
                .ok_or_else(|| JsValue::from_str("ZenFS returned a non-string directory entry"))
        })
        .collect()
}

#[wasm_bindgen(js_name = removeFile)]
pub fn remove_file(path: &str) -> Result<(), JsValue> {
    call_fs("unlinkSync", &[JsValue::from_str(path)])?;
    Ok(())
}

#[wasm_bindgen(js_name = rename)]
pub fn rename(from: &str, to: &str) -> Result<(), JsValue> {
    call_fs(
        "renameSync",
        &[JsValue::from_str(from), JsValue::from_str(to)],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use wasm_bindgen_test::wasm_bindgen_test;

    fn test_path(name: &str) -> String {
        format!("/app/renfs-{name}-{:x}", js_sys::Math::random().to_bits())
    }

    #[wasm_bindgen_test]
    fn app_dir_creates_a_directory_and_rejects_invalid_names() {
        let name = format!("renfs-test-{:x}", js_sys::Math::random().to_bits());
        let path = app_dir(&name).unwrap();

        assert_eq!(path, format!("/app/{name}"));
        assert!(exists(&path).unwrap());

        for invalid in ["", ".", "..", "nested/name", r"nested\name"] {
            let error = app_dir(invalid).unwrap_err();
            assert_eq!(
                error.as_string().as_deref(),
                Some("application name must be a single path component")
            );
        }
    }

    #[wasm_bindgen_test]
    fn text_file_operations_round_trip_and_remove_files() {
        let directory = test_path("text");
        create_dir_all(&directory).unwrap();
        let path = format!("{directory}/message.txt");

        write_text(&path, "hello").unwrap();
        assert!(exists(&path).unwrap());
        assert_eq!(read_text(&path).unwrap(), "hello");

        append_text(&path, " world").unwrap();
        assert_eq!(read_text(&path).unwrap(), "hello world");

        remove_file(&path).unwrap();
        assert!(!exists(&path).unwrap());
    }

    #[wasm_bindgen_test]
    fn binary_file_operations_preserve_all_byte_values() {
        let directory = test_path("binary");
        create_dir_all(&directory).unwrap();
        let path = format!("{directory}/bytes.bin");
        let contents = [0, 1, 127, 128, 255];

        write_file(&path, &contents).unwrap();
        assert_eq!(read_file(&path).unwrap(), contents);
    }

    #[wasm_bindgen_test]
    fn rename_moves_files() {
        let directory = test_path("rename");
        create_dir_all(&directory).unwrap();
        let source = format!("{directory}/source.txt");
        let destination = format!("{directory}/destination.txt");

        write_text(&source, "renamed content").unwrap();
        rename(&source, &destination).unwrap();

        assert!(!exists(&source).unwrap());
        assert_eq!(read_text(&destination).unwrap(), "renamed content");
    }

    #[wasm_bindgen_test]
    fn directory_operations_create_and_list_entries() {
        let directory = test_path("directories");

        let missing_parent_child = format!("{directory}/missing/child");
        assert!(create_dir(&missing_parent_child).is_err());

        create_dir_all(&directory).unwrap();
        create_dir(&format!("{directory}/child")).unwrap();
        create_dir_all(&format!("{directory}/nested/grandchild")).unwrap();

        let entries = read_dir(&directory).unwrap();
        assert!(entries.contains(&"child".to_owned()));
        assert!(entries.contains(&"nested".to_owned()));
    }

    #[wasm_bindgen_test]
    fn javascript_exceptions_are_returned_as_errors() {
        let missing = format!("{}/missing.txt", test_path("missing"));
        let error = read_text(&missing).unwrap_err();
        let message = Reflect::get(&error, &JsValue::from_str("message")).unwrap();
        let message = message.as_string().unwrap();

        assert!(message.starts_with("ENOENT:"));
        assert!(message.contains(&missing));
    }
}

use js_sys::{Array, Function, Object, Reflect, Uint8Array};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;

pub fn invalid_path_error(message: &str) -> JsValue {
    JsValue::from_str(message)
}

#[wasm_bindgen(module = "@zenfs/core")]
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

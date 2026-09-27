use js_sys::{Array, Function, Object, Reflect, Uint8Array};
use std::io::{IoSlice, IoSliceMut};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

use super::api::{Metadata, OpenOptions};

pub type File = u32;

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

macro_rules! zenfs_setup_imports {
    ($module:literal) => {
        #[wasm_bindgen(module = $module)]
        extern "C" {
            #[wasm_bindgen(catch, js_name = configure)]
            fn zenfs_configure(configuration: &JsValue) -> Result<js_sys::Promise, JsValue>;
            #[wasm_bindgen(catch, js_name = configureSync)]
            fn zenfs_configure_sync(configuration: &JsValue) -> Result<(), JsValue>;
            #[wasm_bindgen(catch, js_name = configureSingle)]
            fn zenfs_configure_single(configuration: &JsValue) -> Result<js_sys::Promise, JsValue>;
            #[wasm_bindgen(catch, js_name = configureSingleSync)]
            fn zenfs_configure_single_sync(configuration: &JsValue) -> Result<(), JsValue>;
            #[wasm_bindgen(catch, js_name = configureFileSystem)]
            fn zenfs_configure_file_system(
                filesystem: &JsValue,
                configuration: &JsValue,
            ) -> Result<(), JsValue>;
            #[wasm_bindgen(catch, js_name = resolveMountConfig)]
            fn zenfs_resolve_mount_config(
                configuration: &JsValue,
            ) -> Result<js_sys::Promise, JsValue>;
            #[wasm_bindgen(catch, js_name = resolveMountConfigSync)]
            fn zenfs_resolve_mount_config_sync(configuration: &JsValue)
            -> Result<JsValue, JsValue>;
            #[wasm_bindgen(catch, js_name = resolveRemoteMount)]
            fn zenfs_resolve_remote_mount(
                channel: &JsValue,
                configuration: &JsValue,
            ) -> Result<js_sys::Promise, JsValue>;
            #[wasm_bindgen(catch, js_name = mount)]
            fn zenfs_mount(mount_point: &str, filesystem: &JsValue) -> Result<(), JsValue>;
            #[wasm_bindgen(catch, js_name = umount)]
            fn zenfs_umount(mount_point: &str) -> Result<(), JsValue>;
            #[wasm_bindgen(catch, js_name = sync)]
            fn zenfs_sync() -> Result<js_sys::Promise, JsValue>;
            #[wasm_bindgen(catch, js_name = waitOnline)]
            fn zenfs_wait_online(worker: &JsValue) -> Result<js_sys::Promise, JsValue>;
            #[wasm_bindgen(catch, js_name = attachFS)]
            fn zenfs_attach_fs(channel: &JsValue, filesystem: &JsValue) -> Result<(), JsValue>;
            #[wasm_bindgen(catch, js_name = detachFS)]
            fn zenfs_detach_fs(channel: &JsValue, filesystem: &JsValue) -> Result<(), JsValue>;

            #[wasm_bindgen(thread_local_v2, js_name = mounts)]
            static ZEN_MOUNTS: JsValue;
            #[wasm_bindgen(thread_local_v2, js_name = promises)]
            static ZEN_PROMISES: JsValue;
            #[wasm_bindgen(thread_local_v2, js_name = default)]
            static ZEN_DEFAULT: JsValue;
            #[wasm_bindgen(thread_local_v2, js_name = vfs)]
            static ZEN_VFS: JsValue;
            #[wasm_bindgen(thread_local_v2, js_name = version)]
            static ZEN_VERSION: JsValue;
            #[wasm_bindgen(thread_local_v2, js_name = constants)]
            static ZEN_CONSTANTS: JsValue;
        }
    };
}

#[cfg(not(test))]
zenfs_setup_imports!("@zenfs/core");

#[cfg(test)]
zenfs_setup_imports!("https://esm.sh/@zenfs/core@2.7.6?bundle&target=es2022");

async fn await_zenfs_promise(promise: js_sys::Promise) -> Result<JsValue, JsValue> {
    JsFuture::from(promise).await
}

#[wasm_bindgen]
pub async fn configure(configuration: &JsValue) -> Result<(), JsValue> {
    await_zenfs_promise(zenfs_configure(configuration)?).await?;
    Ok(())
}

#[wasm_bindgen(js_name = configureSync)]
pub fn configure_sync(configuration: &JsValue) -> Result<(), JsValue> {
    zenfs_configure_sync(configuration)
}

#[wasm_bindgen(js_name = configureSingle)]
pub async fn configure_single(configuration: &JsValue) -> Result<(), JsValue> {
    await_zenfs_promise(zenfs_configure_single(configuration)?).await?;
    Ok(())
}

#[wasm_bindgen(js_name = configureSingleSync)]
pub fn configure_single_sync(configuration: &JsValue) -> Result<(), JsValue> {
    zenfs_configure_single_sync(configuration)
}

#[wasm_bindgen(js_name = configureFileSystem)]
pub fn configure_file_system(filesystem: &JsValue, configuration: &JsValue) -> Result<(), JsValue> {
    zenfs_configure_file_system(filesystem, configuration)
}

#[wasm_bindgen(js_name = resolveMountConfig)]
pub async fn resolve_mount_config(configuration: &JsValue) -> Result<JsValue, JsValue> {
    await_zenfs_promise(zenfs_resolve_mount_config(configuration)?).await
}

#[wasm_bindgen(js_name = resolveMountConfigSync)]
pub fn resolve_mount_config_sync(configuration: &JsValue) -> Result<JsValue, JsValue> {
    zenfs_resolve_mount_config_sync(configuration)
}

#[wasm_bindgen(js_name = resolveRemoteMount)]
pub async fn resolve_remote_mount(
    channel: &JsValue,
    configuration: &JsValue,
) -> Result<JsValue, JsValue> {
    await_zenfs_promise(zenfs_resolve_remote_mount(channel, configuration)?).await
}

#[wasm_bindgen]
pub fn mount(mount_point: &str, filesystem: &JsValue) -> Result<(), JsValue> {
    zenfs_mount(mount_point, filesystem)
}

#[wasm_bindgen]
pub fn umount(mount_point: &str) -> Result<(), JsValue> {
    zenfs_umount(mount_point)
}

#[wasm_bindgen]
pub fn mounts() -> JsValue {
    ZEN_MOUNTS.with(JsValue::clone)
}

#[wasm_bindgen]
pub async fn sync() -> Result<(), JsValue> {
    await_zenfs_promise(zenfs_sync()?).await?;
    Ok(())
}

#[wasm_bindgen(js_name = waitOnline)]
pub async fn wait_online(worker: &JsValue) -> Result<(), JsValue> {
    await_zenfs_promise(zenfs_wait_online(worker)?).await?;
    Ok(())
}

#[wasm_bindgen(js_name = attachFS)]
pub fn attach_fs(channel: &JsValue, filesystem: &JsValue) -> Result<(), JsValue> {
    zenfs_attach_fs(channel, filesystem)
}

#[wasm_bindgen(js_name = detachFS)]
pub fn detach_fs(channel: &JsValue, filesystem: &JsValue) -> Result<(), JsValue> {
    zenfs_detach_fs(channel, filesystem)
}

#[wasm_bindgen(js_name = fs)]
pub fn zenfs_fs() -> JsValue {
    ZEN_FS.with(JsValue::clone)
}

#[wasm_bindgen]
pub fn promises() -> JsValue {
    ZEN_PROMISES.with(JsValue::clone)
}

#[wasm_bindgen(js_name = default)]
pub fn default_fs() -> JsValue {
    ZEN_DEFAULT.with(JsValue::clone)
}

#[wasm_bindgen]
pub fn vfs() -> JsValue {
    ZEN_VFS.with(JsValue::clone)
}

#[wasm_bindgen]
pub fn version() -> JsValue {
    ZEN_VERSION.with(JsValue::clone)
}

#[wasm_bindgen]
pub fn constants() -> JsValue {
    ZEN_CONSTANTS.with(JsValue::clone)
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

async fn call_fs_promise(method: &str, arguments: &[JsValue]) -> Result<JsValue, JsValue> {
    let fs = ZEN_FS.with(JsValue::clone);
    let promises = Reflect::get(&fs, &JsValue::from_str("promises"))?;
    let method = Reflect::get(&promises, &JsValue::from_str(method))?
        .dyn_into::<Function>()
        .map_err(|_| JsValue::from_str("ZenFS promise method is not a function"))?;
    let arguments = Array::from_iter(arguments.iter().cloned());
    let promise = method
        .apply(&promises, &arguments)?
        .dyn_into::<js_sys::Promise>()?;
    JsFuture::from(promise).await
}

async fn call_fs_callback(method_name: &str, arguments: &[JsValue]) -> Result<JsValue, JsValue> {
    let fs = ZEN_FS.with(JsValue::clone);
    let method = Reflect::get(&fs, &JsValue::from_str(method_name))?
        .dyn_into::<Function>()
        .map_err(|_| JsValue::from_str("ZenFS callback method is not a function"))?;
    let arguments = Array::from_iter(arguments.iter().cloned());
    let promise = js_sys::Promise::new(&mut |resolve, reject| {
        let callback_resolve = resolve.clone();
        let callback_reject = reject.clone();
        let callback = Closure::once_into_js(move |error: JsValue, value: JsValue| {
            if error.is_null() || error.is_undefined() {
                let _ = callback_resolve.call1(&JsValue::UNDEFINED, &value);
            } else {
                let _ = callback_reject.call1(&JsValue::UNDEFINED, &error);
            }
        });
        arguments.push(&callback);
        if let Err(error) = method.apply(&fs, &arguments) {
            let _ = reject.call1(&JsValue::UNDEFINED, &error);
        }
    });
    JsFuture::from(promise).await
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

fn descriptor_value(fd: &File) -> JsValue {
    JsValue::from_f64(f64::from(*fd))
}

fn returned_usize(value: JsValue, operation: &str) -> Result<usize, JsValue> {
    value
        .as_f64()
        .filter(|value| value.is_finite() && *value >= 0.0)
        .map(|value| value as usize)
        .ok_or_else(|| JsValue::from_str(&format!("ZenFS {operation} did not return a byte count")))
}

fn returned_descriptor(value: JsValue) -> Result<File, JsValue> {
    value
        .as_f64()
        .filter(|fd| fd.is_finite() && *fd >= 0.0 && *fd <= f64::from(u32::MAX))
        .map(|fd| fd as u32)
        .ok_or_else(|| JsValue::from_str("ZenFS open did not return a file descriptor"))
}

fn open_flags(options: &OpenOptions) -> Result<&'static str, JsValue> {
    if !options.read && !options.write && !options.append {
        return Err(JsValue::from_str(
            "at least one file access mode is required",
        ));
    }
    if options.truncate && !options.write && !options.append && !options.create_new {
        return Err(JsValue::from_str("truncate requires write access"));
    }
    if options.create && !options.write && !options.append {
        return Err(JsValue::from_str("create requires write or append access"));
    }
    if options.create_new && !options.write && !options.append {
        return Err(JsValue::from_str(
            "create_new requires write or append access",
        ));
    }

    Ok(if options.append {
        match (options.read, options.create_new) {
            (true, true) => "ax+",
            (false, true) => "ax",
            (true, false) => "a+",
            (false, false) => "a",
        }
    } else if options.create_new {
        if options.read { "wx+" } else { "wx" }
    } else if options.truncate || options.create {
        if options.read { "w+" } else { "w" }
    } else if options.write {
        "r+"
    } else {
        "r"
    })
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
pub async fn exists(path: &str) -> Result<bool, JsValue> {
    call_fs_promise("exists", &[JsValue::from_str(path)])
        .await?
        .as_bool()
        .ok_or_else(|| JsValue::from_str("ZenFS exists did not return a boolean"))
}

pub fn exists_sync(path: &str) -> Result<bool, JsValue> {
    call_fs("existsSync", &[JsValue::from_str(path)])?
        .as_bool()
        .ok_or_else(|| JsValue::from_str("ZenFS existsSync did not return a boolean"))
}

pub async fn access(path: &str, mode: u32) -> Result<(), JsValue> {
    call_fs_promise(
        "access",
        &[JsValue::from_str(path), JsValue::from_f64(f64::from(mode))],
    )
    .await?;
    Ok(())
}

pub fn access_sync(path: &str, mode: u32) -> Result<(), JsValue> {
    call_fs(
        "accessSync",
        &[JsValue::from_str(path), JsValue::from_f64(f64::from(mode))],
    )?;
    Ok(())
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

pub async fn append_file(path: &str, contents: &[u8]) -> Result<(), JsValue> {
    let contents = Uint8Array::from(contents);
    call_fs_promise("appendFile", &[JsValue::from_str(path), contents.into()]).await?;
    Ok(())
}

pub fn append_file_sync(path: &str, contents: &[u8]) -> Result<(), JsValue> {
    let contents = Uint8Array::from(contents);
    call_fs(
        "appendFileSync",
        &[JsValue::from_str(path), contents.into()],
    )?;
    Ok(())
}

#[wasm_bindgen(js_name = readFile)]
pub async fn read_file(path: &str) -> Result<Vec<u8>, JsValue> {
    let contents = call_fs_promise("readFile", &[JsValue::from_str(path)]).await?;
    Ok(Uint8Array::new(&contents).to_vec())
}

pub fn read_file_sync(path: &str) -> Result<Vec<u8>, JsValue> {
    let contents = call_fs("readFileSync", &[JsValue::from_str(path)])?;
    Ok(Uint8Array::new(&contents).to_vec())
}

#[wasm_bindgen(js_name = writeFile)]
pub async fn write_file(path: &str, contents: &[u8]) -> Result<(), JsValue> {
    let contents = Uint8Array::from(contents);
    call_fs_promise("writeFile", &[JsValue::from_str(path), contents.into()]).await?;
    Ok(())
}

pub fn write_file_sync(path: &str, contents: &[u8]) -> Result<(), JsValue> {
    let contents = Uint8Array::from(contents);
    call_fs("writeFileSync", &[JsValue::from_str(path), contents.into()])?;
    Ok(())
}

#[wasm_bindgen(js_name = copyFile)]
pub async fn copy_file(from: &str, to: &str) -> Result<(), JsValue> {
    call_fs_promise(
        "copyFile",
        &[JsValue::from_str(from), JsValue::from_str(to)],
    )
    .await?;
    Ok(())
}

pub fn copy_file_sync(from: &str, to: &str) -> Result<(), JsValue> {
    call_fs(
        "copyFileSync",
        &[JsValue::from_str(from), JsValue::from_str(to)],
    )?;
    Ok(())
}

pub async fn cp(from: &str, to: &str) -> Result<(), JsValue> {
    let options = Object::new();
    Reflect::set(&options, &JsValue::from_str("recursive"), &JsValue::TRUE)?;
    call_fs_promise(
        "cp",
        &[
            JsValue::from_str(from),
            JsValue::from_str(to),
            options.into(),
        ],
    )
    .await?;
    Ok(())
}

pub fn cp_sync(from: &str, to: &str) -> Result<(), JsValue> {
    let options = Object::new();
    Reflect::set(&options, &JsValue::from_str("recursive"), &JsValue::TRUE)?;
    call_fs(
        "cpSync",
        &[
            JsValue::from_str(from),
            JsValue::from_str(to),
            options.into(),
        ],
    )?;
    Ok(())
}

pub fn open_sync(path: &str, options: &OpenOptions) -> Result<File, JsValue> {
    let flags = open_flags(options)?;
    if options.append && !options.create && !options.create_new {
        let exists = call_fs("existsSync", &[JsValue::from_str(path)])?
            .as_bool()
            .ok_or_else(|| JsValue::from_str("ZenFS existsSync did not return a boolean"))?;
        if !exists {
            let descriptor = call_fs(
                "openSync",
                &[JsValue::from_str(path), JsValue::from_str("r")],
            )?;
            call_fs("closeSync", &[descriptor])?;
        }
    }
    let descriptor =
        if options.create && !options.create_new && !options.truncate && !options.append {
            match call_fs(
                "openSync",
                &[JsValue::from_str(path), JsValue::from_str("r+")],
            ) {
                Ok(descriptor) => descriptor,
                Err(error) if is_no_entry(&error) => call_fs(
                    "openSync",
                    &[JsValue::from_str(path), JsValue::from_str("w+")],
                )?,
                Err(error) => return Err(error),
            }
        } else if options.truncate && !options.create && !options.create_new && !options.append {
            let descriptor = call_fs(
                "openSync",
                &[JsValue::from_str(path), JsValue::from_str("r+")],
            )?;
            let descriptor = descriptor
                .as_f64()
                .filter(|fd| fd.is_finite() && *fd >= 0.0 && *fd <= f64::from(u32::MAX))
                .ok_or_else(|| {
                    JsValue::from_str("ZenFS openSync did not return a file descriptor")
                })? as u32;
            if let Err(error) = call_fs(
                "ftruncateSync",
                &[
                    JsValue::from_f64(f64::from(descriptor)),
                    JsValue::from_f64(0.0),
                ],
            ) {
                let _ = call_fs("closeSync", &[JsValue::from_f64(f64::from(descriptor))]);
                return Err(error);
            }
            return Ok(descriptor);
        } else {
            call_fs(
                "openSync",
                &[JsValue::from_str(path), JsValue::from_str(flags)],
            )?
        };
    let descriptor = descriptor
        .as_f64()
        .filter(|fd| fd.is_finite() && *fd >= 0.0 && *fd <= f64::from(u32::MAX))
        .ok_or_else(|| JsValue::from_str("ZenFS openSync did not return a file descriptor"))?;
    let descriptor = descriptor as u32;
    if options.truncate
        && options.append
        && let Err(error) = call_fs(
            "ftruncateSync",
            &[
                JsValue::from_f64(f64::from(descriptor)),
                JsValue::from_f64(0.0),
            ],
        )
    {
        let _ = call_fs("closeSync", &[JsValue::from_f64(f64::from(descriptor))]);
        return Err(error);
    }
    Ok(descriptor)
}

pub async fn open(path: &str, options: &OpenOptions) -> Result<File, JsValue> {
    let flags = open_flags(options)?;
    if options.append && !options.create && !options.create_new {
        let exists = call_fs_promise("exists", &[JsValue::from_str(path)])
            .await?
            .as_bool()
            .ok_or_else(|| JsValue::from_str("ZenFS exists did not return a boolean"))?;
        if !exists {
            let descriptor =
                call_fs_callback("open", &[JsValue::from_str(path), JsValue::from_str("r")])
                    .await?;
            call_fs_callback("close", &[descriptor]).await?;
        }
    }

    let descriptor = if options.create
        && !options.create_new
        && !options.truncate
        && !options.append
    {
        match call_fs_callback("open", &[JsValue::from_str(path), JsValue::from_str("r+")]).await {
            Ok(descriptor) => descriptor,
            Err(error) if is_no_entry(&error) => {
                call_fs_callback("open", &[JsValue::from_str(path), JsValue::from_str("w+")])
                    .await?
            }
            Err(error) => return Err(error),
        }
    } else if options.truncate && !options.create && !options.create_new && !options.append {
        let descriptor =
            call_fs_callback("open", &[JsValue::from_str(path), JsValue::from_str("r+")]).await?;
        if let Err(error) =
            call_fs_callback("ftruncate", &[descriptor.clone(), JsValue::from_f64(0.0)]).await
        {
            let _ = call_fs_callback("close", &[descriptor]).await;
            return Err(error);
        }
        descriptor
    } else {
        call_fs_callback("open", &[JsValue::from_str(path), JsValue::from_str(flags)]).await?
    };

    let descriptor = returned_descriptor(descriptor)?;
    if options.truncate
        && options.append
        && let Err(error) = call_fs_callback(
            "ftruncate",
            &[
                JsValue::from_f64(f64::from(descriptor)),
                JsValue::from_f64(0.0),
            ],
        )
        .await
    {
        let _ = call_fs_callback("close", &[JsValue::from_f64(f64::from(descriptor))]).await;
        return Err(error);
    }
    Ok(descriptor)
}

fn is_no_entry(error: &JsValue) -> bool {
    Reflect::get(error, &JsValue::from_str("code"))
        .ok()
        .and_then(|code| code.as_string())
        .as_deref()
        == Some("ENOENT")
}

pub async fn open_as_blob(path: &str) -> Result<Vec<u8>, JsValue> {
    let result = call_fs("openAsBlob", &[JsValue::from_str(path)])?;
    let promise = result
        .dyn_into::<js_sys::Promise>()
        .map_err(|_| JsValue::from_str("ZenFS openAsBlob did not return a promise"))?;
    let blob = JsFuture::from(promise).await?;
    let array_buffer = Reflect::get(&blob, &JsValue::from_str("arrayBuffer"))?
        .dyn_into::<Function>()
        .map_err(|_| JsValue::from_str("ZenFS openAsBlob did not return a Blob"))?;
    let array_buffer = array_buffer.call0(&blob)?.dyn_into::<js_sys::Promise>()?;
    let buffer = JsFuture::from(array_buffer).await?;
    Ok(Uint8Array::new(&buffer).to_vec())
}

pub fn close_sync(file: File) -> Result<(), JsValue> {
    call_fs("closeSync", &[descriptor_value(&file)])?;
    Ok(())
}

pub async fn close(file: &File) -> Result<(), JsValue> {
    call_fs_callback("close", &[descriptor_value(file)]).await?;
    Ok(())
}

pub fn read_sync(file: &mut File, buffer: &mut [u8]) -> Result<usize, JsValue> {
    let target = Uint8Array::new_with_length(buffer.len() as u32);
    let count = call_fs(
        "readSync",
        &[
            descriptor_value(file),
            target.clone().into(),
            JsValue::from_f64(0.0),
            JsValue::from_f64(buffer.len() as f64),
            JsValue::NULL,
        ],
    )?;
    let count = returned_usize(count, "readSync")?;
    let count = count.min(buffer.len());
    target
        .subarray(0, count as u32)
        .copy_to(&mut buffer[..count]);
    Ok(count)
}

pub async fn read(file: &mut File, buffer: &mut [u8]) -> Result<usize, JsValue> {
    let target = Uint8Array::new_with_length(buffer.len() as u32);
    let count = call_fs_callback(
        "read",
        &[
            descriptor_value(file),
            target.clone().into(),
            JsValue::from_f64(0.0),
            JsValue::from_f64(buffer.len() as f64),
            JsValue::NULL,
        ],
    )
    .await?;
    let count = returned_usize(count, "read")?.min(buffer.len());
    target
        .subarray(0, count as u32)
        .copy_to(&mut buffer[..count]);
    Ok(count)
}

pub fn write_sync(file: &mut File, buffer: &[u8]) -> Result<usize, JsValue> {
    let contents = Uint8Array::from(buffer);
    let count = call_fs(
        "writeSync",
        &[
            descriptor_value(file),
            contents.into(),
            JsValue::from_f64(0.0),
            JsValue::from_f64(buffer.len() as f64),
            JsValue::NULL,
        ],
    )?;
    returned_usize(count, "writeSync")
}

pub async fn write(file: &mut File, buffer: &[u8]) -> Result<usize, JsValue> {
    let contents = Uint8Array::from(buffer);
    let count = call_fs_callback(
        "write",
        &[
            descriptor_value(file),
            contents.into(),
            JsValue::from_f64(0.0),
            JsValue::from_f64(buffer.len() as f64),
            JsValue::NULL,
        ],
    )
    .await?;
    returned_usize(count, "write")
}

pub fn readv_sync(file: &mut File, buffers: &mut [IoSliceMut<'_>]) -> Result<usize, JsValue> {
    let arrays = buffers
        .iter()
        .map(|buffer| Uint8Array::from(&buffer[..]))
        .collect::<Vec<_>>();
    let js_buffers = Array::new();
    for buffer in &arrays {
        js_buffers.push(buffer);
    }
    let count = call_fs(
        "readvSync",
        &[descriptor_value(file), js_buffers.into(), JsValue::NULL],
    )?;
    let count = returned_usize(count, "readvSync")?;
    let mut remaining = count;
    for (target, source) in buffers.iter_mut().zip(arrays.iter()) {
        let copied = remaining.min(target.len());
        source
            .subarray(0, copied as u32)
            .copy_to(&mut target[..copied]);
        remaining -= copied;
        if remaining == 0 {
            break;
        }
    }
    Ok(count)
}

pub async fn readv(file: &mut File, buffers: &mut [IoSliceMut<'_>]) -> Result<usize, JsValue> {
    let arrays = buffers
        .iter()
        .map(|buffer| Uint8Array::from(&buffer[..]))
        .collect::<Vec<_>>();
    let js_buffers = Array::new();
    for buffer in &arrays {
        js_buffers.push(buffer);
    }
    let count = call_fs_callback(
        "readv",
        &[descriptor_value(file), js_buffers.into(), JsValue::NULL],
    )
    .await?;
    let count = returned_usize(count, "readv")?;
    let mut remaining = count;
    for (target, source) in buffers.iter_mut().zip(arrays.iter()) {
        let copied = remaining.min(target.len());
        source
            .subarray(0, copied as u32)
            .copy_to(&mut target[..copied]);
        remaining -= copied;
        if remaining == 0 {
            break;
        }
    }
    Ok(count)
}

pub fn writev_sync(file: &mut File, buffers: &[IoSlice<'_>]) -> Result<usize, JsValue> {
    let js_buffers = Array::new();
    for buffer in buffers {
        js_buffers.push(&Uint8Array::from(&buffer[..]));
    }
    let count = call_fs(
        "writevSync",
        &[descriptor_value(file), js_buffers.into(), JsValue::NULL],
    )?;
    returned_usize(count, "writevSync")
}

pub async fn writev(file: &mut File, buffers: &[IoSlice<'_>]) -> Result<usize, JsValue> {
    let js_buffers = Array::new();
    for buffer in buffers {
        js_buffers.push(&Uint8Array::from(&buffer[..]));
    }
    let count = call_fs_callback(
        "writev",
        &[descriptor_value(file), js_buffers.into(), JsValue::NULL],
    )
    .await?;
    returned_usize(count, "writev")
}

fn stats_bool(stats: &JsValue, name: &str) -> Result<bool, JsValue> {
    let member = Reflect::get(stats, &JsValue::from_str(name))?;
    if let Ok(method) = member.clone().dyn_into::<Function>() {
        method
            .call0(stats)?
            .as_bool()
            .ok_or_else(|| JsValue::from_str("ZenFS stat method did not return a boolean"))
    } else {
        member
            .as_bool()
            .ok_or_else(|| JsValue::from_str("ZenFS stat property was not a boolean"))
    }
}

fn metadata_from_stats(stats: &JsValue) -> Result<Metadata, JsValue> {
    let len = Reflect::get(stats, &JsValue::from_str("size"))?
        .as_f64()
        .filter(|size| size.is_finite() && *size >= 0.0)
        .map(|size| size as u64)
        .ok_or_else(|| JsValue::from_str("ZenFS stat size was not a non-negative number"))?;
    Ok(Metadata::from_parts(
        len,
        stats_bool(stats, "isFile")?,
        stats_bool(stats, "isDirectory")?,
        stats_bool(stats, "isSymbolicLink")?,
    ))
}

pub fn fstat_sync(file: &File) -> Result<Metadata, JsValue> {
    let stats = call_fs("fstatSync", &[descriptor_value(file)])?;
    metadata_from_stats(&stats)
}

pub async fn fstat(file: &File) -> Result<Metadata, JsValue> {
    let stats = call_fs_callback("fstat", &[descriptor_value(file)]).await?;
    metadata_from_stats(&stats)
}

pub fn fsync_sync(file: &File) -> Result<(), JsValue> {
    call_fs("fsyncSync", &[descriptor_value(file)])?;
    Ok(())
}

pub async fn fsync(file: &File) -> Result<(), JsValue> {
    call_fs_callback("fsync", &[descriptor_value(file)]).await?;
    Ok(())
}

pub fn fdatasync_sync(file: &File) -> Result<(), JsValue> {
    call_fs("fdatasyncSync", &[descriptor_value(file)])?;
    Ok(())
}

pub async fn fdatasync(file: &File) -> Result<(), JsValue> {
    call_fs_callback("fdatasync", &[descriptor_value(file)]).await?;
    Ok(())
}

pub fn ftruncate_sync(file: &File, size: u64) -> Result<(), JsValue> {
    call_fs(
        "ftruncateSync",
        &[descriptor_value(file), JsValue::from_f64(size as f64)],
    )?;
    Ok(())
}

pub async fn ftruncate(file: &File, size: u64) -> Result<(), JsValue> {
    call_fs_callback(
        "ftruncate",
        &[descriptor_value(file), JsValue::from_f64(size as f64)],
    )
    .await?;
    Ok(())
}

pub fn truncate_sync(path: &str, len: u64) -> Result<(), JsValue> {
    call_fs(
        "truncateSync",
        &[JsValue::from_str(path), JsValue::from_f64(len as f64)],
    )?;
    Ok(())
}

pub async fn truncate(path: &str, len: u64) -> Result<(), JsValue> {
    call_fs_promise(
        "truncate",
        &[JsValue::from_str(path), JsValue::from_f64(len as f64)],
    )
    .await?;
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
        assert!(exists_sync(&path).unwrap());

        for invalid in ["", ".", "..", "nested/name", r"nested\name"] {
            let error = app_dir(invalid).unwrap_err();
            assert_eq!(
                error.as_string().as_deref(),
                Some("application name must be a single path component")
            );
        }
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

    #[wasm_bindgen_test]
    async fn open_as_blob_reads_a_file_snapshot() {
        let path = test_path("blob");
        write_file_sync(&path, b"blob contents").unwrap();

        let blob = open_as_blob(&path).await.unwrap();

        assert_eq!(blob, b"blob contents");
    }

    #[wasm_bindgen_test]
    async fn promise_and_callback_filesystem_apis_are_async() {
        let path = test_path("async-apis");
        write_file(&path, b"async").await.unwrap();
        append_file(&path, b" file").await.unwrap();
        assert!(exists(&path).await.unwrap());
        assert_eq!(read_file(&path).await.unwrap(), b"async file");

        let mut options = OpenOptions::new();
        options.read(true);
        let mut file = open(&path, &options).await.unwrap();
        let mut contents = [0; 10];
        assert_eq!(
            read(&mut file, &mut contents).await.unwrap(),
            contents.len()
        );
        assert_eq!(&contents, b"async file");
        assert_eq!(fstat(&file).await.unwrap().len(), contents.len() as u64);
        close(&file).await.unwrap();
    }
}

#[cfg(test)]
#[path = "implementation_tests.rs"]
mod shared_tests;

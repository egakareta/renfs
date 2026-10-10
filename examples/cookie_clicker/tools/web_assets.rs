use serde_json::Value;
use std::{collections::BTreeSet, error::Error, fs, path::Path, process::Command};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

fn main() -> Result<()> {
    let project = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new("cargo")
        .current_dir(project)
        .args([
            "metadata",
            "--locked",
            "--format-version",
            "1",
            "--filter-platform",
            "wasm32-unknown-unknown",
        ])
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "cargo metadata failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    let metadata: Value = serde_json::from_slice(&output.stdout)?;
    let root = metadata["resolve"]["root"]
        .as_str()
        .ok_or("missing Cargo root package")?;
    let macroquad = dependency(&metadata, root, "macroquad")?;
    let macroquad_id = macroquad["id"]
        .as_str()
        .ok_or("missing Macroquad package ID")?;
    let miniquad = dependency(&metadata, macroquad_id, "miniquad")?;
    let manifest = Path::new(
        miniquad["manifest_path"]
            .as_str()
            .ok_or("missing Miniquad manifest")?,
    );
    let source = manifest
        .parent()
        .ok_or("missing Miniquad source directory")?;
    let loader = fs::read(source.join("js/gl.js"))?;
    let names = env_names(std::str::from_utf8(&loader)?)?;
    let version = miniquad["version"]
        .as_str()
        .ok_or("missing Miniquad version")?;
    let exports = names
        .into_iter()
        .map(|name| format!("  {name},\n"))
        .collect::<String>();
    let env = format!(
        "// Generated from Miniquad {version}; do not edit.\n\
         export const {{\n{exports}}} = globalThis.importObject.env;\n"
    );

    let generated = project.join("web/generated");
    fs::create_dir_all(&generated)?;
    write_if_changed(&generated.join("gl.js"), &loader)?;
    write_if_changed(&generated.join("env.js"), env.as_bytes())?;
    write_if_changed(
        &generated.join("LICENSE-miniquad-MIT"),
        &fs::read(source.join("LICENSE-MIT"))?,
    )?;
    Ok(())
}

// Follow the resolved dependency edges instead of guessing a Cargo cache path
// or picking the first package named "miniquad" (multiple versions may exist).
fn dependency<'a>(metadata: &'a Value, parent: &str, name: &str) -> Result<&'a Value> {
    let node = metadata["resolve"]["nodes"]
        .as_array()
        .ok_or("missing Cargo dependency graph")?
        .iter()
        .find(|node| node["id"] == parent)
        .ok_or_else(|| format!("missing Cargo node: {parent}"))?;
    let id = node["deps"]
        .as_array()
        .ok_or("missing Cargo dependency edges")?
        .iter()
        .find(|dependency| dependency["name"] == name)
        .and_then(|dependency| dependency["pkg"].as_str())
        .ok_or_else(|| format!("missing dependency: {name}"))?;
    metadata["packages"]
        .as_array()
        .ok_or("missing Cargo packages")?
        .iter()
        .find(|package| package["id"] == id)
        .ok_or_else(|| format!("missing Cargo package: {id}").into())
}

// Miniquad's loader declares the direct env properties at eight-space indent.
// Read both `name: function (...)` and shorthand properties such as `dpi_scale`;
// nested object keys and function bodies must not become module exports.
fn env_names(source: &str) -> Result<BTreeSet<&str>> {
    let object = source
        .split_once("var importObject = {")
        .and_then(|(_, object)| object.split_once("    env: {"))
        .and_then(|(_, env)| env.split_once("\n    }"))
        .map(|(env, _)| env)
        .ok_or("unsupported Miniquad importObject.env layout")?;
    let mut names = BTreeSet::new();
    for line in object.lines() {
        let Some(property) = line.strip_prefix("        ") else {
            continue;
        };
        let length = property
            .bytes()
            .take_while(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'$'))
            .count();
        if length == 0 || property.as_bytes()[0].is_ascii_digit() {
            continue;
        }
        let (name, tail) = property.split_at(length);
        if tail.starts_with(':') || tail.trim().trim_end_matches(',').is_empty() {
            names.insert(name);
        }
    }
    if !names.contains("init_webgl") || !names.contains("run_animation_loop") {
        return Err("could not extract Miniquad's graphics and event-loop imports".into());
    }
    Ok(names)
}

fn write_if_changed(path: &Path, contents: &[u8]) -> Result<()> {
    if fs::read(path).ok().as_deref() != Some(contents) {
        fs::write(path, contents)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_direct_functions_and_shorthand_but_not_nested_properties() {
        let source = r#"var importObject = {
    env: {
        init_webgl,
        run_animation_loop: function () {
            const object = {
                nested: true,
            };
        },
        glViewport: function (x, y, w, h) {},
        dpi_scale,
    }
};"#;
        assert_eq!(
            env_names(source).unwrap().into_iter().collect::<Vec<_>>(),
            [
                "dpi_scale",
                "glViewport",
                "init_webgl",
                "run_animation_loop"
            ]
        );
    }

    #[test]
    fn rejects_an_unknown_loader_layout() {
        assert!(env_names("const env = {};").is_err());
    }

    #[test]
    fn uses_the_resolved_dependency_not_another_installed_version() {
        let metadata = serde_json::json!({
            "resolve": { "nodes": [{
                "id": "macroquad",
                "deps": [{ "name": "miniquad", "pkg": "miniquad-new" }]
            }] },
            "packages": [
                { "id": "miniquad-old", "version": "old" },
                { "id": "miniquad-new", "version": "new" }
            ]
        });
        assert_eq!(
            dependency(&metadata, "macroquad", "miniquad").unwrap()["version"],
            "new"
        );
    }
}

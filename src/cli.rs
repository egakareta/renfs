use std::{
    collections::{HashMap, HashSet},
    fs,
    io::{Read, Write},
    path::PathBuf,
};

use anyhow::{Context, Result, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use clap::Parser;
use flate2::{Compression, read::GzDecoder, write::GzEncoder};
use regex::{Captures, Regex};
use tempfile::NamedTempFile;
use url::Url;

#[derive(Debug, Parser)]
#[command(
    name = "renfs",
    version,
    about = "Bundle ZenFS for browser applications"
)]
struct Cli {
    /// File path to write the combined ZenFS bundle into.
    #[arg(value_name = "OUTPUT_PATH")]
    output_path: PathBuf,

    /// Additional @zenfs modules to bundle (e.g. dom, archives).
    /// Use name@version to select a module version.
    #[arg(long = "module", value_name = "MODULE")]
    modules: Vec<String>,

    /// @zenfs/core version, dist-tag, or semver range to download.
    #[arg(long, default_value = "latest")]
    zenfs_version: String,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    bundle(cli)
}

fn bundle(args: Cli) -> Result<()> {
    ensure!(
        !args.zenfs_version.trim().is_empty(),
        "ZenFS version cannot be empty"
    );

    let output_path = &args.output_path;
    let path_str = output_path.to_string_lossy();
    ensure!(!path_str.trim().is_empty(), "output path cannot be empty");
    ensure!(
        !path_str.ends_with('/') && !path_str.ends_with('\\'),
        "output path '{}' appears to be a directory; please specify a file path",
        output_path.display()
    );
    ensure!(
        !output_path.is_dir(),
        "output path '{}' is a directory; please specify a file path",
        output_path.display()
    );

    let parent = output_path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| std::path::Path::new("."));

    fs::create_dir_all(parent)
        .with_context(|| format!("could not create output directory {}", parent.display()))?;
    let parent = fs::canonicalize(parent)
        .with_context(|| format!("could not resolve output directory {}", parent.display()))?;
    let file_name = output_path.file_name().context("invalid output path")?;
    let output_file = parent.join(file_name);
    let mut modules = args
        .modules
        .iter()
        .map(|module| normalize_module(module))
        .collect::<Result<Vec<_>>>()?;
    let mut requested_modules = HashSet::from(["@zenfs/core".to_owned()]);
    for module in &modules {
        ensure!(
            requested_modules.insert(module.package.clone()),
            "duplicate ZenFS module requested: {}",
            module.package
        );
    }
    let mut namespaces = HashSet::new();
    for module in modules.iter_mut() {
        let base = module.namespace.clone();
        let mut namespace = base.clone();
        let mut suffix = 2;
        while !namespaces.insert(namespace.clone()) {
            namespace = format!("{base}_{suffix}");
            suffix += 1;
        }
        module.namespace = namespace;
    }

    let mut targets = vec![BundleTarget {
        package: "@zenfs/core".to_owned(),
        version: args.zenfs_version.clone(),
        external_core: false,
        namespace: None,
    }];
    targets.extend(modules.into_iter().map(|module| BundleTarget {
        package: module.package,
        version: module.version,
        external_core: true,
        namespace: Some(module.namespace),
    }));

    let mut core_bundle = None;
    let mut module_bundles = Vec::new();
    for target in targets {
        println!("downloading {}@{}...", target.package, target.version);
        let entry_url = package_url(&target.package, &target.version, target.external_core)?;
        let bundle =
            download_single_file_bundle(&entry_url, target.external_core, &args.zenfs_version)?;
        if let Some(namespace) = target.namespace {
            module_bundles.push((namespace, bundle));
        } else {
            core_bundle = Some(bundle);
        }
    }

    let bundle = combine_bundles(
        core_bundle.context("the core bundle was not generated")?,
        module_bundles,
    )?;
    let mut staged_file = NamedTempFile::new_in(&parent)
        .with_context(|| format!("could not create temporary file in {}", parent.display()))?;
    staged_file
        .write_all(bundle.as_bytes())
        .context("could not write the generated ZenFS bundle")?;
    staged_file
        .persist(&output_file)
        .with_context(|| format!("could not write {}", output_file.display()))?;
    println!("written to {}", output_file.display());
    Ok(())
}

struct ZenFsModule {
    package: String,
    namespace: String,
    version: String,
}

struct BundleTarget {
    package: String,
    version: String,
    external_core: bool,
    namespace: Option<String>,
}

fn normalize_module(module: &str) -> Result<ZenFsModule> {
    let module = module
        .strip_prefix("@zenfs/")
        .or_else(|| module.strip_prefix("zenfs/"))
        .unwrap_or(module);
    let (suffix, version) = module
        .split_once('@')
        .map_or((module, "latest"), |(name, version)| (name, version));
    ensure!(
        !suffix.is_empty()
            && suffix
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
            && suffix.as_bytes()[0].is_ascii_alphanumeric(),
        "invalid ZenFS module {module:?}; use a module name such as dom or archives"
    );
    ensure!(
        !version.trim().is_empty(),
        "ZenFS module version cannot be empty"
    );
    Ok(ZenFsModule {
        package: format!("@zenfs/{suffix}"),
        namespace: format!(
            "__zenfs_{}",
            suffix
                .chars()
                .map(|character| if matches!(character, '-' | '.') {
                    '_'
                } else {
                    character
                })
                .collect::<String>()
        ),
        version: version.to_owned(),
    })
}

fn combine_bundles(core_bundle: String, module_bundles: Vec<(String, String)>) -> Result<String> {
    let core_names = collect_export_names(&core_bundle)?;
    let mut output = String::from(
        r#"/* Combined by renfs. */
async function __renfsLoad(payload, coreUrl) {
  const binary = atob(payload);
  const bytes = Uint8Array.from(binary, (character) => character.charCodeAt(0));
  const stream = new Blob([bytes]).stream().pipeThrough(new DecompressionStream("gzip"));
  const source = await new Response(stream).text();
  const url = await __renfsPrepare(source, coreUrl);
  return { url, namespace: await import(url) };
}
async function __renfsPrepare(source, coreUrl) {
  const dependencies = [...source.matchAll(/data:(text\/javascript|application\/javascript\+gzip);base64,([A-Za-z0-9+/=]+)/g)];
  for (const match of dependencies) {
    const specifier = match[0];
    let dependencyUrl = __renfsModuleCache.get(specifier);
    if (!dependencyUrl) {
      const binary = atob(match[2]);
      const bytes = Uint8Array.from(binary, (character) => character.charCodeAt(0));
      let dependencySource;
      if (match[1] === "application/javascript+gzip") {
        const stream = new Blob([bytes]).stream().pipeThrough(new DecompressionStream("gzip"));
        dependencySource = await new Response(stream).text();
      } else {
        dependencySource = new TextDecoder().decode(bytes);
      }
      dependencyUrl = await __renfsPrepare(dependencySource, coreUrl);
      __renfsModuleCache.set(specifier, dependencyUrl);
    }
    source = source.replaceAll(specifier, dependencyUrl);
  }
  if (coreUrl) {
    source = source.replace(
      /(\b(?:from|import)\b\s*(?:\(\s*)?)(["'])@zenfs\/core\2/g,
      (_match, prefix) => prefix + JSON.stringify(coreUrl),
    );
  }
  const url = URL.createObjectURL(new Blob([source], { type: "text/javascript" }));
  return url;
}
const __renfsModuleCache = new Map();
"#,
    );
    let core_payload = gzip_base64(&core_bundle)?;
    output.push_str(&format!(
        "const __renfs_core = await __renfsLoad(\"{core_payload}\");\n"
    ));

    let mut exported_names = HashSet::new();
    let mut export_id = 0;
    append_named_exports(
        &mut output,
        "__renfs_core",
        core_names,
        "__zenfs_core",
        true,
        &mut exported_names,
        &mut export_id,
    );

    for (module_index, (namespace, bundle)) in module_bundles.into_iter().enumerate() {
        let module_names = collect_export_names(&bundle)?;
        let module_payload = gzip_base64(&bundle)?;
        let module_var = format!("__renfs_module_{module_index}");
        output.push_str(&format!(
            "const {module_var} = await __renfsLoad(\"{module_payload}\", __renfs_core.url);\n"
        ));
        append_named_exports(
            &mut output,
            &module_var,
            module_names,
            &namespace,
            false,
            &mut exported_names,
            &mut export_id,
        );

        let mut namespace_export = namespace.clone();
        let mut alias_suffix = 2;
        while exported_names.contains(&namespace_export) {
            namespace_export = format!("{namespace}_{alias_suffix}");
            alias_suffix += 1;
        }
        exported_names.insert(namespace_export.clone());
        let binding = format!("__renfs_export_{export_id}");
        export_id += 1;
        output.push_str(&format!(
            "const {binding} = {module_var}.namespace;\nexport {{ {binding} as {namespace_export} }};\n"
        ));
    }
    Ok(output)
}

fn append_named_exports(
    output: &mut String,
    module_var: &str,
    names: HashSet<String>,
    namespace: &str,
    is_core: bool,
    exported_names: &mut HashSet<String>,
    export_id: &mut usize,
) {
    let mut names = names.into_iter().collect::<Vec<_>>();
    names.sort();
    let mut alias_suffix = 2;
    for name in names {
        let alias = if is_core || (name != "default" && exported_names.insert(name.clone())) {
            name.clone()
        } else {
            let mut alias = format!("{namespace}_{name}");
            while exported_names.contains(&alias) {
                alias = format!("{namespace}_{name}_{alias_suffix}");
                alias_suffix += 1;
            }
            exported_names.insert(alias.clone());
            alias
        };
        let binding = format!("__renfs_export_{}", *export_id);
        *export_id += 1;
        output.push_str(&format!(
            "const {binding} = {module_var}.namespace[{name:?}];\nexport {{ {binding} as {alias} }};\n"
        ));
        exported_names.insert(alias);
    }
}

fn gzip_base64(source: &str) -> Result<String> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::best());
    encoder
        .write_all(source.as_bytes())
        .context("could not compress a ZenFS module")?;
    Ok(STANDARD.encode(
        encoder
            .finish()
            .context("could not finish module compression")?,
    ))
}

fn collect_export_names(source: &str) -> Result<HashSet<String>> {
    let mut names = HashSet::new();
    let mut visited = HashSet::new();
    collect_export_names_inner(source, &mut names, &mut visited)?;
    Ok(names)
}

fn collect_export_names_inner(
    source: &str,
    names: &mut HashSet<String>,
    visited: &mut HashSet<String>,
) -> Result<()> {
    for captures in export_list_pattern().captures_iter(source) {
        for item in captures.name("list").unwrap().as_str().split(',') {
            let item = item.trim();
            if item.is_empty() {
                continue;
            }
            let exported = item
                .rsplit_once(" as ")
                .map_or(item, |(_, exported)| exported.trim());
            names.insert(exported.to_owned());
        }
    }

    for captures in export_all_pattern().captures_iter(source) {
        if let Some(namespace) = captures.name("name") {
            names.insert(namespace.as_str().to_owned());
            continue;
        }

        let specifier = captures.name("specifier").unwrap().as_str();
        let Some(module) = decode_embedded_module(specifier)? else {
            // External imports such as @zenfs/core are bundled separately.
            continue;
        };
        if !visited.insert(specifier.to_owned()) {
            continue;
        }
        let mut child_names = HashSet::new();
        collect_export_names_inner(&module, &mut child_names, visited)?;
        names.extend(child_names.into_iter().filter(|name| name != "default"));
    }

    for captures in export_declaration_pattern().captures_iter(source) {
        names.insert(captures.name("name").unwrap().as_str().to_owned());
    }
    if source.contains("export default") {
        names.insert("default".to_owned());
    }
    Ok(())
}

#[cfg(test)]
fn as_data_url(source: &str) -> String {
    format!(
        "data:text/javascript;base64,{}",
        STANDARD.encode(source.as_bytes())
    )
}

fn as_gzip_data_url(source: &str) -> Result<String> {
    Ok(format!(
        "data:application/javascript+gzip;base64,{}",
        gzip_base64(source)?
    ))
}

fn decode_embedded_module(specifier: &str) -> Result<Option<String>> {
    let (encoded, compressed) = if let Some(encoded) =
        specifier.strip_prefix("data:text/javascript;base64,")
    {
        (encoded, false)
    } else if let Some(encoded) = specifier.strip_prefix("data:application/javascript+gzip;base64,")
    {
        (encoded, true)
    } else {
        return Ok(None);
    };

    let bytes = STANDARD
        .decode(encoded)
        .with_context(|| format!("invalid embedded JavaScript module {specifier}"))?;
    let bytes = if compressed {
        let mut decoder = GzDecoder::new(bytes.as_slice());
        let mut decoded = Vec::new();
        decoder
            .read_to_end(&mut decoded)
            .with_context(|| format!("could not decompress embedded module {specifier}"))?;
        decoded
    } else {
        bytes
    };
    String::from_utf8(bytes)
        .map(Some)
        .with_context(|| format!("embedded JavaScript module is not UTF-8: {specifier}"))
}

fn package_url(package: &str, version: &str, external_core: bool) -> Result<Url> {
    let mut url = Url::parse(&format!(
        "https://esm.sh/{package}@{version}?bundle&target=es2022"
    ))?;
    if external_core {
        url.query_pairs_mut().append_pair("external", "@zenfs/core");
    }
    Ok(url)
}

fn download_single_file_bundle(
    entry_url: &Url,
    external_core: bool,
    core_version: &str,
) -> Result<String> {
    let mut inlined_modules = HashMap::new();
    let mut active_modules = HashSet::new();
    let source = download_module_source(entry_url)?;
    rewrite_imports(
        entry_url,
        &source,
        &mut inlined_modules,
        &mut active_modules,
        external_core,
        core_version,
    )
}

fn inline_module(
    url: &Url,
    inlined_modules: &mut HashMap<String, String>,
    active_modules: &mut HashSet<String>,
    external_core: bool,
    core_version: &str,
) -> Result<String> {
    let key = url.as_str().to_owned();
    if let Some(module) = inlined_modules.get(&key) {
        return Ok(module.clone());
    }
    ensure!(
        active_modules.insert(key.clone()),
        "esm.sh returned a cyclic module graph; cannot inline {url} into one file"
    );

    let source = download_module_source(url)?;
    let result = rewrite_imports(
        url,
        &source,
        inlined_modules,
        active_modules,
        external_core,
        core_version,
    );
    active_modules.remove(&key);
    let inlined = result?;
    let data_url = as_gzip_data_url(&inlined)?;
    inlined_modules.insert(key, data_url.clone());
    Ok(data_url)
}

fn download_module_source(url: &Url) -> Result<String> {
    ensure!(
        url.scheme() == "https" && url.host_str() == Some("esm.sh"),
        "refusing to download unexpected module URL {url}"
    );
    let mut response = ureq::get(url.as_str())
        .header("User-Agent", concat!("renfs/", env!("CARGO_PKG_VERSION")))
        .header("Accept", "application/javascript")
        .call()
        .with_context(|| format!("failed to download JavaScript module {url}"))?;
    response
        .body_mut()
        .read_to_string()
        .with_context(|| format!("failed to read JavaScript module {url}"))
}

fn rewrite_imports(
    url: &Url,
    source: &str,
    inlined_modules: &mut HashMap<String, String>,
    active_modules: &mut HashSet<String>,
    external_core: bool,
    core_version: &str,
) -> Result<String> {
    let replacements = import_pattern()
        .captures_iter(source)
        .map(|captures| {
            let specifier = captures.name("specifier").unwrap().as_str();
            if external_core && is_zenfs_core_root_import(url, specifier) {
                return Ok((specifier.to_owned(), "@zenfs/core".to_owned()));
            }
            if external_core && is_zenfs_core_internal_import(url, specifier) {
                return Ok((specifier.to_owned(), "@zenfs/core".to_owned()));
            }
            let dependency = if external_core && specifier.starts_with("@zenfs/core/") {
                zenfs_core_subpath_url(specifier, core_version)?
            } else {
                url.join(specifier)
                    .with_context(|| format!("invalid ESM import {specifier:?} in {url}"))?
            };
            ensure!(
                dependency.fragment().is_none(),
                "unsupported fragment on module import {dependency}"
            );
            let data_url = inline_module(
                &dependency,
                inlined_modules,
                active_modules,
                external_core,
                core_version,
            )?;
            Ok((specifier.to_owned(), data_url))
        })
        .collect::<Result<HashMap<_, _>>>()?;

    let rewritten = import_pattern()
        .replace_all(source, |captures: &Captures<'_>| {
            let specifier = captures.name("specifier").unwrap().as_str();
            format!(
                "{}{}{}{}",
                captures.name("prefix").unwrap().as_str(),
                captures.name("quote").unwrap().as_str(),
                replacements.get(specifier).expect("import was collected"),
                captures.name("closing").unwrap().as_str()
            )
        })
        .into_owned();
    Ok(source_map_pattern()
        .replace_all(&rewritten, "")
        .into_owned())
}

fn is_zenfs_core_root_import(base_url: &Url, specifier: &str) -> bool {
    if specifier == "@zenfs/core" || specifier.starts_with("@zenfs/core@") {
        return true;
    }

    let Ok(url) = base_url.join(specifier) else {
        return false;
    };
    url.host_str() == Some("esm.sh")
        && (url.path() == "/@zenfs/core"
            || url
                .path()
                .strip_prefix("/@zenfs/core@")
                .is_some_and(|suffix| {
                    suffix.is_empty() || url.path().ends_with("/core.bundle.mjs")
                }))
}

fn is_zenfs_core_internal_import(base_url: &Url, specifier: &str) -> bool {
    if specifier.starts_with("@zenfs/core/dist/") {
        return true;
    }

    let Ok(url) = base_url.join(specifier) else {
        return false;
    };
    url.host_str() == Some("esm.sh")
        && url.path().starts_with("/@zenfs/core@")
        && url.path().contains("/dist/")
}

fn zenfs_core_subpath_url(specifier: &str, version: &str) -> Result<Url> {
    let subpath = specifier
        .strip_prefix("@zenfs/core/")
        .expect("subpath specifier was checked");
    let mut url = Url::parse(&format!(
        "https://esm.sh/@zenfs/core@{version}/{subpath}?bundle&target=es2022"
    ))?;
    url.set_fragment(None);
    Ok(url)
}

fn import_pattern() -> &'static Regex {
    static PATTERN: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(
            r#"(?P<prefix>\b(?:from|import)\b\s*(?:\(\s*)?)(?P<quote>["'])(?P<specifier>[^"']+)(?P<closing>["'])"#,
        )
        .expect("static import pattern is valid")
    })
}

fn source_map_pattern() -> &'static Regex {
    static PATTERN: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"(?m)^\s*//# sourceMappingURL=.*(?:\r?\n|$)")
            .expect("static source map pattern is valid")
    })
}

fn export_list_pattern() -> &'static Regex {
    static PATTERN: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(r"\bexport\s*\{(?P<list>[^}]*)\}").expect("static export list pattern is valid")
    })
}

fn export_all_pattern() -> &'static Regex {
    static PATTERN: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(
            r#"\bexport\s*\*\s*(?:as\s+(?P<name>[A-Za-z_$][\w$]*))?\s*from\s*["'](?P<specifier>[^"']+)["']"#,
        )
        .expect("static export-all pattern is valid")
    })
}

fn export_declaration_pattern() -> &'static Regex {
    static PATTERN: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    PATTERN.get_or_init(|| {
        Regex::new(
            r"\bexport\s+(?:(?:async\s+)?(?:function|class)|const|let|var)\s+(?P<name>[A-Za-z_$][\w$]*)",
        )
        .expect("static export declaration pattern is valid")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_names_accept_shorthand_and_package_names() {
        let dom = normalize_module("dom").unwrap();
        assert_eq!(dom.package, "@zenfs/dom");
        assert_eq!(dom.namespace, "__zenfs_dom");
        assert_eq!(dom.version, "latest");

        let archives = normalize_module("@zenfs/archives@1.2.3").unwrap();
        assert_eq!(archives.package, "@zenfs/archives");
        assert_eq!(archives.namespace, "__zenfs_archives");
        assert_eq!(archives.version, "1.2.3");

        assert_eq!(normalize_module("zenfs/dom").unwrap().package, "@zenfs/dom");
    }

    #[test]
    fn module_names_cannot_escape_output_directory() {
        assert!(normalize_module("../other").is_err());
        assert!(normalize_module("@other/dom").is_err());
        assert!(normalize_module("").is_err());
        assert!(normalize_module("dom@").is_err());
    }

    #[test]
    fn additional_modules_keep_core_imports_external() {
        let url =
            Url::parse("https://esm.sh/@zenfs/dom@2.7.6?bundle&external=%40zenfs%2Fcore").unwrap();
        let source = "import { fs } from \"@zenfs/core\";\nexport { fs };";
        let mut inlined_modules = HashMap::new();
        let mut active_modules = HashSet::new();

        let rewritten = rewrite_imports(
            &url,
            source,
            &mut inlined_modules,
            &mut active_modules,
            true,
            "2.7.6",
        )
        .unwrap();

        assert!(rewritten.contains("from \"@zenfs/core\""));
    }

    #[test]
    fn esm_sh_core_root_paths_are_normalized_to_the_core_package_import() {
        let url = Url::parse("https://esm.sh/@zenfs/dom@2.7.6").unwrap();
        let source = "export * from \"/@zenfs/core@2.7.6/es2022/core.bundle.mjs\";";
        let mut inlined_modules = HashMap::new();
        let mut active_modules = HashSet::new();

        let rewritten = rewrite_imports(
            &url,
            source,
            &mut inlined_modules,
            &mut active_modules,
            true,
            "2.7.6",
        )
        .unwrap();

        assert!(rewritten.contains("export * from \"@zenfs/core\""));
    }

    #[test]
    fn core_subpaths_are_resolved_at_the_requested_core_version() {
        let url = zenfs_core_subpath_url("@zenfs/core/path", "2.7.6").unwrap();
        assert_eq!(
            url.as_str(),
            "https://esm.sh/@zenfs/core@2.7.6/path?bundle&target=es2022"
        );
        assert!(!is_zenfs_core_root_import(
            &Url::parse("https://esm.sh/@zenfs/dom@1.2.14").unwrap(),
            "@zenfs/core/path"
        ));
    }

    #[test]
    fn module_urls_externalize_core() {
        let url = package_url("@zenfs/dom", "2.7.6", true).unwrap();
        assert_eq!(
            url.query_pairs()
                .find(|(key, _)| key == "external")
                .unwrap()
                .1,
            "@zenfs/core"
        );
    }

    #[test]
    fn combined_bundle_reexports_each_module_once() {
        let core = "export const fs = {}; export default fs;".to_owned();
        let dom = "import { fs } from \"@zenfs/core\"; export const IndexedDB = fs;".to_owned();
        let core_payload = gzip_base64(&core).unwrap();
        let dom_payload = gzip_base64(&dom).unwrap();
        let bundle =
            combine_bundles(core.clone(), vec![("__zenfs_dom".to_owned(), dom.clone())]).unwrap();

        assert!(bundle.contains("__renfs_core.namespace[\"default\"]"));
        assert!(bundle.contains("__renfs_core.namespace[\"fs\"]"));
        assert!(bundle.contains("__renfs_module_0.namespace[\"IndexedDB\"]"));
        assert_eq!(bundle.matches(&core_payload).count(), 1);
        assert_eq!(bundle.matches(&dom_payload).count(), 1);
        assert!(bundle.contains("as __zenfs_dom"));
    }

    #[test]
    fn export_names_are_collected_through_embedded_star_exports() {
        let child = "export { alpha as renamed, beta }; export default {};";
        assert_eq!(
            decode_embedded_module(&as_data_url(child))
                .unwrap()
                .as_deref(),
            Some(child)
        );
        let child_url = as_gzip_data_url(child).unwrap();
        let root =
            format!("export * from \"{child_url}\"; export {{ default }} from \"{child_url}\";");

        let names = collect_export_names(&root).unwrap();
        assert!(names.contains("renamed"));
        assert!(names.contains("beta"));
        assert!(names.contains("default"));
    }

    #[test]
    fn duplicate_export_names_receive_module_prefixed_aliases() {
        let bundle = combine_bundles(
            "export const exists = true;".to_owned(),
            vec![(
                "__zenfs_dom".to_owned(),
                "export const exists = false; export const IndexedDB = exists;".to_owned(),
            )],
        )
        .unwrap();

        assert!(bundle.contains("__renfs_module_0.namespace[\"exists\"]"));
        assert!(bundle.contains("as __zenfs_dom_exists"));
        assert!(bundle.contains("__renfs_module_0.namespace[\"IndexedDB\"]"));
    }

    #[test]
    fn cli_parses_output_path_and_options() {
        let args = Cli::try_parse_from([
            "renfs",
            "custom/path/bundle.js",
            "--module",
            "dom@2.7.6",
            "--zenfs-version",
            "2.7.6",
        ])
        .unwrap();
        assert_eq!(args.output_path, PathBuf::from("custom/path/bundle.js"));
        assert_eq!(args.modules, vec!["dom@2.7.6"]);
        assert_eq!(args.zenfs_version, "2.7.6");
    }

    #[test]
    fn cli_requires_output_path() {
        assert!(Cli::try_parse_from(["renfs"]).is_err());
    }

    #[test]
    fn bundle_rejects_empty_or_directory_output_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        let dir_path = temp_dir.path().to_path_buf();

        let args = Cli {
            output_path: dir_path,
            modules: vec![],
            zenfs_version: "latest".to_owned(),
        };
        assert!(bundle(args).is_err());

        let args = Cli {
            output_path: PathBuf::from("some/dir/"),
            modules: vec![],
            zenfs_version: "latest".to_owned(),
        };
        assert!(bundle(args).is_err());

        let args = Cli {
            output_path: PathBuf::from(""),
            modules: vec![],
            zenfs_version: "latest".to_owned(),
        };
        assert!(bundle(args).is_err());
    }
}

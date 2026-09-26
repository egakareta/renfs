use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Write,
    path::PathBuf,
};

use anyhow::{Context, Result, ensure};
use base64::{Engine as _, engine::general_purpose::STANDARD};
use clap::{Args, Parser, Subcommand};
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
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// Download ZenFS and bundle it as a browser-ready ESM file.
    Bundle(BundleArgs),
}

#[derive(Debug, Args)]
struct BundleArgs {
    /// Directory to write the combined zenfs.js bundle into.
    #[arg(value_name = "DIRECTORY")]
    output_dir: PathBuf,

    /// Additional @zenfs modules to bundle (for example: dom, archives).
    /// May be repeated; use name@version to select a module version.
    #[arg(long = "module", value_name = "MODULE")]
    modules: Vec<String>,

    /// @zenfs/core version, dist-tag, or semver range to download.
    #[arg(long, default_value = "latest")]
    zenfs_version: String,
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Bundle(args) => bundle(args),
    }
}

fn bundle(args: BundleArgs) -> Result<()> {
    ensure!(
        !args.zenfs_version.trim().is_empty(),
        "ZenFS version cannot be empty"
    );

    fs::create_dir_all(&args.output_dir).with_context(|| {
        format!(
            "could not create output directory {}",
            args.output_dir.display()
        )
    })?;
    let output_dir = fs::canonicalize(&args.output_dir).with_context(|| {
        format!(
            "could not resolve output directory {}",
            args.output_dir.display()
        )
    })?;
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
        println!(
            "Downloading {}@{} and inlining its modules...",
            target.package, target.version
        );
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
    );
    let output_file = output_dir.join("zenfs.js");
    let mut staged_file = NamedTempFile::new_in(&output_dir).with_context(|| {
        format!(
            "could not create temporary file in {}",
            output_dir.display()
        )
    })?;
    staged_file
        .write_all(bundle.as_bytes())
        .context("could not write the generated ZenFS bundle")?;
    staged_file
        .persist(&output_file)
        .with_context(|| format!("could not write {}", output_file.display()))?;
    println!("Combined ZenFS bundle written to {}", output_file.display());
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

fn combine_bundles(core_bundle: String, module_bundles: Vec<(String, String)>) -> String {
    let core_url = as_data_url(&core_bundle);
    let mut output = format!(
        "/* Combined by renfs */\nexport * from \"{core_url}\";\nexport {{ default }} from \"{core_url}\";\n"
    );
    for (namespace, bundle) in module_bundles {
        let module_url = as_data_url(&bundle);
        output.push_str(&format!("export * from \"{module_url}\";\n"));
        output.push_str(&format!("export * as {namespace} from \"{module_url}\";\n"));
    }
    output
}

fn as_data_url(source: &str) -> String {
    format!(
        "data:text/javascript;base64,{}",
        STANDARD.encode(source.as_bytes())
    )
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
    let data_url = as_data_url(&inlined);
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
    fn combined_bundle_reexports_core_and_additional_modules() {
        let bundle = combine_bundles(
            "export const fs = {}; export default fs;".to_owned(),
            vec![(
                "__zenfs_dom".to_owned(),
                "import { fs } from \"@zenfs/core\"; export const IndexedDB = fs;".to_owned(),
            )],
        );

        assert!(bundle.contains("export * from \"data:text/javascript;base64,"));
        assert!(bundle.contains("export { default } from \"data:text/javascript;base64,"));
        assert!(bundle.contains("export * as __zenfs_dom from \"data:text/javascript;base64,"));

        let encoded_module = bundle
            .lines()
            .filter(|line| line.starts_with("export * from") && line.contains("data:"))
            .nth(1)
            .unwrap();
        let module_url = encoded_module
            .split("data:text/javascript;base64,")
            .nth(1)
            .unwrap()
            .trim_end_matches("\";");
        let module_source = String::from_utf8(STANDARD.decode(module_url).unwrap()).unwrap();
        assert!(module_source.contains("from \"@zenfs/core\""));
    }
}

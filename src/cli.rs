use std::{
    collections::{HashMap, HashSet},
    fs,
    io::Write,
    path::PathBuf,
};

use anyhow::{Context, Result, bail, ensure};
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
    /// Directory to write zenfs.js into.
    #[arg(value_name = "DIRECTORY")]
    output_dir: PathBuf,

    /// ZenFS package version, dist-tag, or semver range to download.
    #[arg(long, default_value = "latest")]
    zenfs_version: String,

    /// Replace an existing zenfs.js.
    #[arg(long)]
    force: bool,
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
    let output_file = output_dir.join("zenfs.js");

    if output_file.exists() && !args.force {
        bail!(
            "ZenFS bundle already exists in {} (pass --force to replace it)",
            output_dir.display()
        );
    }

    println!(
        "Downloading @zenfs/core@{} and inlining its modules...",
        args.zenfs_version
    );
    let entry_url = Url::parse(&format!(
        "https://esm.sh/@zenfs/core@{}?bundle&target=es2022",
        args.zenfs_version
    ))?;
    let bundle = download_single_file_bundle(&entry_url)?;
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

    println!("ZenFS bundle written to {}", output_file.display());
    Ok(())
}

fn download_single_file_bundle(entry_url: &Url) -> Result<String> {
    let mut inlined_modules = HashMap::new();
    let mut active_modules = HashSet::new();
    let source = download_module_source(entry_url)?;
    rewrite_imports(
        entry_url,
        &source,
        &mut inlined_modules,
        &mut active_modules,
    )
}

fn inline_module(
    url: &Url,
    inlined_modules: &mut HashMap<String, String>,
    active_modules: &mut HashSet<String>,
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
    let result = rewrite_imports(url, &source, inlined_modules, active_modules);
    active_modules.remove(&key);
    let inlined = result?;
    let data_url = format!(
        "data:text/javascript;base64,{}",
        STANDARD.encode(inlined.as_bytes())
    );
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
) -> Result<String> {
    let replacements = import_pattern()
        .captures_iter(source)
        .map(|captures| {
            let specifier = captures.name("specifier").unwrap().as_str();
            let dependency = url
                .join(specifier)
                .with_context(|| format!("invalid ESM import {specifier:?} in {url}"))?;
            ensure!(
                dependency.query().is_none() && dependency.fragment().is_none(),
                "unsupported query or fragment on module import {dependency}"
            );
            let data_url = inline_module(&dependency, inlined_modules, active_modules)?;
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

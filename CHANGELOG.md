# Changelog

## [0.2.1] - 2026-10-10

### Fixed

- Updated the lockfile to use `yoke-derive` 0.8.4 instead of the yanked 0.8.3 release.

## [0.2.0] - 2026-10-09

### Added

- Added a localStorage-backed WebFS fallback for WebAssembly. With the default `webfs` feature, RenFS now works without ZenFS setup and uses ZenFS automatically when it is configured.
- Added the standalone `webfs` crate, with a `std::fs`-style API in browsers and `std::fs` re-exports on native targets.
- Added a setupless browser example demonstrating persisted application data.

### Changed

- Changed the default Cargo features to enable `webfs` and `zenfs`; the `cli` feature is no longer enabled by default.

### Documentation

- Documented setupless browser usage, the localStorage fallback, and its storage and durability limitations.

## [0.1.0] - 2026-09-27

### Added

- Rust bindings for ZenFS, with native filesystem support and a WebAssembly backend for browser filesystems.
- Synchronous and asynchronous filesystem APIs for files, directories, metadata, links, and file handles.
- Directory-scoped operations, including file copy and rename, path and glob utilities, and temporary directories.
- File read/write streams and filesystem watchers.
- A `renfs` CLI command to download and bundle ZenFS core and optional modules. The command defaults to the DOM module, supports core-only bundles, and caches downloaded modules with an upgrade option.
- Native and WebAssembly filesystem tests, plus cross-platform CI and release workflows.

### Documentation

- Added installation and browser setup guidance, examples, contribution instructions, and notes about browser write durability.

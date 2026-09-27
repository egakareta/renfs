<div align="center">
  <h1>📂 RenFS</h1>

  <p>
    <a href="https://github.com/egakareta/renfs/actions/workflows/ci.yml"><img alt="Build Status" src="https://github.com/egakareta/renfs/actions/workflows/ci.yml/badge.svg?branch=master"/></a>
    <a href="https://crates.io/crates/renfs"><img alt="crates.io" src="https://img.shields.io/crates/v/renfs"/></a>
  </p>
</div>

## About

RenFS provides bindings for [ZenFS](https://github.com/zen-fs/core) in Rust, delegating
to browser APIs like:

- [localStorage](https://developer.mozilla.org/en-US/docs/Web/API/Window/localStorage),
- [IndexedDB](https://developer.mozilla.org/en-US/docs/Web/API/IndexedDB_API), and
- [OPFS](https://developer.mozilla.org/en-US/docs/Web/API/File_System_API/Origin_private_file_system).

For convenience, [std::fs](https://doc.rust-lang.org/std/fs/index.html) is used on native platforms.

## Usage

Please read the [examples](https://github.com/egakareta/renfs/tree/master/examples) for guidance.

## License

Licensed under the [MIT License](https://github.com/egakareta/renfs/blob/master/LICENSE).

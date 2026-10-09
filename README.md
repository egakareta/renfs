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

For convenience, [std::fs](https://doc.rust-lang.org/std/fs/index.html) is used on native platforms and works out-of-the-box on the web with [webfs](https://crates.io/crates/webfs).

## Usage

Please read the [examples](https://github.com/egakareta/renfs/tree/master/examples) for guidance.

### Install ZenFS

RenFS has a CLI to automatically install a bundled version of ZenFS directly into a project:

```sh
cargo install renfs --locked # or cargo binstall renfs
renfs dist/zenfs.js
```

Otherwise, you can always download it manually in this repository [here](https://github.com/egakareta/renfs/tree/master/vendor).

### Configure imports

In your HTML file:

```html
<script type="importmap">
  {
    "imports": {
      "@zenfs/core": "./zenfs.js",
      "@zenfs/dom": "./zenfs.js"
    }
  }
</script>
```

### Configure a browser filesystem

For most use cases, configure ZenFS before Rust application initialization.
[Trunk](https://github.com/trunk-rs/trunk) makes this easy with its `data-initializer` attribute:

```html
<link data-trunk rel="rust" data-initializer="./init.mjs" />
```

where `./init.mjs` looks like:

```js
import { configure, InMemory } from "@zenfs/core";
import { IndexedDB } from "@zenfs/dom";

await configure({
  mounts: {
    "/": IndexedDB,
    "/tmp": InMemory,
  },
});

export default () => ({});
```

### When you just need something simple

You can just not set up ZenFS. If the `webfs` feature is enabled (it is by default), RenFS can
run on the web without any additional setup with localStorage.

The obvious tradeoffs are that localStorage is not a very generous API in terms of storage size
and that a lot of advanced filesystem APIs such as symlinking or file stats will error or no-op.

## Q & A

<details> <summary> <b>Why use RenFS?</b> </summary>

Crates such as [opfs](https://github.com/anchpop/opfs), [rexie](https://github.com/devashishdxt/rexie),
[idb](https://github.com/devashishdxt/idb) are locked into one browser API and call it a day.
For a long time, this was the tradeoff developers had to make if they wanted to persist code in
browsers.

RenFS delegates to ZenFS, one of the most versatile and powerful filesystem solutions available
in browsers.

</details>

<details> <summary> <b>Are write operations durable?</b> </summary>

For synchronous APIs of some browser backends like IndexedDB and OPFS, **no**. If you write to
a file then immediately refresh the browser window, there is no guarantee that your data has
been saved. This is a fundamental limitation of ZenFS.

If you are using localStorage, you should be fine but this is subject to browser policy.

If you want durable operations, consider using asynchronous APIs such as `write_file` instead
of `write_file_sync`.

</details>

## License

Licensed under the [MIT License](https://github.com/egakareta/renfs/blob/master/LICENSE).

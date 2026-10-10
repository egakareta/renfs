import { configure, InMemory } from "@zenfs/core";
import { IndexedDB, WebAccess, WebStorage } from "@zenfs/dom";

await configure({
  mounts: {
    "/": IndexedDB,
    "/tmp": {
      backend: InMemory,
    },
    "/opfs": {
      backend: WebAccess,
      handle: await navigator.storage.getDirectory(),
    },
    "/indexeddb": IndexedDB,
    "/localstorage": { backend: WebStorage, storage: localStorage },
  },
});

export default () => ({});

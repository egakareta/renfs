import { configure } from "@zenfs/core";
import { IndexedDB } from "@zenfs/dom";

await configure({
  mounts: { "/": IndexedDB },
});

export default () => ({});

import { test } from "node:test";
import assert from "node:assert/strict";
import { collectionState, formatSize, percent, type DownloadProgress } from "../src/lib/downloadState.ts";

test("sizes in the Spanish style", () => {
  assert.equal(formatSize(0), "0 B");
  assert.equal(formatSize(1023), "1023 B");
  assert.equal(formatSize(1536), "1,5 KB");
  assert.equal(formatSize(5_412_176), "5,2 MB");
  assert.equal(formatSize(300 * 1024 * 1024), "300 MB");
  assert.equal(formatSize(3.25 * 1024 ** 3), "3,3 GB");
});

test("percentage once the size is known", () => {
  assert.equal(percent(undefined), null);
  assert.equal(percent({ received: 10, total: null }), null);
  assert.equal(percent({ received: 50, total: 200 }), 25);
  assert.equal(percent({ received: 999, total: 1000 }), 99, "100 only when complete");
  assert.equal(percent({ received: 1000, total: 1000 }), 100);
});

test("a collection: downloaded, on the way, and what is missing", () => {
  const progress = new Map<string, DownloadProgress>([
    ["s/b", { key: "s/b", state: "downloading", received: 1, total: 10, message: null }],
    ["s/c", { key: "s/c", state: "failed", received: 0, total: null, message: "403" }],
    ["s/d", { key: "s/d", state: "queued", received: 0, total: null, message: null }],
  ]);
  const items = ["a", "b", "c", "d", "e", "a"];
  const state = collectionState(items, (id) => `s/${id}`, new Set(["s/a"]), progress);
  assert.deepEqual(state, { total: 5, downloaded: 1, pending: 2, missing: ["c", "e"] });
});

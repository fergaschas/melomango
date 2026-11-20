import { test } from "node:test";
import assert from "node:assert/strict";
import { addRecent, matchingRecent, MAX_RECENT, removeRecent } from "../src/lib/recentSearches.ts";

test("newest first, each search once, at most MAX_RECENT", () => {
  let list: string[] = [];
  list = addRecent(list, "vetusta");
  list = addRecent(list, "rosalía");
  list = addRecent(list, "  Vetusta ");
  assert.deepEqual(list, ["Vetusta", "rosalía"]);
  assert.deepEqual(addRecent(list, "   "), list, "blank searches are not kept");
  for (let i = 0; i < 20; i++) list = addRecent(list, `búsqueda ${i}`);
  assert.equal(list.length, MAX_RECENT);
  assert.equal(list[0], "búsqueda 19");
});

test("removes one, ignoring case and accents", () => {
  assert.deepEqual(removeRecent(["Rosalía", "izal"], "rosalia"), ["izal"]);
});

test("suggests recent searches containing what is typed", () => {
  const list = ["vetusta morla", "rosalía saoko", "rosalía", "izal"];
  assert.deepEqual(matchingRecent(list, "rosa"), ["rosalía saoko", "rosalía"]);
  assert.deepEqual(matchingRecent(list, "ROSALIA"), ["rosalía saoko"], "not the exact same text");
  assert.deepEqual(matchingRecent(list, "", 2), ["vetusta morla", "rosalía saoko"]);
});

import { test } from "node:test";
import assert from "node:assert/strict";
import { arrange } from "../src/lib/sortTracks.ts";
import type { Track } from "../src/lib/api.ts";

const t = (id: string, title: string, artist: string, durationSecs: number): Track => ({
  id,
  source: "test",
  title,
  artist,
  durationSecs,
  artworkUrl: null,
  genre: null,
});

const tracks = [
  t("a", "Copenhague", "Vetusta Morla", 304),
  t("b", "Saoko", "ROSALÍA", 138),
  t("c", "Ángel", "Álvaro Soler", 200),
  t("d", "Bohemian Rhapsody", "Queen", 355),
];
const addedAt = [300, 100, 400, 200];
const ids = (rows: { track: Track }[]) => rows.map((r) => r.track.id);

test("keeps the list's own order by default and can reverse it", () => {
  assert.deepEqual(ids(arrange(tracks, addedAt, "", { key: "custom", desc: false })), ["a", "b", "c", "d"]);
  assert.deepEqual(ids(arrange(tracks, addedAt, "", { key: "custom", desc: true })), ["d", "c", "b", "a"]);
});

test("sorts by title and artist ignoring accents and case", () => {
  assert.deepEqual(ids(arrange(tracks, addedAt, "", { key: "title", desc: false })), ["c", "d", "a", "b"]);
  assert.deepEqual(ids(arrange(tracks, addedAt, "", { key: "artist", desc: false })), ["c", "d", "b", "a"]);
});

test("sorts by duration and by date added", () => {
  assert.deepEqual(ids(arrange(tracks, addedAt, "", { key: "duration", desc: false })), ["b", "c", "a", "d"]);
  assert.deepEqual(ids(arrange(tracks, addedAt, "", { key: "added", desc: true })), ["c", "a", "d", "b"]);
});

test("filters by title or artist, every word, without accents", () => {
  assert.deepEqual(ids(arrange(tracks, addedAt, "rosalia", { key: "custom", desc: false })), ["b"]);
  assert.deepEqual(ids(arrange(tracks, addedAt, "angel alvaro", { key: "custom", desc: false })), ["c"]);
  assert.deepEqual(ids(arrange(tracks, addedAt, "zzz", { key: "custom", desc: false })), []);
  const rows = arrange(tracks, addedAt, "queen", { key: "custom", desc: false });
  assert.equal(rows[0].index, 3, "rows keep their position in the list");
});

// Run with `npm test` (Node runs TypeScript directly).
import { test } from "node:test";
import assert from "node:assert/strict";
import * as q from "../src/lib/queue.ts";
import type { Track } from "../src/lib/api.ts";

const t = (id: string): Track => ({
  id,
  source: "test",
  title: `Canción ${id}`,
  artist: "Artista",
  durationSecs: 180,
  artworkUrl: null,
  genre: null,
});
const ids = (s: q.QueueState) => s.items.map((i) => i.track.id);
const current = (s: q.QueueState) => q.currentItem(s)?.track.id;
/** Deterministic "random": reverses the list. */
const reverse: q.Random = () => 0.999;

test("plays in order and stops at the end", () => {
  let s = q.play(q.emptyQueue, [t("a"), t("b")], 0);
  assert.equal(current(s), "a");
  s = q.advance(s, true) as q.QueueState;
  assert.equal(current(s), "b");
  assert.equal(q.advance(s, true), "stop");
  assert.equal(q.peekNext(s), null);
});

test("repeat all wraps around; repeat one replays only when the track ends by itself", () => {
  let s = q.cycleRepeat(q.play(q.emptyQueue, [t("a"), t("b")], 1));
  assert.equal(s.repeat, "all");
  assert.equal(q.peekNext(s)?.id, "a");
  assert.equal(current(q.advance(s, true) as q.QueueState), "a");
  assert.equal(current(q.back(q.play(s, [t("a"), t("b")], 0))), "b");

  s = q.cycleRepeat(s);
  assert.equal(s.repeat, "one");
  assert.equal(q.advance(s, true), "same");
  assert.equal(current(q.advance(s, false) as q.QueueState), "a", "skipping by hand moves on");
  assert.equal(q.cycleRepeat(s).repeat, "off");
});

test("shuffle keeps the current track first and restores the order when turned off", () => {
  let s = q.play(q.emptyQueue, [t("a"), t("b"), t("c"), t("d")], 1);
  s = q.setShuffle(s, true, reverse);
  assert.equal(current(s), "b");
  assert.equal(s.index, 0);
  assert.deepEqual(ids(s).slice().sort(), ["a", "b", "c", "d"]);
  s = q.advance(s, false) as q.QueueState;
  const playing = current(s);
  s = q.setShuffle(s, false);
  assert.deepEqual(ids(s), ["a", "b", "c", "d"]);
  assert.equal(current(s), playing);
});

test("playing with shuffle on starts with the chosen track", () => {
  const s = q.play({ ...q.emptyQueue, shuffle: true }, [t("a"), t("b"), t("c")], 2, reverse);
  assert.equal(current(s), "c");
  assert.equal(s.items.length, 3);
  assert.deepEqual(q.setShuffle(s, false).items.map((i) => i.track.id), ["a", "b", "c"]);
});

test("play next, add to queue, move, remove and clear", () => {
  let s = q.play(q.emptyQueue, [t("a"), t("b"), t("c")], 0);
  s = q.playNext(s, t("x"));
  s = q.addToQueue(s, t("z"));
  assert.deepEqual(ids(s), ["a", "x", "b", "c", "z"]);
  s = q.move(s, 4, 1);
  assert.deepEqual(ids(s), ["a", "z", "x", "b", "c"]);
  assert.deepEqual(ids(q.move(s, 0, 2)), ids(s), "the current track can't be moved");
  s = q.remove(s, s.items[2].uid);
  assert.deepEqual(ids(s), ["a", "z", "b", "c"]);
  assert.deepEqual(ids(q.remove(s, s.items[0].uid)), ids(s), "the current track can't be removed");
  s = q.jump(s, s.items[2].uid);
  assert.equal(current(s), "b");
  s = q.remove(s, s.items[0].uid);
  assert.equal(current(s), "b", "removing a played track keeps the current one");
  assert.deepEqual(ids(q.clearUpcoming(s)), ["z", "b"]);
});

test("queue actions while shuffled also land in the original order", () => {
  let s = q.setShuffle(q.play(q.emptyQueue, [t("a"), t("b"), t("c")], 0), true, reverse);
  s = q.playNext(s, t("x"));
  s = q.addToQueue(s, t("z"));
  assert.equal(q.upcoming(s)[0].track.id, "x");
  assert.deepEqual(ids(q.setShuffle(s, false)), ["a", "x", "b", "c", "z"]);
});

test("adding to an empty queue starts it", () => {
  const s = q.addToQueue(q.emptyQueue, t("a"));
  assert.equal(current(s), "a");
});

test("saves and restores the queue, shuffle included", () => {
  let s = q.setShuffle(q.play(q.emptyQueue, [t("a"), t("b"), t("c")], 0), true, reverse);
  s = q.cycleRepeat(q.advance(s, false) as q.QueueState);
  const restored = q.deserialize(JSON.parse(JSON.stringify(q.serialize(s))));
  assert.deepEqual(ids(restored), ids(s));
  assert.equal(current(restored), current(s));
  assert.equal(restored.repeat, "all");
  assert.deepEqual(ids(q.setShuffle(restored, false)), ["a", "b", "c"]);
});

test("big queues are trimmed around the current track when saved", () => {
  const tracks = Array.from({ length: 5000 }, (_, i) => t(String(i)));
  const s = q.jump(q.play(q.emptyQueue, tracks, 0), 0);
  const at4000 = { ...s, index: 4000 };
  const saved = q.serialize(at4000);
  assert.equal(saved.tracks.length, 3000);
  assert.equal(saved.tracks[saved.index].id, "4000");
});

test("ignores a corrupt saved index", () => {
  const restored = q.deserialize({ tracks: [t("a")], index: 7, original: null, shuffle: false, repeat: "off" });
  assert.equal(current(restored), "a");
});

test("the radio appends marked songs, also to the pre-shuffle order", () => {
  let s = q.play(q.emptyQueue, [t("a"), t("b")], 1);
  assert.equal(q.atEnd(s), true);
  s = q.setShuffle(s, true, reverse);
  s = q.appendRadio(s, [t("r1"), t("r2")]);
  assert.equal(q.atEnd(s), false);
  assert.deepEqual(q.upcoming(s).filter((i) => i.radio).map((i) => i.track.id), ["r1", "r2"]);
  assert.deepEqual(ids(q.setShuffle(s, false)).slice(-2), ["r1", "r2"]);
  assert.equal(current(q.advance(q.setShuffle(s, false), true) as q.QueueState), "r1");
});

test("radio marks survive saving and restoring", () => {
  const s = q.appendRadio(q.play(q.emptyQueue, [t("a")], 0), [t("r1")]);
  const restored = q.deserialize(JSON.parse(JSON.stringify(q.serialize(s))));
  assert.deepEqual(restored.items.map((i) => !!i.radio), [false, true]);
});

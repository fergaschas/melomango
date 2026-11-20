import { test } from "node:test";
import assert from "node:assert/strict";
import { freshListen, listenTick, parseSaved, PLAY_COUNTS_AFTER } from "../src/lib/playerState.ts";

/** Feeds positions to listenTick; returns at which update (if any) it became a play. */
function playThrough(positions: number[], length: number, playing = true) {
  let l = freshListen();
  let countedAt: number | null = null;
  positions.forEach((p, i) => {
    const r = listenTick(l, p, playing, length);
    l = r.next;
    if (r.counts) {
      assert.equal(countedAt, null, "counted only once");
      countedAt = i;
    }
  });
  return { l, countedAt };
}
const steps = (from: number, to: number, step = 0.25) =>
  Array.from({ length: Math.round((to - from) / step) + 1 }, (_, i) => from + i * step);

test("a song counts as listened after 30 s of actual playback", () => {
  const { countedAt, l } = playThrough(steps(0, 40), 200);
  assert.equal(countedAt, PLAY_COUNTS_AFTER / 0.25);
  assert.equal(l.recorded, true);
});

test("short songs count at half their length", () => {
  // A 40 s song: half of it, 20 s.
  assert.equal(playThrough(steps(0, 25), 40).countedAt, 20 / 0.25);
});

test("seeking forward is not listening", () => {
  // 10 s played, then a jump to 3:00, then 10 s more: 20 s heard.
  assert.equal(playThrough([...steps(0, 10), ...steps(180, 190)], 240).countedAt, null);
});

test("paused time doesn't count", () => {
  assert.equal(playThrough(steps(0, 40), 200, false).countedAt, null);
});

test("reads the saved state, with the position saved apart", () => {
  const raw = JSON.stringify({ queue: { tracks: [], index: -1, original: null, shuffle: false, repeat: "off" }, time: 12, volume: 0.5, muted: false });
  assert.equal(parseSaved(raw, null)?.time, 12, "older versions kept the time in it");
  assert.equal(parseSaved(raw, "95.5")?.time, 95.5);
  assert.equal(parseSaved(raw, "nope")?.time, 12);
  assert.equal(parseSaved(null, "3"), null);
  assert.equal(parseSaved("{broken", null), null);
  assert.equal(parseSaved("{}", null), null, "no queue: nothing to restore");
});

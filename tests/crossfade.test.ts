import { test } from "node:test";
import assert from "node:assert/strict";
import { clampCrossfade, fadeGains, MAX_CROSSFADE, shouldStartFade } from "../src/lib/crossfade.ts";

test("starts the next song when the crossfade's seconds are left", () => {
  assert.equal(shouldStartFade(6, 190, 200), false, "10 s left");
  assert.equal(shouldStartFade(6, 194, 200), true, "6 s left");
  assert.equal(shouldStartFade(6, 199.9, 200), false, "too late to blend");
  assert.equal(shouldStartFade(0, 199, 200), false, "off");
  assert.equal(shouldStartFade(6, 5, 10), false, "too short a song");
  assert.equal(shouldStartFade(6, 100, Infinity), false, "unknown length");
  assert.equal(shouldStartFade(6, 100, NaN), false, "not loaded yet");
});

test("equal-power blend from the old song to the new one", () => {
  assert.deepEqual(fadeGains(0, 6, 6), { in: 0, out: 1 });
  const mid = fadeGains(3, 6, 6);
  assert.ok(Math.abs(mid.in ** 2 + mid.out ** 2 - 1) < 1e-9, "same loudness halfway");
  assert.deepEqual(fadeGains(6, 6, 6), { in: 1, out: 0 });
  assert.deepEqual(fadeGains(2, 6, 1), { in: Math.sin(Math.PI / 6), out: 0 }, "the old one may run out first");
  assert.deepEqual(fadeGains(9, 6, 6), { in: 1, out: 0 }, "past the end");
});

test("keeps the setting within range, in whole seconds", () => {
  assert.equal(clampCrossfade(-3), 0);
  assert.equal(clampCrossfade(4.6), 5);
  assert.equal(clampCrossfade(99), MAX_CROSSFADE);
  assert.equal(clampCrossfade(NaN), 0);
});

import { test } from "node:test";
import assert from "node:assert/strict";
import {
  LoudnessMeter,
  MIN_FRAMES,
  normalizationGainDb,
  remember,
  rmsDb,
  TARGET_DB,
  typicalLoudness,
} from "../src/lib/loudness.ts";

const close = (a: number, b: number, eps = 0.05) => assert.ok(Math.abs(a - b) < eps, `${a} ≈ ${b}`);

test("RMS level of a block, in dBFS", () => {
  close(rmsDb(new Float32Array(100).fill(1)), 0);
  close(rmsDb(new Float32Array(100).fill(0.5)), -6.02);
  // A full-scale sine is 3 dB below its peak.
  const sine = Float32Array.from({ length: 4800 }, (_, i) => Math.sin((2 * Math.PI * 440 * i) / 48000));
  close(rmsDb(sine), -3.01);
  assert.equal(rmsDb(new Float32Array(10)), -Infinity);
});

test("averages by energy, ignoring silence, once enough is heard", () => {
  const m = new LoudnessMeter();
  for (let i = 0; i < MIN_FRAMES - 1; i++) m.add(-10);
  assert.equal(m.value, null, "not enough yet");
  m.add(-Infinity);
  m.add(-70);
  assert.equal(m.value, null, "silence doesn't count");
  m.add(-10);
  close(m.value!, -10);
  for (let i = 0; i < MIN_FRAMES; i++) m.add(-20);
  close(m.value!, 10 * Math.log10((10 ** -1 + 10 ** -2) / 2), 0.01);
});

test("brings songs to the target, boosting at most 6 dB and cutting at most 12", () => {
  assert.equal(normalizationGainDb(-8), TARGET_DB + 8);
  assert.equal(normalizationGainDb(TARGET_DB), 0);
  assert.equal(normalizationGainDb(-40), 6);
  assert.equal(normalizationGainDb(5), -12);
});

test("remembers the latest songs, newest last", () => {
  let store: Record<string, number> = {};
  store = remember(store, "a", -9.04, 2);
  store = remember(store, "b", -12, 2);
  store = remember(store, "a", -10, 2);
  assert.deepEqual(Object.keys(store), ["b", "a"]);
  store = remember(store, "c", -11, 2);
  assert.deepEqual(store, { a: -10, c: -11 });
});

test("guesses an unknown song from the ones measured", () => {
  assert.equal(typicalLoudness({}), -10);
  assert.equal(typicalLoudness({ a: -8, b: -12 }), -10);
  assert.equal(typicalLoudness({ a: -14 }), -14);
});

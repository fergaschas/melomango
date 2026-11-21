import { test } from "node:test";
import assert from "node:assert/strict";
import { formatTime, formatTotal } from "../src/lib/format.ts";

test("song positions and lengths as m:ss", () => {
  assert.equal(formatTime(0), "0:00");
  assert.equal(formatTime(5.9), "0:05");
  assert.equal(formatTime(61), "1:01");
  assert.equal(formatTime(3600), "60:00");
  assert.equal(formatTime(-1), "0:00");
  assert.equal(formatTime(NaN), "0:00");
  assert.equal(formatTime(Infinity), "0:00", "a stream of unknown length");
});

test("total length of a list in minutes, or hours and minutes", () => {
  assert.equal(formatTotal(0), "0 min");
  assert.equal(formatTotal(29), "0 min");
  assert.equal(formatTotal(30), "1 min");
  assert.equal(formatTotal(59 * 60), "59 min");
  assert.equal(formatTotal(3600), "1 h 0 min");
  assert.equal(formatTotal(2 * 3600 + 56 * 60), "2 h 56 min");
});

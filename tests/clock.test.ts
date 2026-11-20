import { test } from "node:test";
import assert from "node:assert/strict";
import { Clock } from "../src/lib/clock.ts";

test("tells its listeners when the position changes, and only then", () => {
  const clock = new Clock(5);
  assert.equal(clock.get(), 5);
  let calls = 0;
  const stop = clock.subscribe(() => calls++);
  clock.set(6);
  clock.set(6);
  assert.equal(calls, 1, "the same position again is not a change");
  assert.equal(clock.get(), 6);
  stop();
  clock.set(7);
  assert.equal(calls, 1, "unsubscribed");
});

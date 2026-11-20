import { test } from "node:test";
import assert from "node:assert/strict";
import { MAX_BACK, navReducer, type NavState } from "../src/lib/navigation.ts";

type View = { kind: string; id?: number };
const start: NavState<View> = { view: { kind: "home" }, back: [] };
const go = (s: NavState<View>, view: View) => navReducer(s, { type: "go", view });
const back = (s: NavState<View>) => navReducer(s, { type: "back" });

test("going somewhere remembers where it was; back returns there", () => {
  let s = go(start, { kind: "likes" });
  s = go(s, { kind: "playlist", id: 3 });
  assert.deepEqual(s.view, { kind: "playlist", id: 3 });
  assert.deepEqual(s.back, [{ kind: "home" }, { kind: "likes" }]);
  s = back(s);
  assert.deepEqual(s.view, { kind: "likes" });
  s = back(back(s));
  assert.deepEqual(s.view, { kind: "home" });
  assert.equal(back(s), s, "nothing left: stays");
});

test("opening the view already shown adds no step", () => {
  const s = go(start, { kind: "playlist", id: 3 });
  assert.equal(go(s, { kind: "playlist", id: 3 }), s);
  assert.equal(go(s, { kind: "playlist", id: 4 }).back.length, 2, "another playlist is another view");
});

test("back remembers a bounded number of views", () => {
  let s = start;
  for (let i = 0; i < MAX_BACK + 20; i++) s = go(s, { kind: "playlist", id: i });
  assert.equal(s.back.length, MAX_BACK);
  assert.deepEqual(s.back[s.back.length - 1], { kind: "playlist", id: MAX_BACK + 18 });
});

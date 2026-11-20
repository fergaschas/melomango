import { test } from "node:test";
import assert from "node:assert/strict";
import { ownsKey } from "../src/lib/keyTarget.ts";

test("nothing focused: every key can be a shortcut", () => {
  assert.equal(ownsKey(null, " "), false);
  assert.equal(ownsKey({ tag: "div" }, " "), false);
});

test("text fields keep every key", () => {
  assert.equal(ownsKey({ tag: "input", type: "text" }, "m"), true);
  assert.equal(ownsKey({ tag: "input", type: "search" }, " "), true);
  assert.equal(ownsKey({ tag: "textarea" }, "q"), true);
  assert.equal(ownsKey({ tag: "div", editable: true }, "l"), true);
  assert.equal(ownsKey({ tag: "select" }, "ArrowDown"), true);
});

test("sliders keep only their movement keys", () => {
  assert.equal(ownsKey({ tag: "input", type: "range" }, "ArrowRight"), true);
  assert.equal(ownsKey({ tag: "input", type: "range" }, "Home"), true);
  assert.equal(ownsKey({ tag: "input", type: "range" }, " "), false, "Space still plays/pauses");
  assert.equal(ownsKey({ tag: "input", type: "range" }, "m"), false);
});

test("Space and Enter press the focused button, link or menu item", () => {
  assert.equal(ownsKey({ tag: "button" }, " "), true, "Space on «Me gusta» likes, it doesn't pause");
  assert.equal(ownsKey({ tag: "button" }, "Enter"), true);
  assert.equal(ownsKey({ tag: "a" }, " "), true);
  assert.equal(ownsKey({ tag: "div", role: "menuitem" }, " "), true);
  assert.equal(ownsKey({ tag: "li", role: "option" }, "Enter"), true);
  // Other keys are still shortcuts there.
  assert.equal(ownsKey({ tag: "button" }, "m"), false);
  assert.equal(ownsKey({ tag: "button" }, "ArrowRight"), false, "seeking still works from a button");
});

test("menus, lists of options and tabs move with the arrows", () => {
  assert.equal(ownsKey({ tag: "button", role: "menuitem" }, "ArrowDown"), true);
  assert.equal(ownsKey({ tag: "li", role: "option" }, "ArrowUp"), true);
  assert.equal(ownsKey({ tag: "button", role: "tab" }, "ArrowLeft"), true);
});

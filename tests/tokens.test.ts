import { test } from "node:test";
import assert from "node:assert/strict";
import fs from "node:fs";
import path from "node:path";
import {
  closestPairs,
  contrast,
  contrastResults,
  cssName,
  deltaE,
  generateAll,
  kotlinName,
  loadTokens,
  MODES,
  ROOT,
} from "../scripts/tokens.ts";

const set = loadTokens();

test("what is generated from the tokens is up to date (npm run tokens)", () => {
  for (const [rel, text] of Object.entries(generateAll(set))) {
    const file = path.join(ROOT, rel);
    const onDisk = fs.existsSync(file) ? fs.readFileSync(file, "utf8").replace(/\r\n/g, "\n") : "";
    assert.ok(onDisk === text, `${rel} no coincide con los tokens: ejecuta npm run tokens y súbelo`);
    assert.match(onDisk.split("\n").slice(0, 3).join("\n"), /Generado por scripts\/tokens\.ts/, `${rel}: sin cabecera`);
  }
});

test("both themes have the same tokens, every reference resolves", () => {
  assert.deepEqual([...set.modes.claro.keys()].sort(), [...set.modes.oscuro.keys()].sort());
  for (const mode of MODES) {
    for (const t of set.modes[mode].values()) {
      if (t.type === "color") assert.match(t.value as string, /^#[0-9a-f]{6}([0-9a-f]{2})?$/, `${mode} ${t.name}`);
    }
  }
});

test("every text and mark passes its contrast in both themes", () => {
  const failing = contrastResults(set)
    .filter((r) => !r.ok)
    .map((r) => `${r.mode}: ${r.fg} sobre ${r.bg} = ${r.ratio.toFixed(2)} (mínimo ${r.min})`);
  assert.deepEqual(failing, [], failing.join("\n"));
});

test("data colors stay apart with every kind of color blindness", () => {
  const min = set.contrast.distinct.minDeltaE;
  const close = closestPairs(set)
    .filter((r) => r.deltaE < min)
    .map((r) => `${r.mode}, ${r.vision}: ${r.a} y ${r.b} (ΔE ${r.deltaE.toFixed(1)})`);
  assert.deepEqual(close, [], close.join("\n"));
});

test("every Material 3 role takes a semantic color", () => {
  for (const [role, name] of Object.entries(set.material3)) {
    for (const mode of MODES) assert.equal(set.modes[mode].get(name)?.type, "color", `${role} → ${name} (${mode})`);
  }
  assert.equal(set.material3.onPrimary, "color.accent.on", "text on the mango is the dark one in both apps");
});

test("the audit's fixes hold", () => {
  const claro = set.modes.claro;
  const oscuro = set.modes.oscuro;
  // KAN-85: the dark error background had drifted on mobile (#3B1A16).
  assert.equal(oscuro.get("color.danger.bg")?.value, "#3d1f1c");
  // Muted and orange text pass AA on every light surface (they were 4.21 and 4.34).
  for (const bg of ["color.bg", "color.surface.soft", "color.surface.hover"]) {
    const on = claro.get(bg)!.value as string;
    assert.ok(contrast(claro.get("color.text.muted")!.value as string, on) >= 4.5, `muted sobre ${bg}`);
    assert.ok(contrast(claro.get("color.accent.text")!.value as string, on) >= 4.5, `accent-text sobre ${bg}`);
  }
});

test("names map the same way in CSS and Kotlin", () => {
  assert.equal(cssName("color.text.muted"), "--color-text-muted");
  assert.equal(kotlinName("color.text.muted"), "textMuted");
  assert.equal(kotlinName("color.decor.cover-soft"), "decorCoverSoft");
  assert.equal(kotlinName("color.data.1"), "data1");
  assert.equal(kotlinName("size.player-height"), "sizePlayerHeight");
});

test("contrast and color-blindness math match known values", () => {
  assert.equal(contrast("#000000", "#ffffff").toFixed(1), "21.0");
  assert.equal(contrast("#ffffff", "#ffa21f").toFixed(2), "2.01");
  // 50 % black over white is a mid grey.
  assert.ok(Math.abs(contrast("#00000080", "#ffffff") - 3.98) < 0.05);
  // Red and green are far apart normally and close for a deuteranope.
  assert.ok(deltaE("#ff0000", "#00ff00") > deltaE("#ff0000", "#00ff00", "deuteranopia") * 2);
});

/**
 * The design tokens of Melomango (design/tokens/, W3C Design Tokens format) turned into what
 * each app uses (KAN-87):
 *
 * - desktop: `src/tokens.css`, CSS variables for light and dark (`prefers-color-scheme` and
 *   `data-theme`, as App.css always did);
 * - mobile: `MelomangoTokens.kt`, Compose colors, sizes and the Material 3 ColorScheme of each
 *   theme. A copy lives in `design/generated/` (checked here) and goes to the mobile repo;
 * - `docs/sistema-diseno/tokens.md`, the reference: every token with its CSS, Kotlin and
 *   Material name, its values and the contrast checks.
 *
 *   npm run tokens                             regenerate
 *   npm run tokens -- --mobile ../melomango-mobile   … and copy the Kotlin file there
 *   npm run tokens -- --check [--mobile …]     fail if anything generated is out of date
 *
 * Generated files say so in their first line and are never edited by hand; `npm test`
 * (tests/tokens.test.ts) fails when they don't match the tokens, like bindings.ts.
 */
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const TOKENS_DIR = path.join(ROOT, "design", "tokens");
/** Where the Kotlin file goes inside the mobile repo. */
export const MOBILE_KOTLIN = "app/src/main/kotlin/com/fgascon/melomango/ui/theme/MelomangoTokens.kt";
export const OUTPUTS = {
  css: "src/tokens.css",
  kotlin: "design/generated/MelomangoTokens.kt",
  docs: "docs/sistema-diseno/tokens.md",
} as const;

export type Mode = "claro" | "oscuro";
export const MODES: Mode[] = ["claro", "oscuro"];

type Json = { [key: string]: unknown };

export interface Token {
  /** Canonical name: the path without a final `default` ("color.text.muted"). */
  name: string;
  type: string;
  /** As written, references unresolved. */
  raw: unknown;
  description: string;
  extensions: Json;
}

export interface GradientStop {
  color: string;
  position: number;
}

export interface Shadow {
  color: string;
  offsetX: number;
  offsetY: number;
  blur: number;
  spread: number;
}

/** A token with its references resolved: colors are lowercase #rrggbb or #rrggbbaa. */
export type Value = string | number | GradientStop[] | Shadow | { value: number; unit: string };

export interface Resolved extends Token {
  value: Value;
}

export interface ContrastPair {
  fg: string;
  bg: string[];
  min: number;
  why?: string;
}

export interface TokenSet {
  version: string;
  primitives: Map<string, Token>;
  /** Semantic tokens of each theme, resolved. */
  modes: Record<Mode, Map<string, Resolved>>;
  /** Tokens that don't change with the theme (sizes, opacities), resolved. */
  shared: Map<string, Resolved>;
  material3: Record<string, string>;
  contrast: { pairs: ContrastPair[]; distinct: { tokens: string[]; minDeltaE: number } };
}

const readJson = (file: string): Json => JSON.parse(fs.readFileSync(path.join(TOKENS_DIR, file), "utf8")) as Json;

/** Every token of a DTCG tree, by canonical name; `$type` is inherited from the groups. */
export function flatten(tree: Json, prefix: string[] = [], inheritedType = ""): Map<string, Token> {
  const out = new Map<string, Token>();
  const type = typeof tree.$type === "string" ? tree.$type : inheritedType;
  if ("$value" in tree) {
    const parts = prefix[prefix.length - 1] === "default" ? prefix.slice(0, -1) : prefix;
    const name = parts.join(".");
    if (!type) throw new Error(`${name}: sin $type`);
    out.set(name, {
      name,
      type,
      raw: tree.$value,
      description: typeof tree.$description === "string" ? tree.$description : "",
      extensions: (tree.$extensions as Json | undefined) ?? {},
    });
    return out;
  }
  for (const [key, child] of Object.entries(tree)) {
    if (key.startsWith("$") || typeof child !== "object" || child === null) continue;
    for (const [name, token] of flatten(child as Json, [...prefix, key], type)) {
      if (out.has(name)) throw new Error(`${name}: definido dos veces`);
      out.set(name, token);
    }
  }
  return out;
}

const REFERENCE = /^\{([^{}]+)\}$/;
const HEX = /^#[0-9a-f]{6}([0-9a-f]{2})?$/;

function resolveColor(raw: unknown, lookup: (name: string) => Token | undefined, seen: string[]): string {
  if (typeof raw !== "string") throw new Error(`color no válido: ${JSON.stringify(raw)}`);
  const ref = REFERENCE.exec(raw);
  if (ref) {
    const name = ref[1]!;
    if (seen.includes(name)) throw new Error(`referencia circular: ${[...seen, name].join(" → ")}`);
    const target = lookup(name);
    if (!target) throw new Error(`no existe el token {${name}}`);
    return resolveColor(target.raw, lookup, [...seen, name]);
  }
  const hex = raw.toLowerCase();
  if (!HEX.test(hex)) throw new Error(`color no válido: ${raw} (se esperaba #rrggbb o #rrggbbaa)`);
  return hex;
}

function dimension(raw: unknown, name: string): number {
  const d = raw as { value?: unknown; unit?: unknown };
  if (typeof d?.value !== "number" || d.unit !== "px") throw new Error(`${name}: dimensión no válida (se esperaba { value, unit: "px" })`);
  return d.value;
}

function resolve(token: Token, lookup: (name: string) => Token | undefined): Resolved {
  const at = (raw: unknown) => resolveColor(raw, lookup, [token.name]);
  switch (token.type) {
    case "color":
      return { ...token, value: at(token.raw) };
    case "gradient": {
      if (!Array.isArray(token.raw)) throw new Error(`${token.name}: un degradado es una lista de paradas`);
      return {
        ...token,
        value: token.raw.map((stop: { color: unknown; position: unknown }) => {
          if (typeof stop.position !== "number") throw new Error(`${token.name}: parada sin posición`);
          return { color: at(stop.color), position: stop.position };
        }),
      };
    }
    case "shadow": {
      const s = token.raw as Json;
      return {
        ...token,
        value: {
          color: at(s.color),
          offsetX: dimension(s.offsetX, token.name),
          offsetY: dimension(s.offsetY, token.name),
          blur: dimension(s.blur, token.name),
          spread: dimension(s.spread, token.name),
        },
      };
    }
    case "dimension":
      return { ...token, value: { value: dimension(token.raw, token.name), unit: "px" } };
    case "number":
      if (typeof token.raw !== "number") throw new Error(`${token.name}: se esperaba un número`);
      return { ...token, value: token.raw };
    default:
      throw new Error(`${token.name}: tipo ${token.type} no soportado`);
  }
}

/** Reads design/tokens/ and resolves every reference. Throws on anything inconsistent. */
export function loadTokens(): TokenSet {
  const manifest = readJson("manifest.json") as {
    version: string;
    primitives: string;
    modes: Record<Mode, string>;
    shared: string[];
    material3: string;
    contrast: string;
  };
  const primitives = flatten(readJson(manifest.primitives));
  const shared = new Map<string, Resolved>();
  for (const file of manifest.shared) {
    for (const [name, token] of flatten(readJson(file))) shared.set(name, resolve(token, (n) => primitives.get(n)));
  }
  const modes = {} as Record<Mode, Map<string, Resolved>>;
  for (const mode of MODES) {
    const semantic = flatten(readJson(manifest.modes[mode]));
    const lookup = (n: string) => primitives.get(n) ?? semantic.get(n);
    modes[mode] = new Map([...semantic].map(([name, token]) => [name, resolve(token, lookup)]));
  }
  const [claro, oscuro] = [[...modes.claro.keys()], [...modes.oscuro.keys()]];
  const onlyOne = [...claro.filter((k) => !modes.oscuro.has(k)), ...oscuro.filter((k) => !modes.claro.has(k))];
  if (onlyOne.length) throw new Error(`tokens que solo existen en un tema: ${onlyOne.join(", ")}`);
  for (const name of claro) {
    if (modes.claro.get(name)!.type !== modes.oscuro.get(name)!.type) throw new Error(`${name}: tipo distinto en cada tema`);
  }
  const material3 = Object.fromEntries(
    Object.entries(readJson(manifest.material3)).filter(([k]) => !k.startsWith("$")),
  ) as Record<string, string>;
  for (const [role, name] of Object.entries(material3)) {
    if (modes.claro.get(name)?.type !== "color") throw new Error(`Material 3 ${role}: ${name} no es un color`);
  }
  const contrast = readJson(manifest.contrast) as unknown as TokenSet["contrast"];
  return { version: manifest.version, primitives, modes, shared, material3, contrast };
}

// ---------- Names ----------

/** "color.text.muted" -> "--color-text-muted". */
export const cssName = (name: string) => `--${name.replace(/\./g, "-")}`;

/** "color.decor.cover-soft" -> "decorCoverSoft" (colors drop their "color." prefix). */
export function kotlinName(name: string): string {
  const words = name.replace(/^color\./, "").split(/[.-]/);
  return words.map((w, i) => (i === 0 ? w : w.charAt(0).toUpperCase() + w.slice(1))).join("");
}

// ---------- Color math ----------

export function rgba(hex: string): [number, number, number, number] {
  const h = hex.slice(1);
  const n = (i: number) => parseInt(h.slice(i, i + 2), 16);
  return [n(0), n(2), n(4), h.length === 8 ? n(6) / 255 : 1];
}

const channel = (v: number) => {
  const c = v / 255;
  return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
};

/** A translucent color painted over an opaque one. */
export function over(fg: string, bg: string): [number, number, number] {
  const [r, g, b, a] = rgba(fg);
  const [br, bgG, bb] = rgba(bg);
  return [r * a + br * (1 - a), g * a + bgG * (1 - a), b * a + bb * (1 - a)];
}

const luminance = ([r, g, b]: [number, number, number]) =>
  0.2126 * channel(r) + 0.7152 * channel(g) + 0.0722 * channel(b);

/** WCAG contrast of `fg` (with its transparency) over the opaque `bg`. */
export function contrast(fg: string, bg: string): number {
  const a = luminance(over(fg, bg));
  const b = luminance(over(bg, bg));
  return (Math.max(a, b) + 0.05) / (Math.min(a, b) + 0.05);
}

/** Machado et al. (2009), severity 1, applied to linear RGB. */
const CVD = {
  protanopia: [
    [0.152286, 1.052583, -0.204868],
    [0.114503, 0.786281, 0.099216],
    [-0.003882, -0.048116, 1.051998],
  ],
  deuteranopia: [
    [0.367322, 0.860646, -0.227968],
    [0.280085, 0.672501, 0.047413],
    [-0.01182, 0.04294, 0.968881],
  ],
  tritanopia: [
    [1.255528, -0.076749, -0.178779],
    [-0.078411, 0.930809, 0.147602],
    [0.004733, 0.691367, 0.3039],
  ],
} as const;
export type Vision = "normal" | keyof typeof CVD;
export const VISIONS: Vision[] = ["normal", "protanopia", "deuteranopia", "tritanopia"];

function lab(hex: string, vision: Vision): [number, number, number] {
  const [r, g, b] = rgba(hex);
  let lin = [channel(r), channel(g), channel(b)];
  if (vision !== "normal") {
    lin = CVD[vision].map((row) => Math.min(1, Math.max(0, row[0] * lin[0]! + row[1] * lin[1]! + row[2] * lin[2]!)));
  }
  const [lr, lg, lb] = lin as [number, number, number];
  const x = (0.4124 * lr + 0.3576 * lg + 0.1805 * lb) / 0.95047;
  const y = 0.2126 * lr + 0.7152 * lg + 0.0722 * lb;
  const z = (0.0193 * lr + 0.1192 * lg + 0.9505 * lb) / 1.08883;
  const f = (t: number) => (t > 0.008856 ? Math.cbrt(t) : 7.787 * t + 16 / 116);
  return [116 * f(y) - 16, 500 * (f(x) - f(y)), 200 * (f(y) - f(z))];
}

/** How different two colors look (CIE76 ΔE) to someone with `vision`. */
export function deltaE(a: string, b: string, vision: Vision = "normal"): number {
  const [l1, a1, b1] = lab(a, vision);
  const [l2, a2, b2] = lab(b, vision);
  return Math.hypot(l1 - l2, a1 - a2, b1 - b2);
}

/** The colors behind a background token: one, or each stop of a gradient. */
export function backgroundColors(token: Resolved): string[] {
  if (token.type === "color") return [token.value as string];
  if (token.type === "gradient") return (token.value as GradientStop[]).map((s) => s.color);
  throw new Error(`${token.name} no es un fondo`);
}

export interface ContrastResult {
  mode: Mode;
  fg: string;
  bg: string;
  ratio: number;
  min: number;
  ok: boolean;
}

/** Every pair of contraste.json in both themes (gradients: their worst stop). */
export function contrastResults(set: TokenSet): ContrastResult[] {
  const out: ContrastResult[] = [];
  for (const mode of MODES) {
    const tokens = set.modes[mode];
    const get = (name: string) => {
      const t = tokens.get(name);
      if (!t) throw new Error(`contraste.json: no existe ${name}`);
      return t;
    };
    for (const pair of set.contrast.pairs) {
      const fg = get(pair.fg);
      if (fg.type !== "color") throw new Error(`contraste.json: ${pair.fg} no es un color`);
      for (const bgName of pair.bg) {
        const ratio = Math.min(...backgroundColors(get(bgName)).map((bg) => contrast(fg.value as string, bg)));
        out.push({ mode, fg: pair.fg, bg: bgName, ratio, min: pair.min, ok: ratio >= pair.min });
      }
    }
  }
  return out;
}

export interface DistinctResult {
  mode: Mode;
  vision: Vision;
  a: string;
  b: string;
  deltaE: number;
}

/** The closest pair of the "distinct" colors, per theme and kind of vision. */
export function closestPairs(set: TokenSet): DistinctResult[] {
  const out: DistinctResult[] = [];
  for (const mode of MODES) {
    const colors = set.contrast.distinct.tokens.map((n) => [n, set.modes[mode].get(n)!.value as string] as const);
    for (const vision of VISIONS) {
      let best: DistinctResult | null = null;
      for (let i = 0; i < colors.length; i++) {
        for (let j = i + 1; j < colors.length; j++) {
          const d = deltaE(colors[i]![1], colors[j]![1], vision);
          if (!best || d < best.deltaE) best = { mode, vision, a: colors[i]![0], b: colors[j]![0], deltaE: d };
        }
      }
      if (best) out.push(best);
    }
  }
  return out;
}

// ---------- Outputs ----------

const pct = (p: number) => `${Math.round(p * 1000) / 10}%`;

function cssValue(t: Resolved): string {
  switch (t.type) {
    case "color":
      return t.value as string;
    case "gradient": {
      const angle = (t.extensions["com.melomango"] as { angle?: number } | undefined)?.angle ?? 180;
      const stops = (t.value as GradientStop[]).map((s) => `${s.color} ${pct(s.position)}`).join(", ");
      return `linear-gradient(${angle}deg, ${stops})`;
    }
    case "shadow": {
      const s = t.value as Shadow;
      const px = (n: number) => (n === 0 ? "0" : `${n}px`);
      return `${px(s.offsetX)} ${px(s.offsetY)} ${px(s.blur)}${s.spread ? ` ${px(s.spread)}` : ""} ${s.color}`;
    }
    case "dimension":
      return `${(t.value as { value: number }).value}px`;
    default:
      return String(t.value);
  }
}

const HEADER = (version: string, comment: string) =>
  `${comment} Generado por scripts/tokens.ts desde design/tokens/ (versión ${version}). No lo edites a mano:\n` +
  `${comment} cambia los tokens y ejecuta \`npm run tokens\` en el repo melomango.`;

export function generateCss(set: TokenSet): string {
  const block = (tokens: Map<string, Resolved>, indent: string) =>
    [...tokens.values()].map((t) => `${indent}${cssName(t.name)}: ${cssValue(t)};`).join("\n");
  return [
    `/*\n${HEADER(set.version, " *")}\n *\n * Claro en :root; oscuro si Windows lo usa (salvo que se elija Claro en Ajustes) o si se elige\n * Oscuro (data-theme en <html>). Docs: docs/sistema-diseno/tokens.md\n */`,
    "",
    ":root {",
    block(set.shared, "  "),
    block(set.modes.claro, "  "),
    "  color-scheme: light;",
    "}",
    "",
    "@media (prefers-color-scheme: dark) {",
    '  :root:not([data-theme="light"]) {',
    block(set.modes.oscuro, "    "),
    "    color-scheme: dark;",
    "  }",
    "}",
    "",
    ':root[data-theme="dark"] {',
    block(set.modes.oscuro, "  "),
    "  color-scheme: dark;",
    "}",
    "",
  ].join("\n");
}

const kotlinColor = (hex: string) => {
  const [r, g, b, a] = rgba(hex);
  const h = (n: number) => n.toString(16).padStart(2, "0").toUpperCase();
  return `Color(0x${h(Math.round(a * 255))}${h(r)}${h(g)}${h(b)})`;
};

function kotlinValue(t: Resolved): string {
  switch (t.type) {
    case "color":
      return kotlinColor(t.value as string);
    case "gradient":
      return `listOf(${(t.value as GradientStop[]).map((s) => `${s.position}f to ${kotlinColor(s.color)}`).join(", ")})`;
    case "dimension":
      return `${(t.value as { value: number }).value}.dp`;
    case "number":
      return `${t.value}f`;
    default:
      throw new Error(`${t.name}: ${t.type} no se genera para Kotlin`);
  }
}

const kotlinType = (t: Resolved) => (t.type === "gradient" ? "List<Pair<Float, Color>>" : "Color");
const kdoc = (text: string, indent: string) => (text ? `${indent}/** ${text.replace(/\*\//g, "* /")} */\n` : "");

export function generateKotlin(set: TokenSet): string {
  // Compose has elevation instead of CSS shadows: those stay on desktop.
  const colorTokens = [...set.modes.claro.values()].filter((t) => t.type === "color" || t.type === "gradient");
  const fields = colorTokens
    .map((t) => `${kdoc(`${t.description} (\`${cssName(t.name)}\`)`.trim(), "    ")}    val ${kotlinName(t.name)}: ${kotlinType(t)},`)
    .join("\n");
  const instance = (mode: Mode) =>
    colorTokens.map((t) => `        ${kotlinName(t.name)} = ${kotlinValue(set.modes[mode].get(t.name)!)},`).join("\n");
  const shared = [...set.shared.values()]
    .map((t) => `${kdoc(`${t.description} (\`${cssName(t.name)}\`)`, "    ")}    val ${kotlinName(t.name)} = ${kotlinValue(t)}`)
    .join("\n");
  const scheme = (fn: string, colors: string) =>
    `    val ${colors}ColorScheme: ColorScheme = ${fn}(\n` +
    Object.entries(set.material3)
      .map(([role, name]) => `        ${role} = ${colors}.${kotlinName(name)},`)
      .join("\n") +
    "\n    )";
  return `${HEADER(set.version, "//")}
@file:Suppress("MagicNumber")

package com.fgascon.melomango.ui.theme

import androidx.compose.material3.ColorScheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp

/** Los colores de un tema de Melomango. Cada uno lleva su nombre en CSS para buscarlo en escritorio. */
class MelomangoColors(
${fields}
)

/** Los tokens de diseño, los mismos que \`src/tokens.css\` de escritorio. */
object MelomangoTokens {
    const val VERSION = "${set.version}"

    /** Tema claro: crema y mango. */
    val light = MelomangoColors(
${instance("claro")}
    )

    /** Tema oscuro: hueso tostado. */
    val dark = MelomangoColors(
${instance("oscuro")}
    )

${shared}

    /** Los roles de Material 3, según design/tokens/material3.json. */
${scheme("lightColorScheme", "light")}

${scheme("darkColorScheme", "dark")}
}
`;
}

function swatchValue(t: Resolved): string {
  if (t.type === "color") return `\`${t.value as string}\``;
  if (t.type === "gradient") return (t.value as GradientStop[]).map((s) => `\`${s.color}\``).join(" → ");
  return `\`${cssValue(t)}\``;
}

export function generateDocs(set: TokenSet): string {
  const roles = new Map<string, string[]>();
  for (const [role, name] of Object.entries(set.material3)) roles.set(name, [...(roles.get(name) ?? []), role]);
  const semantic = [...set.modes.claro.values()]
    .map((t) => {
      const kt = t.type === "shadow" ? "—" : `\`${kotlinName(t.name)}\``;
      const m3 = (roles.get(t.name) ?? []).map((r) => `\`${r}\``).join(", ") || "—";
      return `| \`${t.name}\` | \`${cssName(t.name)}\` | ${kt} | ${m3} | ${swatchValue(t)} | ${swatchValue(set.modes.oscuro.get(t.name)!)} | ${t.description} |`;
    })
    .join("\n");
  const shared = [...set.shared.values()]
    .map((t) => `| \`${t.name}\` | \`${cssName(t.name)}\` | \`MelomangoTokens.${kotlinName(t.name)}\` | ${swatchValue(t)} | ${t.description} |`)
    .join("\n");
  const groups = new Map<string, Token[]>();
  for (const t of set.primitives.values()) {
    const group = t.name.split(".")[0]!;
    groups.set(group, [...(groups.get(group) ?? []), t]);
  }
  const primitives = [...groups]
    .map(([group, tokens]) => `| \`${group}\` | ${tokens.map((t) => `${t.name.slice(group.length + 1) || group} \`${t.raw as string}\``).join(" · ")} |`)
    .join("\n");
  const fmt = (n: number) => n.toFixed(2).replace(".", ",");
  const contrastRows = contrastResults(set)
    .map((r) => `| ${r.mode} | \`${r.fg}\` | \`${r.bg}\` | ${fmt(r.ratio)} | ${fmt(r.min)} | ${r.ok ? "sí" : "**no**"} |`)
    .join("\n");
  const distinct = closestPairs(set)
    .map((r) => `| ${r.mode} | ${r.vision} | \`${r.a}\` y \`${r.b}\` | ${r.deltaE.toFixed(1).replace(".", ",")} |`)
    .join("\n");
  return `<!-- ${HEADER(set.version, "").replace(/\n/g, " ").trim()} -->

# Tokens de diseño

La referencia de los tokens de Melomango, versión **${set.version}**: qué hay, cómo se llama cada uno
en CSS, en Kotlin y en Material 3, y sus valores. Este archivo se genera; el porqué está en
[color.md](color.md) y el formato y el flujo de trabajo en [design/tokens/](../../design/tokens/).

## Semánticos

Los que usan las apps. En escritorio, \`var(--color-…)\`; en el móvil, \`MelomangoTokens.light\` o
\`MelomangoTokens.dark\` (o el rol de Material 3 de \`MaterialTheme.colorScheme\`).

| Token | CSS | Kotlin | Material 3 | Claro | Oscuro | Para qué |
|---|---|---|---|---|---|---|
${semantic}

## Comunes a los dos temas

| Token | CSS | Kotlin | Valor | Para qué |
|---|---|---|---|---|
${shared}

## Primitivos

La paleta en bruto, de la que eligen los semánticos. No se usa directamente en las apps.

| Rampa | Pasos |
|---|---|
${primitives}

## Contraste

Lo comprueba \`npm test\` con las parejas de [contraste.json](../../design/tokens/contraste.json): 4,5:1
para texto normal y 3:1 para texto grande, iconos y el foco. Los degradados cuentan con su parada peor.

| Tema | Texto o marca | Sobre | Contraste | Mínimo | Pasa |
|---|---|---|---|---|---|
${contrastRows}

## Colores de datos con daltonismo

La pareja que más se parece de los colores de datos, con visión normal y simulando cada tipo de
daltonismo (Machado 2009). Tiene que superar ΔE ${set.contrast.distinct.minDeltaE}.

| Tema | Visión | Pareja más parecida | ΔE |
|---|---|---|---|
${distinct}
`;
}

/** Everything generated, by path relative to the repo. */
export function generateAll(set: TokenSet = loadTokens()): Record<string, string> {
  return {
    [OUTPUTS.css]: generateCss(set),
    [OUTPUTS.kotlin]: generateKotlin(set),
    [OUTPUTS.docs]: generateDocs(set),
  };
}

const sameText = (a: string, b: string) => a.replace(/\r\n/g, "\n") === b.replace(/\r\n/g, "\n");

function main(args: string[]) {
  const check = args.includes("--check");
  const mobileAt = args.indexOf("--mobile");
  const mobile = mobileAt >= 0 ? args[mobileAt + 1] : undefined;
  if (mobileAt >= 0 && !mobile) throw new Error("--mobile necesita la ruta del repo móvil");
  const files = generateAll();
  const targets = Object.entries(files).map(([rel, text]) => [path.join(ROOT, rel), text] as const);
  if (mobile) targets.push([path.join(path.resolve(mobile), MOBILE_KOTLIN), files[OUTPUTS.kotlin]!]);
  const stale = targets.filter(([file, text]) => !fs.existsSync(file) || !sameText(fs.readFileSync(file, "utf8"), text));
  if (check) {
    if (stale.length) {
      console.error(`Desactualizado (ejecuta npm run tokens):\n${stale.map(([f]) => `  ${f}`).join("\n")}`);
      process.exit(1);
    }
    console.log("Tokens al día.");
    return;
  }
  for (const [file, text] of stale) {
    fs.mkdirSync(path.dirname(file), { recursive: true });
    fs.writeFileSync(file, text);
    console.log(`escrito ${path.relative(process.cwd(), file)}`);
  }
  if (!stale.length) console.log("Nada que cambiar.");
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) main(process.argv.slice(2));

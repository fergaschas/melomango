// Inventory of the design values written in Melomango desktop (App.css, .tsx) and mobile
// (ui/*.kt), with how many times each is used: the base of docs/sistema-diseno/auditoria.md
// (KAN-85). Run it again to see how the migration to tokens is going.
//
//   node scripts/design-inventory.mjs [ruta a melomango-mobile]
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const DESKTOP = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const MOBILE_REPO = path.resolve(process.argv[2] ?? path.join(DESKTOP, "..", "melomango-mobile"));
const MOBILE = path.join(MOBILE_REPO, "app/src/main/kotlin/com/fgascon/melomango/ui");

function walk(dir, ext, out = []) {
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, e.name);
    if (e.isDirectory()) walk(p, ext, out);
    else if (p.endsWith(ext)) out.push(p);
  }
  return out;
}
const count = (map, key, n = 1) => map.set(key, (map.get(key) ?? 0) + n);
const sorted = (map, byKey) =>
  [...map.entries()].sort((a, b) => (byKey ? byKey(a[0], b[0]) : b[1] - a[1] || String(a[0]).localeCompare(String(b[0]))));
const num = (s) => parseFloat(String(s).replace(",", "."));
const table = (title, map, byKey) => {
  const rows = sorted(map, byKey);
  const total = rows.reduce((a, [, n]) => a + n, 0);
  console.log(`\n### ${title} (${rows.length} distintos, ${total} usos)\n`);
  console.log(rows.map(([k, n]) => `${k} ×${n}`).join(" · "));
};

// ---------- Desktop CSS ----------
const css = fs.readFileSync(`${DESKTOP}/src/App.css`, "utf8").replace(/\r\n/g, "\n");
const cssLines = css.split("\n");
// The theme blocks: :root {…}, the dark @media block and :root[data-theme="dark"] {…} (first ~95 lines).
const themeEnd = cssLines.findIndex((l, i) => i > 60 && l.startsWith("* {"));
const body = cssLines.slice(themeEnd).join("\n").replace(/\/\*[\s\S]*?\*\//g, "");
const decls = [...body.matchAll(/([a-z-]+)\s*:\s*([^;{}]+);/g)].map((m) => [m[1], m[2].trim()]);

const cssColors = new Map();
const fontSizes = new Map();
const fontWeights = new Map();
const radii = new Map();
const spacing = new Map();
const shadows = new Map();
const durations = new Map();
const varUse = new Map();
const lineHeights = new Map();
for (const [prop, value] of decls) {
  for (const c of value.match(/#[0-9a-fA-F]{3,8}\b|rgba?\([^)]*\)/g) ?? []) count(cssColors, `${c.toLowerCase()} (${prop})`);
  for (const v of value.match(/var\(--[a-z-]+\)/g) ?? []) count(varUse, v.slice(4, -1));
  if (prop === "font-size") count(fontSizes, value);
  if (prop === "font-weight") count(fontWeights, value);
  if (prop === "line-height") count(lineHeights, value);
  if (prop.endsWith("radius")) count(radii, value);
  if (/^(padding|margin|gap|row-gap|column-gap)(-|$)/.test(prop))
    for (const v of value.split(/\s+/)) if (/^-?[\d.]+(px|rem|em)$/.test(v) && num(v) !== 0) count(spacing, v.replace(/^-/, ""));
  if (prop === "box-shadow") count(shadows, value);
  if (prop === "transition" || prop.startsWith("animation") || prop === "transition-duration")
    for (const d of value.match(/[\d.]+m?s\b/g) ?? []) count(durations, d);
}

console.log("## ESCRITORIO");
console.log(`declaraciones: ${decls.length}, líneas App.css: ${cssLines.length}, bloque de tema: líneas 1-${themeEnd}`);
table("Colores escritos fuera de las variables", cssColors);
table("Tamaños de letra", fontSizes, (a, b) => num(a) - num(b));
table("Pesos", fontWeights);
table("Interlineados", lineHeights);
table("Radios", radii, (a, b) => num(a) - num(b));
table("Espaciados (padding/margin/gap, cada longitud)", spacing, (a, b) => num(a) - num(b));
table("Sombras", shadows);
table("Duraciones", durations, (a, b) => num(a) - num(b));
table("Variables usadas", varUse);

// Icon sizes and inline styles in the .tsx
const tsx = walk(`${DESKTOP}/src`, ".tsx");
const iconSizes = new Map();
const inlineStyles = new Map();
for (const f of tsx) {
  const t = fs.readFileSync(f, "utf8");
  for (const m of t.matchAll(/Icon size=\{(\d+)\}/g)) count(iconSizes, `${m[1]}px`);
  for (const m of t.matchAll(/Icon\s*\/>/g)) count(iconSizes, "20px (por defecto)");
  for (const m of t.matchAll(/style=\{\{([^}]*)\}\}/g)) count(inlineStyles, `${path.basename(f)}: ${m[1].trim()}`);
}
table("Tamaños de icono (.tsx)", iconSizes, (a, b) => num(a) - num(b));
table("Estilos en línea (.tsx)", inlineStyles);

// ---------- Mobile Kotlin ----------
if (!fs.existsSync(MOBILE)) {
  console.log(`
No encuentro el móvil en ${MOBILE_REPO}: pasa su ruta como argumento.`);
  process.exit(0);
}
const kts = walk(MOBILE, ".kt");
const kColors = new Map();
const sp = new Map();
const weights = new Map();
const typo = new Map();
const shapes = new Map();
const dp = new Map();
const kIcon = new Map();
const tweens = new Map();
const elev = new Map();
const colorRefs = new Map();
for (const f of kts) {
  const name = path.relative(MOBILE, f).replace(/\\/g, "/");
  const t = fs.readFileSync(f, "utf8");
  const theme = name.startsWith("theme/");
  if (!theme) {
    for (const m of t.matchAll(/Color\((0x[0-9A-Fa-f]+)\)/g)) count(kColors, `${m[1]} (${name})`);
    for (const m of t.matchAll(/Color\.(White|Black|Red|Gray|Transparent|Unspecified)\b/g)) count(kColors, `Color.${m[1]} (${name})`);
  }
  for (const m of t.matchAll(/([\d.]+)\.sp\b/g)) count(sp, `${m[1]}.sp`);
  for (const m of t.matchAll(/FontWeight\.(\w+)/g)) count(weights, m[1]);
  for (const m of t.matchAll(/MaterialTheme\.typography\.(\w+)/g)) count(typo, m[1]);
  for (const m of t.matchAll(/MaterialTheme\.colorScheme\.(\w+)/g)) count(colorRefs, m[1]);
  for (const m of t.matchAll(/RoundedCornerShape\(([^)]*)\)/g)) count(shapes, `RoundedCornerShape(${m[1]})`);
  for (const m of t.matchAll(/\b(CircleShape|RectangleShape)\b/g)) count(shapes, m[1]);
  for (const m of t.matchAll(/MaterialTheme\.shapes\.(\w+)/g)) count(shapes, `shapes.${m[1]}`);
  for (const m of t.matchAll(/(padding|spacedBy|Spacer|height|width|size|offset)\(([^()]*)\)/g)) {
    for (const d of m[2].match(/[\d.]+\.dp/g) ?? []) count(dp, `${m[1] === "spacedBy" || m[1] === "padding" || m[1] === "offset" ? "espaciado" : m[1] === "Spacer" ? "espaciado" : "tamaño"} ${d}`);
  }
  for (const m of t.matchAll(/Icon\([\s\S]{0,300}?Modifier\.size\(([\d.]+)\.dp\)/g)) count(kIcon, `${m[1]}.dp`);
  for (const m of t.matchAll(/tween\((?:durationMillis\s*=\s*)?(\d+)/g)) count(tweens, `${m[1]} ms`);
  for (const m of t.matchAll(/(tonalElevation|shadowElevation|elevation|shadow)\s*[=(]\s*([\d.]+)\.dp/g)) count(elev, `${m[1]} ${m[2]}.dp`);
}
console.log("\n## MÓVIL");
console.log(`archivos .kt en ui/: ${kts.length}`);
table("Colores fuera de theme/", kColors);
table("Tamaños de letra a mano (.sp)", sp, (a, b) => num(a) - num(b));
table("Estilos de MaterialTheme.typography", typo);
table("FontWeight", weights);
table("Formas", shapes);
table("Espaciados y tamaños (dp)", dp, (a, b) => a.split(" ")[0].localeCompare(b.split(" ")[0]) || num(a.split(" ")[1]) - num(b.split(" ")[1]));
table("Tamaños de icono (Modifier.size en Icon)", kIcon, (a, b) => num(a) - num(b));
table("Duraciones (tween)", tweens);
table("Elevaciones", elev);
table("Colores del tema usados", colorRefs);

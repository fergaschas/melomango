// Builds the signed Windows installers and (with --publish) creates the GitHub release
// that the in-app updater reads through latest.json.
//
// Usage:
//   node scripts/release.mjs                         build + latest.json, nothing published
//   node scripts/release.mjs --publish --notes "…"   also create release v<version> with all assets
//   node scripts/release.mjs --publish --notes-file notas.md
//
// Signing key: $TAURI_SIGNING_PRIVATE_KEY (path or contents) or ~/.tauri/melomango.key.
// Password (if any): $TAURI_SIGNING_PRIVATE_KEY_PASSWORD.
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";

const REPO = "fergaschas/melomango";
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const args = process.argv.slice(2);
const flag = (name) => {
  const i = args.indexOf(name);
  return i >= 0 ? args[i + 1] : undefined;
};
const publish = args.includes("--publish");

function fail(message) {
  console.error(`\n✗ ${message}`);
  process.exit(1);
}

// 1. Versions must agree everywhere.
const conf = JSON.parse(fs.readFileSync(path.join(root, "src-tauri/tauri.conf.json"), "utf8"));
const version = conf.version;
const pkgVersion = JSON.parse(fs.readFileSync(path.join(root, "package.json"), "utf8")).version;
const cargoVersion = fs.readFileSync(path.join(root, "src-tauri/Cargo.toml"), "utf8").match(/^version = "(.+)"/m)?.[1];
if (pkgVersion !== version || cargoVersion !== version) {
  fail(`Versiones distintas: tauri.conf.json ${version}, package.json ${pkgVersion}, Cargo.toml ${cargoVersion}`);
}

const notes = flag("--notes-file")
  ? fs.readFileSync(flag("--notes-file"), "utf8").trim()
  : flag("--notes") ?? `Melomango ${version}`;

// 2. Signing key.
const key = process.env.TAURI_SIGNING_PRIVATE_KEY ?? path.join(os.homedir(), ".tauri", "melomango.key");
if (!process.env.TAURI_SIGNING_PRIVATE_KEY && !fs.existsSync(key)) {
  fail(`No encuentro la clave de firma en ${key}. Define TAURI_SIGNING_PRIVATE_KEY.`);
}

if (publish) {
  const dirty = execFileSync("git", ["status", "--porcelain"], { cwd: root, encoding: "utf8" }).trim();
  if (dirty) fail("Hay cambios sin commitear. Haz commit y push antes de publicar.");
}

// 3. Build signed installers.
console.log(`==> Compilando Melomango ${version} firmado`);
execFileSync("npm", ["run", "tauri", "build"], {
  cwd: root,
  stdio: "inherit",
  shell: true,
  env: {
    ...process.env,
    TAURI_SIGNING_PRIVATE_KEY: key,
    TAURI_SIGNING_PRIVATE_KEY_PASSWORD: process.env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD ?? "",
  },
});

// 4. Collect assets and their signatures.
const bundle = path.join(root, "src-tauri/target/release/bundle");
const asset = (rel) => {
  const file = path.join(bundle, rel);
  if (!fs.existsSync(file)) fail(`Falta ${file}`);
  return file;
};
const nsis = asset(`nsis/Melomango_${version}_x64-setup.exe`);
const msiEs = asset(`msi/Melomango_${version}_x64_es-ES.msi`);
const msiEn = asset(`msi/Melomango_${version}_x64_en-US.msi`);
const sig = (file) => fs.readFileSync(asset(`${path.relative(bundle, file)}.sig`), "utf8").trim();
const url = (file) => `https://github.com/${REPO}/releases/download/v${version}/${path.basename(file)}`;

// 5. latest.json for tauri-plugin-updater (installer-specific keys + generic fallback).
const nsisEntry = { signature: sig(nsis), url: url(nsis) };
const latest = {
  version,
  notes,
  pub_date: new Date().toISOString(),
  platforms: {
    "windows-x86_64": nsisEntry,
    "windows-x86_64-nsis": nsisEntry,
    "windows-x86_64-msi": { signature: sig(msiEs), url: url(msiEs) },
  },
};
const latestPath = path.join(bundle, "latest.json");
fs.writeFileSync(latestPath, JSON.stringify(latest, null, 2) + "\n");
console.log(`==> ${latestPath}`);

if (!publish) {
  console.log("\nListo. Para publicar: node scripts/release.mjs --publish --notes \"…\"");
  process.exit(0);
}

// 6. GitHub release (latest.json is what the app downloads via releases/latest/download/latest.json).
console.log(`==> Publicando v${version} en ${REPO}`);
execFileSync(
  "gh",
  [
    "release", "create", `v${version}`, "--repo", REPO, "--target", "main",
    "--title", `Melomango ${version}`, "--notes", notes,
    nsis, `${nsis}.sig`, msiEs, `${msiEs}.sig`, msiEn, `${msiEn}.sig`, latestPath,
  ],
  { stdio: "inherit" },
);
console.log(`\n✓ Publicado: https://github.com/${REPO}/releases/tag/v${version}`);

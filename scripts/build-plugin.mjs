// Builds a plugin from plugins/<name> and packages it as an installable folder in plugins/dist/<id>.
// Usage: npm run plugin:build -- youtube
import { execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const name = process.argv[2] ?? "youtube";
const dir = path.join(root, "plugins", name);

if (!fs.existsSync(path.join(dir, "plugin.json"))) {
  console.error(`No existe el plugin "${name}" (falta plugins/${name}/plugin.json)`);
  process.exit(1);
}

execFileSync("cargo", ["build", "--release", "--manifest-path", path.join(dir, "Cargo.toml")], {
  stdio: "inherit",
});

const manifest = JSON.parse(fs.readFileSync(path.join(dir, "plugin.json"), "utf8"));
const exe = manifest.executable + (process.platform === "win32" ? ".exe" : "");
const out = path.join(root, "plugins", "dist", manifest.id);

fs.rmSync(out, { recursive: true, force: true });
fs.mkdirSync(out, { recursive: true });
fs.copyFileSync(path.join(dir, "plugin.json"), path.join(out, "plugin.json"));
fs.copyFileSync(path.join(dir, "target", "release", exe), path.join(out, exe));

console.log(`\nPlugin "${manifest.name}" listo en ${out}`);
console.log("Instálalo en Melomango: botón Plugins → Instalar plugin… → elige esa carpeta.");

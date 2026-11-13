// Copies the product name and id from /brand.json (the single source) into the files that
// cannot read it themselves. Run automatically before every desktop dev/build.
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const brand = JSON.parse(readFileSync(join(root, "brand.json"), "utf8"));

const confPath = join(root, "apps/desktop/src-tauri/tauri.conf.json");
const conf = JSON.parse(readFileSync(confPath, "utf8"));
conf.productName = brand.appName;
conf.identifier = brand.appId;
for (const w of conf.app.windows) {
  if (w.label === "main") w.title = brand.appName;
}
const next = JSON.stringify(conf, null, 2) + "\n";
if (next !== readFileSync(confPath, "utf8")) writeFileSync(confPath, next);

const brandTs = `// Generated from /brand.json by scripts/sync-brand.mjs. Do not edit.\nexport const APP_NAME = ${JSON.stringify(brand.appName)};\n`;
const tsPath = join(root, "apps/desktop/src/brand.ts");
let current = "";
try {
  current = readFileSync(tsPath, "utf8");
} catch {}
if (current !== brandTs) writeFileSync(tsPath, brandTs);

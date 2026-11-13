// Builds crates/wasm for the browser and writes the bindings to apps/web/src/wasm.
// Needs: rustup target wasm32-unknown-unknown, and wasm-bindgen-cli 0.2.100.
import { execSync } from "node:child_process";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const target = process.env.CARGO_TARGET_DIR || join(root, "target");
const run = (cmd) => execSync(cmd, { cwd: root, stdio: "inherit" });
run("cargo build --release -p app-wasm --target wasm32-unknown-unknown");
run(
  `wasm-bindgen --target web --out-dir "${join(root, "apps/web/src/wasm")}" "${join(target, "wasm32-unknown-unknown/release/app_wasm.wasm")}"`,
);

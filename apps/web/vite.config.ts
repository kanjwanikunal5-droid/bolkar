import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import react from "@vitejs/plugin-react";
import { defineConfig, type Plugin } from "vite";

const here = dirname(fileURLToPath(import.meta.url));
// The product name and repository live only in /brand.json.
const brand = JSON.parse(readFileSync(join(here, "../../brand.json"), "utf8")) as {
  appName: string;
  repository?: string;
};
const fill = (s: string) =>
  s.replaceAll("%APP_NAME%", brand.appName).replaceAll("%REPOSITORY%", brand.repository ?? "");

function brandPlugin(): Plugin {
  let outDir = "dist";
  return {
    name: "brand",
    configResolved(c) {
      outDir = c.build.outDir;
    },
    transformIndexHtml: (html) => fill(html),
    closeBundle() {
      // public/ files are copied as they are; put the name into the web app manifest.
      const path = join(here, outDir, "manifest.webmanifest");
      try {
        writeFileSync(path, fill(readFileSync(path, "utf8")));
      } catch {
        // not built yet (dev server)
      }
    },
  };
}

// Relative paths so the app works from any address, including GitHub Pages sub-folders.
export default defineConfig({
  base: "./",
  plugins: [react(), brandPlugin()],
  define: { __APP_NAME__: JSON.stringify(brand.appName) },
  build: {
    target: "es2020",
    rollupOptions: {
      input: { main: join(here, "index.html"), download: join(here, "download.html") },
    },
  },
});

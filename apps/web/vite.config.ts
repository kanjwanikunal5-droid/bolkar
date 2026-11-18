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

// Content-Security-Policy for the built app page: only this site's own scripts (plus
// WebAssembly), network calls only to this site and Groq, no plugins, no <base> tricks.
// Not applied to the dev server (its hot-reload uses an inline script).
const CSP = [
  "default-src 'self'",
  "script-src 'self' 'wasm-unsafe-eval'",
  "style-src 'self' 'unsafe-inline'",
  "img-src 'self' data: blob:",
  "media-src 'self' blob:",
  "connect-src 'self' https://api.groq.com",
  "worker-src 'self'",
  "manifest-src 'self'",
  "object-src 'none'",
  "base-uri 'none'",
  "form-action 'none'",
].join("; ");

function cspPlugin(): Plugin {
  return {
    name: "csp",
    apply: "build",
    transformIndexHtml(html, ctx) {
      if (!ctx.filename.endsWith("index.html")) return html;
      return html.replace(
        '<meta charset="UTF-8" />',
        `<meta charset="UTF-8" />\n    <meta http-equiv="Content-Security-Policy" content="${CSP}" />`,
      );
    },
  };
}

// Relative paths so the app works from any address, including GitHub Pages sub-folders.
export default defineConfig({
  base: "./",
  plugins: [react(), brandPlugin(), cspPlugin()],
  define: { __APP_NAME__: JSON.stringify(brand.appName) },
  build: {
    target: "es2020",
    rollupOptions: {
      input: { main: join(here, "index.html"), download: join(here, "download.html") },
    },
  },
});

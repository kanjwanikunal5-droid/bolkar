import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

// Relative paths so the app works from any address, including GitHub Pages sub-folders.
export default defineConfig({
  base: "./",
  plugins: [react()],
  build: { target: "es2020" },
});

import react from "@vitejs/plugin-react";
import { fileURLToPath } from "node:url";
import { defineConfig } from "vite";

const here = (file: string) => fileURLToPath(new URL(file, import.meta.url));

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 1420, strictPort: true },
  build: {
    target: "es2021",
    rollupOptions: {
      input: { main: here("./index.html"), pill: here("./pill.html") },
    },
  },
});

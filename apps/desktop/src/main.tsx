import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import "./styles.css";

async function start() {
  // Development only: sample data when the page is opened in a plain browser (see dev-preview.ts).
  if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) await import("./dev-preview");
  const { App } = await import("./App");
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
}
void start();
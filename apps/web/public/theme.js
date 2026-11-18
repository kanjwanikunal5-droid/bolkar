// Applies the saved theme before the page draws (no flash). A file, not an inline script,
// so the Content-Security-Policy can forbid inline scripts.
try {
  var t = localStorage.getItem("theme") || "system";
  document.documentElement.dataset.theme =
    t === "dark" || (t === "system" && matchMedia("(prefers-color-scheme: dark)").matches) ? "dark" : "light";
} catch (e) {}

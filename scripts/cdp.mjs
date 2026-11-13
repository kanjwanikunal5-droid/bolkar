// Reads (and optionally drives) the app's WebView2 pages through the DevTools protocol.
// Usage: node cdp.mjs <port> <urlSubstring> <javascript expression>
const [port, match, expr] = process.argv.slice(2);
const pages = await (await fetch(`http://127.0.0.1:${port}/json`)).json();
const page = pages.find((p) => p.type === "page" && (match === "main" ? !p.url.includes("pill") : p.url.includes(match)));
if (!page) {
  console.log("NO PAGE matching", match, "found:", pages.map((p) => p.url).join(", "));
  process.exit(1);
}
const ws = new WebSocket(page.webSocketDebuggerUrl);
await new Promise((r) => ws.addEventListener("open", r, { once: true }));
const result = await new Promise((resolve) => {
  ws.addEventListener("message", (m) => {
    const msg = JSON.parse(m.data);
    if (msg.id === 1) resolve(msg.result);
  });
  ws.send(JSON.stringify({ id: 1, method: "Runtime.evaluate", params: { expression: expr, awaitPromise: true, returnByValue: true } }));
});
console.log(page.url);
console.log(typeof result.result.value === "string" ? result.result.value : JSON.stringify(result.result.value ?? result, null, 1));
ws.close();

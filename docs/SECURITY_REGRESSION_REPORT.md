# Security regression report - 0.4.4

Date: 20 Nov 2026. Branch `fix/v0.4.4-audit-remediation`. Findings from the independent audit of
0.4.3 (`docs/audit-0.4.3/`). "Tested" means an automated test ran and passed; anything else is
said plainly.

## 1. Provider address could send the key to another server (audit P0/P1)

**Was:** `set_ai` accepted any address starting with `https://`, `http://localhost` or
`http://127.0.0.1` (string prefix). `http://localhost.attacker.example/v1`,
`http://localhost@attacker.example/v1` and `http://127.0.0.1.attacker.example/v1` passed, and the
key was then sent as `Authorization: Bearer` over plain HTTP. Redirects were followed by the
HTTP client.

**Now** (`apps/desktop/src-tauri/src/providers.rs`, `check_endpoint`, `key_client`):
- the address is parsed as a URL; `https` is allowed for any host (normal certificate checks);
  plain `http` only when the parsed host is exactly `localhost` or a loopback IP
  (`127.0.0.0/8`, `::1`);
- user name/password, query (`?`) and fragment (`#`) are refused; other schemes are refused;
- the check runs in Settings (`set_ai`) **and again before every request that carries the key**
  (chat, speech recognition, model list), so an address stored by an older version is also
  checked;
- the HTTP client for keyed requests never follows redirects (`Policy::none()`), so the key
  cannot be forwarded; a redirect comes back as an error.

| Address | Expected | Result (test `only_https_or_this_computer_may_receive_the_key`) |
|---|---|---|
| `https://api.groq.com/openai/v1` | allowed | allowed |
| `http://localhost:11434/v1`, `http://LOCALHOST:11434/v1` | allowed | allowed |
| `http://127.0.0.1:11434/v1` | allowed | allowed |
| `http://[::1]:11434/v1` | allowed | allowed |
| `http://localhost.attacker.example/v1` | refused | refused |
| `http://localhost@attacker.example/v1` | refused | refused |
| `http://127.0.0.1.attacker.example/v1` | refused | refused |
| `http://192.168.1.10:11434/v1` (LAN) | refused | refused |
| `http://example.com/v1` | refused | refused |
| `https://user:pass@api.groq.com/...` | refused | refused |
| `...?key=x`, `...#x` | refused | refused |
| `ftp://...`, `file:///...`, `localhost:11434/v1`, `not a url`, empty | refused | refused |

Further tests: `a_bad_address_is_refused_before_any_request` (a stored bad address makes no
request at all) and `redirects_are_not_followed_so_the_key_stays_put` (a local server answers 302
to a second local server; the call fails with HTTP 302 and the second server is never contacted).
All passed (`cargo test -p app-desktop --lib providers`, 9 of 9).

A LAN model server (for example Ollama on another PC) is no longer accepted over plain http;
use https or run it on this computer. This is deliberate.

## 2. Correction learning read more than it said (audit P1/P2)

**Was:** the watcher kept only the process id and read whatever text box of that app had the
focus, up to 20,000 characters of a document; COM was initialised every 4 s and never closed.

**Now** (`apps/desktop/src-tauri/src/learn_watch.rs`):
- at insertion the window (HWND) and the focused UI Automation element are pinned; a box that
  cannot be pinned (another window in front, password box, other process) is not watched;
- every look checks that the same window is in front and that the focused element is the
  pinned one (`IUIAutomation::CompareElements`); otherwise watching stops for good;
- only a stretch around the cursor is read (the dictation's length plus 120 characters on each
  side, via the caret range); a one-line box is read only if its whole value is at most 4,000
  characters; nothing read is stored, logged or sent - only the learned word pairs are kept;
- COM is set up once per thread and closed when the thread ends (interfaces released first);
  `RPC_E_CHANGED_MODE` is not closed by us;
- Settings > Writing has "Forget learned words" (deletes every learned entry, keeps the user's own).

Tests: `watching_stops_when_another_window_or_box_has_the_focus`, `com_is_set_up_once_per_thread`,
`uploads_of_one_dictation_are_counted` (includes forgetting learned words). **Not tested by
hand:** reading a real app's text box, switching to Field B in the same app (needs the owner at
the keyboard; doing it from here would show on the owner's screen). Listed in
`RELEASE_CHECKLIST.md`.

## 3. Other items

- **Upload fan-out** is stated in the Groq speech setting and its consent note (twice at the same
  time in English + Hinglish mode, sometimes three times; up to three short text requests after).
  The diagnostics timing line now ends with `uploads: N audio, M text` for each dictation.
- **Keyboard hook** no longer waits for a lock inside the hook (a few quick tries, then the key
  passes through), so Windows cannot drop the hook for being slow.
- **Not done:** local-only switch that blocks all network use, code signing, penetration test.
  No certification is claimed (no SOC 2, ISO 27001 or HIPAA).

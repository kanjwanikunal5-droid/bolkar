# Status

Last updated: **13 November 2026**. Stages 0, 1 and 2 are built. Several of their acceptance
tests need the owner at the keyboard (or his API key) and are listed below as **not run**.

Machine for every measurement: HP Victus 15, Ryzen 7 7445HS, 15.2 GB RAM, RTX 4050 Laptop GPU
(Vulkan), Windows 11 build 26300, Rust 1.99.1.

## Automated tests (run 13 Nov 2026)

| Command | Result |
|---|---|
| `cargo test` on app-core, app-bench, app-asr, app-desktop | **107 passed, 0 failed** |
| `cargo clippy --workspace --all-targets -- -D warnings` | Clean |
| `cargo fmt --all -- --check` | Clean |
| `pnpm --filter desktop build` (TypeScript type-check + bundle) | Clean |
| Core builds for wasm32 and both Android targets | Builds (Stage 0) |

What the 107 cover:
- clean-up rules and the golden fixtures;
- AI-answer validation and learning from fixes;
- WER/CER scoring and the speech hallucination guard;
- audio helpers and the keyboard state machine;
- the real Windows clipboard: private write, restore, and an image round trip;
- Windows Credential Manager: save, read and delete;
- the AI client against a fake local server: success, time-out, rate limit, bad key, bad answer;
- the database: migrations, history, dictionary, snippets, consent, upload log;
- model checksums and per-app paste rules.

### Golden clean-up fixtures: 160 of 166 pass (96.4%)

`spec/fixtures/cleanup_core.json` (32) + `cleanup_extended.json` (134). The five must-pass
examples from the build prompt all pass. About half of the cases are sentences that must
**not** change. The 6 known failures, kept as failures on purpose:

| Case | Input | Rules give | Why |
|---|---|---|---|
| x-neg-045 | "Hmm is a sound, not a word." | "Is a sound, not a word." | "Hmm" used as a noun is taken for a hesitation |
| x-fl-014 | "That's fine, I mean, we can wait." | "That's fine we can wait." | Which comma to keep is ambiguous |
| x-fl-017 | "We should, um, maybe, postpone it." | "We should maybe, postpone it." | Same |
| x-hg-002 | "Report, matlab, ready hai." | "Report, ready hai." | Same |
| x-cm-005 | "Line one new line line two" | unchanged | "one new line" reads as a noun phrase |
| x-lit-002 | "first, a. second, b." | "First, a. second, b." | "a." looks like an abbreviation |

The AI clean-up path is meant to handle these; that is measured with `ai-eval` (below).

## Measured speed: local speech recognition

Test clip: 26.7 s of synthetic (text-to-speech) English, so the numbers show **speed only, not
accuracy**. `rtf` tool, median of 3 runs.

| Model | Graphics card (Vulkan) | Processor only |
|---|---|---|
| base (57 MB) | 1.6 s (RTF 0.06) | 29.1 s (RTF 1.09) |
| small (181 MB) | 3.1 s (RTF 0.11) | 110.9 s (RTF 4.15) |
| large-v3-turbo (547 MB) | 4.6 s (RTF 0.17) | stopped after 10 min |

Inside the app, the first dictation after start-up took 8.5 s on large-v3-turbo with the
graphics card, because the card's programs are prepared on first use. On processor only, this
laptop is too slow for anything but the base model. The GPU build is therefore the default.

## Stage 1 — Windows dictation

| Acceptance check | State | Evidence |
|---|---|---|
| Recording -> local Whisper -> clean-up, inside the real app | **Passed** | App self-test on the test clip: correct raw text, "um" removed, 15% -> 12% correction applied, list formatted |
| Paste key sent, clipboard restored afterwards | **Passed** | App self-test: outcome "Pasted", clipboard marker text restored |
| Dictation kept out of Win+V history | **Passed (marker check)** | Unit test confirms the exclusion format is on the clipboard. Opening Win+V by hand: **not run** |
| Refuses to paste when focus moved | **Passed** | Self-test run where focus changed: outcome "Copied - focus changed" |
| Text actually appears in a target app | **Not run** | The unattended test window could not stay in front: Windows' focus rules, plus the Claude app taking focus every few seconds. Needs the owner |
| Scenario 1: Gmail in Chrome | **Not run** | Needs the owner's voice and keyboard |
| Notepad, Word, VS Code, Windows Terminal | **Not run** | Needs the owner |
| Elevated Notepad shows "copied" fallback | **Not run** | Code path exists (`target::is_elevated`); not exercised by hand |
| Start menu does not open after Ctrl+Win | **Not run** | Masking key implemented; needs a person pressing the keys |
| Long pauses do not end push-to-talk | **Passed (logic)** | State-machine test: 60 s hold, release ends it. Real hold: not run |
| 50 dictations without crash or duplicate | **Not run** | Needs the owner |

## Stage 2 — clean-up, dictionary, snippets, AI providers

| Acceptance check | State | Evidence |
|---|---|---|
| >= 95% of golden fixtures pass with rules | **Passed** | 96.4% (160/166) |
| >= 150 fixtures including backtrack, lists, Hinglish | **Passed** | 166 |
| AI answer never adds a number, name, e-mail or link | **Passed (automated check)** | `llm::accept` rejects these; 8 unit tests including invented time, name and e-mail |
| AI provider offline or slow -> rules within 2.5 s | **Passed (fake server)** | Time-out test: a slow server is abandoned at the limit. Real provider: not run |
| AI path passes >= 90% of fixtures | **Not run** | Needs the owner's Groq key saved in the app; then run `ai-eval` |
| Finance + Tech packs on by default | **Passed** | Fresh install: 35 terms loaded; Excel, React, Claude, Cursor are hint-only |
| Dictionary feeds the recogniser | **Passed** | App run with the packs as Whisper prompt: same correct transcript, nothing invented |
| Keys only in Windows Credential Manager | **Passed** | Unit test round trip; the key is never written to the database |
| Every upload logged locally | **Passed** | Code logs each request; fresh install shows 0 uploads |
| Fix popup (Alt+Shift+F) and learning from fixes | **Built**; manual use **not run** | `learn::suggest` tests pass |
| Styles per app | **Built** | App category from the window decides the style sent to the AI |

Not built in Stage 2 (moved later): Command Mode (Stage 7), import/export of dictionary and
snippets (Stage 5), per-app rules editable in the UI.

## Measured accuracy per language

**None yet.** No real recordings have been scored. No accuracy figure is shown anywhere in the app.

## Costs so far

₹0. All tools are free; Groq is used on its free tier with the owner's own key.

# Architecture

Decision record: [adr/0001-stack.md](adr/0001-stack.md). Items marked *(planned)* do not exist yet.

## Layout

```
brand.json            product name, the single source
crates/core           shared Rust core (exists)
bench                 command-line tools: wer, probe, clean (exists)
spec                  prompts, schemas, golden fixtures (exists)
apps/desktop          Tauri 2 Windows app (planned, Stage 1)
apps/web              React PWA (planned, Stage 4)
apps/android          Kotlin keyboard (planned, Stage 5)
packages/ui           shared React components and design tokens (planned)
```

## The core (`crates/core`)

Deterministic and offline. It builds for Windows, `wasm32-unknown-unknown`,
`aarch64-linux-android` and `armv7-linux-androideabi` (checked in CI).

| Module | Job |
|---|---|
| `pipeline` | The dictation text pipeline (below) |
| `dictionary` | Write terms the way the user spells them |
| `snippets` | Spoken trigger -> saved block of text |
| `metrics` | Word / character error rate with shared normalisation |
| `hardware` | First-guess speech model for a machine |
| `brand` | Product name, generated from `brand.json` |

### Text pipeline

```
raw transcript
 -> normalise        Unicode NFC, spaces, ellipsis
 -> snippets         expanded bodies are parked behind placeholders so no later rule alters them
 -> dictionary
 -> spoken commands  "new line", "full stop", "bullet point" (can be switched off)
 -> LIGHT only:
      backtracking   "2 PM... actually make that 3 PM" -> "3 PM"
      fillers        um, uh; "you know" / "like" / "matlab" only when clearly a filler
      stutters       "I I think", "th- the"
      lists          "First, ... Second, ..." -> numbered list
 -> tidy             spacing and capitals; never lower-cases, never adds a full stop
 -> restore snippet bodies
```

Design rules:

1. **Never invent.** No step adds a word that was not spoken.
2. **When unsure, do nothing.** A doubtful self-correction keeps both parts and is reported in
   `uncertain_spans`.
3. **Every rule has a negative test** — a sentence that looks similar and must not change.
4. LITERAL level skips the LIGHT steps. PROFESSIONAL needs an LLM for grammar; without one it
   equals LIGHT.

The rules are documented in `spec/prompts/backtrack_rules.md` and pinned by
`spec/fixtures/cleanup_core.json`.

## Planned pieces

- **Desktop:** low-level keyboard hook for push-to-talk, WASAPI microphone and loopback capture,
  non-activating overlay pill, clipboard insertion excluded from clipboard history, per-app
  paste key, SQLite history.
- **Speech engines** behind one `AsrEngine` trait: whisper.cpp (Vulkan or processor), Parakeet
  (ONNX), IndicConformer (ONNX), and optional cloud providers with the user's own key.
- **LLM providers** behind one `LlmProvider` trait, with the deterministic pipeline as the
  always-available fallback.
- **Privacy boundary:** audio and text stay on the device unless a provider is enabled and its
  consent screen accepted. Every upload is logged locally.

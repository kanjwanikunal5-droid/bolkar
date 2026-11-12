# Status

Last updated: **12 November 2026**. Current stage: **0 — Foundations (not yet complete)**.

There is no installable app yet. What exists is the shared text-cleanup core, the measuring
tools, and the project scaffolding.

## Stage 0 checklist

| Item | State | Evidence |
|---|---|---|
| Hardware probe -> model recommendation | **Done** | `probe` run on the owner's laptop, output below |
| Benchmark harness (WER/CER, shared normalisation, Indic NFC) | **Done** | 10 scoring tests pass |
| 20 golden cleanup fixtures | **Done (30)** | 30/30 pass, including the 5 must-pass examples |
| `docs/RESEARCH_DELTA.md` | **Done**, 4 items still open | See that file |
| CI workflow written | **Done** | `.github/workflows/ci.yml` |
| Public GitHub repo, CI green | **Not done** | Waiting for GitHub login on this laptop |
| Local speech speed (real-time factor per model) | **Not run** | No speech engine is integrated yet; moves to the start of Stage 1 |
| Local LLM latency | **Not run** | Ollama is not installed |
| Groq latency | **Not run** | Needs the owner's own Groq API key |
| TypeScript lint/tests in CI | **Not applicable yet** | No TypeScript code until Stage 4 |
| Android unit tests in CI | **Not applicable yet** | No Android code until Stage 5 |

Stage 0 is **not** marked complete because CI has not run on GitHub and no speed numbers exist.

## What was run on 12 November 2026 (owner's laptop, Rust 1.99.1)

| Command | Result |
|---|---|
| `cargo test --workspace` | **57 passed, 0 failed** (50 core unit, 3 golden/property, 3 bench library, 1 probe) |
| `cargo clippy --workspace --all-targets -- -D warnings` | Clean |
| `cargo fmt --all -- --check` | Clean |
| `cargo check -p app-core --target wasm32-unknown-unknown` | Builds |
| `cargo check -p app-core --target aarch64-linux-android` | Builds |
| `cargo check -p app-core --target armv7-linux-androideabi` | Builds |

### Golden fixtures: 30 of 30 pass (100%)

| Category | Cases |
|---|---|
| Self-correction (backtracking) | 7 |
| Self-correction, must NOT change | 3 |
| Fillers | 4 |
| Fillers, must NOT change | 1 |
| Stutters | 1 |
| Lists | 2 (+1 must NOT change) |
| Spoken commands | 2 (+1 must NOT change) |
| Dictionary, snippet | 1 each |
| Hinglish | 2 (+1 must NOT change) |
| LITERAL level, plain negatives | 3 |

The five examples named in the build prompt all pass:

- "Let's meet at 2 PM... actually make that 3 PM" -> "Let's meet at 3 PM."
- "Schedule the meeting for Monday at 3 PM. Actually, make that 4 PM." -> "...Monday at 4 PM."
- "Revenue increased by 15 percent. Sorry, it increased by 12 percent." -> "Revenue increased by 12 percent."
- "So, um, I think, you know, we should probably review the budget again." -> "So, I think we should probably review the budget again."
- "I have two points to make" -> unchanged

### Hardware probe output

```
OS:        Windows 11 Home Single Language
Processor: AMD Ryzen 7 7445HS w/ Radeon 740M Graphics (6 cores / 12 threads)
Memory:    15.2 GB
Free disk: 241.1 GB (D:\bolkar)
Graphics:  AMD Radeon 740M Graphics (integrated, Vulkan)
Graphics:  NVIDIA GeForce RTX 4050 Laptop GPU (dedicated, Vulkan)

Suggested model: LargeV3Turbo on Vulkan
```

This is a first guess from the hardware. It has not been confirmed by a speed measurement.

## Measured accuracy per language

**None yet.** No speech engine is integrated and no recordings have been scored. No accuracy
figure for this app exists, and none will be shown in the UI until one is measured.

## Known limits of the deterministic cleanup

These are by design: when a rule is unsure it leaves the text alone.

- It does not add punctuation the recogniser did not produce (apart from the full stop that
  ends a corrected sentence).
- Self-corrections are applied only when a value is replaced by a value of the same kind
  (time, percentage, money, date, day, number) or the sentence is clearly restated. Corrections
  of names or phrases ("send it to Rahul, sorry, Priya") are kept as spoken.
- Spelled-out times ("three thirty") are not recognised as times.
- The word "period" is a command only when the recogniser already set it apart; "full stop"
  always works.
- Removing a hesitation between two commas can drop a comma a careful writer would keep
  ("If it rains, um, we cancel" -> "If it rains we cancel").
- Only one numbered list per dictation is formatted.
- Stutter removal covers common English function words only, so Hindi reduplication
  ("dheere dheere") is never damaged.
- PROFESSIONAL level equals LIGHT until an LLM provider exists (Stage 2).

## Costs so far

₹0 for tools and services. The only cost is the builder's own Claude usage.

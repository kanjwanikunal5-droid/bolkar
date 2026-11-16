# Speak Flow (formerly Bolkar)

Free voice dictation and AI meeting notes that run on your own device.

- **Windows desktop app** — hold a shortcut, speak, and clean text is typed into any app.
- **Android keyboard** — a voice keyboard that works in every app.
- **Web app** — dictation-to-clipboard and meeting notes in any browser (this is the iPhone route).

Local by default: nothing leaves the device unless you switch on a cloud provider with your own
free API key. No telemetry. No certifications are claimed.

> **Status: early development.** Stages 0-2 are built: the Windows dictation app with local
> speech recognition, clean-up, dictionary, snippets and optional AI clean-up. There is no
> installer yet. See [docs/STATUS.md](docs/STATUS.md) for exactly what was tested and what was not.

## What exists today

| Part | State |
|---|---|
| `crates/core` — clean-up pipeline, dictionary, snippets, AI-answer checks, learning from fixes | Working, tested |
| `crates/asr` — local Whisper (graphics card or processor), silence-hallucination guard | Working, tested |
| `apps/desktop` — Windows app: hold Ctrl+Win to dictate into any app | Built; manual acceptance tests pending |
| `bench` — accuracy scorer (WER/CER), hardware probe, pipeline runner | Working, tested |
| Meeting notes, Android keyboard, web app | Not started |

## Try it (developers)

Needs Rust (stable). On Windows also the Visual Studio C++ Build Tools.

```bash
cargo test --workspace
```

```bash
cargo run -p app-bench --bin clean -- "Let's meet at 2 PM... actually make that 3 PM"
```

```bash
cargo run -p app-bench --bin probe
```

## Honest limits

- No speech recogniser is error-free. The goal is the lowest *measured* error rate plus easy fixing.
- A free, link-shareable native iPhone app is not possible; iPhone uses the web app.
- Android cannot record phone, Zoom, Meet or WhatsApp call audio. Phone meeting notes are microphone-only.
- This project holds no SOC 2, ISO 27001 or HIPAA certification and never will claim one.

## Documents

- [docs/STATUS.md](docs/STATUS.md) — what works, measured numbers
- [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) — how the pieces fit
- [docs/DEVELOPMENT_PLAN.md](docs/DEVELOPMENT_PLAN.md) — the stages
- [docs/TESTING_PLAN.md](docs/TESTING_PLAN.md) — what is tested and how
- [docs/RESEARCH_DELTA.md](docs/RESEARCH_DELTA.md) — facts verified during the build
- [docs/PRIVACY.md](docs/PRIVACY.md) and [docs/SECURITY_CONTROLS.md](docs/SECURITY_CONTROLS.md)

## Licence

MIT. See [LICENSE](LICENSE) and [NOTICE](NOTICE). Not affiliated with any commercial dictation product.

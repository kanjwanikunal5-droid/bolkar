# Conventions for future sessions

Read `docs/STATUS.md` first: it says which stage is current and what is proven.

## Rules that must not be broken

- **Honesty.** Never claim something works unless a test or manual check actually ran. Record
  what was run in `docs/STATUS.md`. Unrun acceptance tests are listed as "not run" with the reason.
- No fake features, confidence scores, benchmark numbers, speaker names or certifications.
  SOC 2 / ISO 27001 / HIPAA must never appear as claims.
- Local by default. No API keys in code, config, logs or commits. Users bring their own keys.
- Cleanup never invents content. When a rule is unsure it leaves the text alone.
- Raw transcript and cleaned text are stored separately.
- Ask the owner before anything that costs money, installs system tools, or cannot be undone.
- Never disable Defender or the firewall. Never touch the system Python.
- Do not copy Wispr Flow code, names, icons, colours or text.

## Layout

- `brand.json` — the ONE place the product name lives. `crates/core/build.rs` turns it into
  `app_core::brand::APP_NAME`. Crates are named `app-*` so a rename touches one file.
- `crates/core` — deterministic, offline, portable (must build for Windows, Android, wasm32).
  No network, audio or UI code here.
- `bench` — command-line tools: `wer`, `probe`, `clean`.
- `spec/fixtures/*.json` — golden cleanup cases. Any port (Kotlin, TS) must pass the same files.
- `spec/prompts`, `spec/schemas` — versioned LLM prompts and output schemas.

## This laptop

- Rust lives in `D:\dev\rust` (`RUSTUP_HOME`, `CARGO_HOME`) because the Windows user name
  contains a space. If `cargo` is not found in a fresh shell, add `D:\dev\rust\cargo\bin` to PATH.
- The shell is Windows PowerShell 5.1: scripts must be ASCII-only and must not use `&&`.

## Before every commit

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Working style

- Work in stages (`docs/DEVELOPMENT_PLAN.md`). After each: tests, update STATUS.md, commit, push,
  short plain-English note to the owner.
- New cleanup behaviour needs a golden fixture, including a negative case (text that must NOT change).
- The owner sits far from the screen: default UI scale 125%, large text, WCAG AA contrast.

# ADR 0001 — Technology stack

- **Status:** accepted (12 November 2026)
- **Source:** research report section C3, master build prompt section 3

## Decision

One public monorepo (Cargo workspace + pnpm workspace):

| Part | Choice |
|---|---|
| Shared logic | Rust crate `crates/core` |
| Windows desktop | Tauri 2 (Rust backend, React + TypeScript UI) |
| Android | Native Kotlin keyboard (InputMethodService); core via UniFFI |
| Web / iPhone | React PWA; core via wasm |
| Local speech | whisper.cpp, Parakeet (ONNX), IndicConformer (ONNX) |
| Storage | SQLite (desktop, Android), IndexedDB (web) |

## Why

- Tauri gives a small installer and direct Win32 access (keyboard hook, SendInput, WASAPI).
  The open-source app Handy shows the same stack working for push-to-talk dictation.
- A Rust core lets Windows, Android and web run the *same* cleanup rules against the same
  golden fixtures.
- A Python/PySide6 desktop app was rejected: large installers, no reuse on mobile.

## Deviations and additions made during Stage 0

1. **Crates are named `app-core` / `app-bench`,** not after the product, so renaming the product
   means editing `brand.json` only.
2. **The benchmark scorer is Rust (`bench/src/bin/wer.rs`), not Python.** It shares the
   normalisation code with the app and runs in CI without Python.
3. **IndicConformer will run through onnxruntime by default.** No official sherpa-onnx build of
   IndicConformer was found (see `docs/RESEARCH_DELTA.md`). sherpa-onnx stays an option.
4. **Rust toolchain on `D:\dev\rust` on the owner's laptop,** because the Windows user name has
   a space in it.

## Open risks to settle in Stage 1

- whisper.cpp with Vulkan needs the Vulkan SDK at build time and must fall back to the processor
  at run time when no GPU is present.
- A reported C-runtime linking conflict between `whisper-rs-sys` and `tokenizers` on Windows.
  Check Handy's current build configuration before choosing crates.

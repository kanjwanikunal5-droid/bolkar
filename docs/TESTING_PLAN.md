# Testing plan

Legend: **auto** = runs in `cargo test` / CI. **manual** = done by hand on the owner's laptop or
phone and logged here with the date. Results live in [STATUS.md](STATUS.md).

## Automated today

| Area | Where | What it proves |
|---|---|---|
| Golden cleanup fixtures | `crates/core/tests/golden_fixtures.rs` reading `spec/fixtures/*.json` | Each case gives exactly the expected text; must-pass examples can never fail; pass rate >= 95% |
| Clean text is untouched | same file | Already-correct English, Hindi and Tamil sentences come out identical |
| Idempotence | same file | Running the pipeline on its own output changes nothing |
| Backtracking | `pipeline/backtrack.rs` tests | Corrections applied; doubtful ones keep both parts |
| Fillers / stutters | `pipeline/fillers.rs` tests | Fillers removed; meaningful "like", "5 mm", Hindi reduplication kept |
| Spoken commands | `pipeline/commands.rs` tests | Commands applied; ordinary sentences untouched |
| Lists | `pipeline/lists.rs` tests | Enumerations formatted; prose untouched |
| Scoring | `metrics.rs`, `bench/src/lib.rs` tests | WER/CER arithmetic, pooled corpus rate, Indic NFC |
| Hardware advice | `hardware.rs`, `probe.rs` tests | Model choice per machine class; Vulkan summary parsing |
| Portability | CI job `core-portability` | Core compiles for wasm32 and both Android targets |
| Secrets | CI job `secret-scan` (gitleaks) | No keys committed |

## Accuracy benchmarks (Stage 0 harness, Stage 6 results)

- Tool: `cargo run -p app-bench --bin wer -- --ref refs.tsv --hyp hyps.tsv --json out.json`
- Both sides are normalised identically (see `metrics::normalise_for_scoring`).
- The corpus rate is pooled: total errors / total reference words.
- Data: the owner's 30 recordings plus public sets (`bench/datasets.md`).
- Synthetic (TTS) audio may be used to test plumbing only, never as accuracy evidence.

## Manual checklist (not started — needs the desktop app)

- [ ] Hotkey: press/release, double-tap toggle, conflict detection, Start-menu suppression, injected-event ignore
- [ ] Insertion: Notepad, Word, Chrome (Gmail, Docs, WhatsApp Web, ChatGPT, Claude), VS Code, Windows Terminal
- [ ] Elevated-window fallback, focus-change abort, Win+V exclusion, clipboard restore including an image
- [ ] Audio: pre-roll, quiet speech, long pauses, 20-minute hands-free cap, Bluetooth microphone
- [ ] 30 s of silence gives empty output (no hallucinated text)
- [ ] Meeting: mic only, system only, both, 60+ minutes, pause/resume, device change, crash recovery, export formatting
- [ ] Languages: WER/CER table for >= 13 languages from real recordings
- [ ] Providers: offline fallback, HTTP 429 handling, invalid-key message
- [ ] Android: insertion in 5 apps, offline mode, permission flows, APK install from link
- [ ] Web: Chrome and iPhone Safari dictation + copy; tab-audio capture in Chrome
- [ ] Accessibility: 200% scale, keyboard-only navigation, screen-reader labels, contrast
- [ ] Packaging: per-user install, clean uninstall, updater

## Manual checks already done

| Date | Check | Result |
|---|---|---|
| 12 Nov 2026 | Shortcut availability probe on the owner's laptop | See RESEARCH_DELTA.md section 2 |
| 12 Nov 2026 | `probe` tool output compared with Windows' own hardware report | Matches (processor, memory, both graphics chips) |

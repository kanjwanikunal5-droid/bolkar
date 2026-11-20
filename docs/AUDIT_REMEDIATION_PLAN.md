# Audit remediation plan - Speak Flow 0.4.4

Source: the independent audit of 0.4.3 (`docs/audit-0.4.3/INDEPENDENT_AUDIT_0.4.3.md`, score
6.2/10) and its remediation prompt (`docs/audit-0.4.3/REMEDIATION_PROMPT_0.4.4.md`). Work is on
branch `fix/v0.4.4-audit-remediation` from tag `v0.4.3` (commit 98c4751). Every finding was
checked against the code before changing it. Status is kept current in
`docs/V0.4.4_TEST_REPORT.md`; nothing is marked done without a test that ran.

| # | Finding (audit section) | Files | Change | Test / evidence |
|---|---|---|---|---|
| 1 | P0 Custom provider address checked with `starts_with` ("http://localhost.evil" passed); key could follow a redirect | `providers.rs`, `lib.rs` (`set_ai`) | `check_endpoint`: parsed URL; https any host; http only for host `localhost` or a loopback IP; no user info, query or fragment. Checked again before every request that carries the key; redirects never followed | Unit tests: every address in the prompt; refused address makes no request; redirect target never contacted |
| 2 | P0 Meeting transcript: one sentence 5 times, 3 sentences missing | `controller.rs` (`transcribe_meeting`, `own_words_hint`), `crates/asr/src/guard.rs` | Root cause measured: the 400-character dictionary hint (346 pack words since 0.4.3) makes Whisper loop. Hints now use only the user's own and learned words (200 chars meetings, 240 dictation). A piece that still loops is recognised again without the hint; 3+ identical lines in a row are collapsed (2 kept: people repeat themselves) and logged | A/B on the same WAV (with/without dictionary); guard unit test; release-build meeting re-run against the 13-turn reference with WER |
| 3 | P0 Decision "Confirmed" and "check this" at once | `crates/core/src/summary.rs` (`validate`) | If a detail is missing, the 1-2 lines before the cited ones are tried as evidence (the condition is usually there); if still unsupported, the decision is shown as proposed. Never both | Unit test from the real 50,000 / ROI above 3x case plus a wrong-amount case |
| 4 | Participants listed as "System audio" | `summary.rs` (`validate`, `to_markdown`) | Audio sources removed from participants; notes say "Participants: not identified" and list "Audio sources" separately | Unit test |
| 5 | P1/P2 Correction learning reads any focused box of the same app; 20,000 characters; COM not balanced | `learn_watch.rs`, `controller.rs` | Pin the window and the UI Automation element (RuntimeId) at insertion; stop when either changes; read at most a small window around the dictation; COM initialised once per thread with a guard; off for apps without a stable element | Unit tests for the matching logic; real-app check needs the owner (listed as not run if not done) |
| 6 | P1 Layout check lets meaning-carrying words go (we, will, is, hai ...) | `crates/core/src/layout.rs` | Only list scaffolding may be left out (and, also, then, next, finally, ordinals replaced by numbers, "bullet/point"); every other word must stay, with no share threshold | Adversarial tests for each protected word; ordinal-to-number test |
| 7 | Two bullet styles ("- " and "•") | `pipeline/structure.rs`, `layout.rs` | One bullet character everywhere | Fixture/unit tests |
| 8 | Settings says 0.3 s pre-roll, code keeps 1 s | `SettingsPage.tsx` | Text corrected to "up to 1 second, in memory only" | Screenshot |
| 9 | "Any non-zero sample = sound" | `audio.rs` | Level above a small noise floor, measured per block | Unit test |
| 10 | Live meeting transcript waits 25 s | `Meetings.tsx` | Labelled honestly as "updates about every 30 seconds" (chunk size not changed: accuracy first) | Screenshot |
| 11 | Hotkey hook takes a blocking lock | `hotkey/mod.rs` | `try_lock`; if busy the key passes through | Unit/existing tests |
| 12 | Android recorder released while the reader may still run | `Recorder.kt` | Release only after the reader thread has ended; otherwise report an error | Android CI build; device test needs the owner |
| 13 | Benchmarks not re-run on the release route | `bench`, scripts | All 30 English + 72 Hinglish clips through the release build, per-clip CSV | `ACCURACY_ENGLISH.csv`, `ACCURACY_HINGLISH.csv` |
| 14 | Upload fan-out not shown at consent | `SettingsPage.tsx`, `PRIVACY.md` | Consent text lists how many requests a dictation can make | Screenshot |

Not in 0.4.4 (stated, not hidden): global local-only switch, speaker recognition, code signing,
smaller live-transcript chunks, the hands-on tests on the owner's Windows profile (100 cold/warm
mic starts, other apps, devices), real-voice accuracy with corrected transcripts.

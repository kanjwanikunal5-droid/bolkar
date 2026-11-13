# Status

Last updated: **13 November 2026**. Stages 0, 1 and 2 are built, and GitHub CI passed on all
of them. An installer exists and was installed and checked on the owner's laptop. Several of their acceptance
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

## Stage 1 â€” Windows dictation

| Acceptance check | State | Evidence |
|---|---|---|
| Recording -> local Whisper -> clean-up, inside the real app | **Passed** | App self-test on the test clip: correct raw text, "um" removed, 15% -> 12% correction applied, list formatted |
| Paste key sent, clipboard restored afterwards | **Passed** | App self-test: outcome "Pasted", clipboard marker text restored |
| Dictation kept out of Win+V history | **Passed (marker check)** | Unit test confirms the exclusion format is on the clipboard. Opening Win+V by hand: **not run** |
| Refuses to paste when focus moved | **Passed** | Self-test run where focus changed: outcome "Copied - focus changed" |
| Text actually appears in a target app | **Passed (Notepad)** | 13 Nov: `scripts/selftest-notepad.ps1` with the real app: test clip -> text pasted into Notepad, read back through UI Automation, identical to the cleaned text; clipboard restored. A Chrome compose-box run was inconclusive (the page's text box did not have keyboard focus) and was stopped because the owner was using Chrome |
| App window shows its screens | **Fixed and passed** | Bug found 13 Nov: the built app loaded `localhost:1420` (a development address) and showed nothing. Fixed by embedding the screens; every page then checked in the installed app |
| Installer | **Passed** | `Bolkar_0.1.0_x64-setup.exe` (9.2 MB): installs for the current user without admin, adds a Start-menu entry and an uninstall entry; the installed app starts and shows every page |
| First word not cut off | **Fixed** | Measured 13 Nov: the microphone takes 579-1846 ms to start. "Keep the microphone ready" (300 ms pre-roll held in memory) is now on by default; the installed app reports the microphone open with live levels |
| Scenario 1: Gmail in Chrome | **Not run** | Needs the owner's voice and keyboard |
| Notepad, Word, VS Code, Windows Terminal | **Not run** | Needs the owner |
| Elevated Notepad shows "copied" fallback | **Not run** | Code path exists (`target::is_elevated`); not exercised by hand |
| Start menu does not open after Ctrl+Win | **Not run** | Masking key implemented; needs a person pressing the keys |
| Long pauses do not end push-to-talk | **Passed (logic)** | State-machine test: 60 s hold, release ends it. Real hold: not run |
| 50 dictations without crash or duplicate | **Not run** | Needs the owner |

## Stage 2 â€” clean-up, dictionary, snippets, AI providers

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

## Update 13 November 2026 (evening): speed, accuracy, pill, meeting notes

### Speed - found and fixed
The speech engine had been running on the built-in AMD Radeon 740M (Vulkan lists it first),
not on the RTX 4050. The app now picks the dedicated card. Measured with large-v3-turbo:

| Clip | Before (built-in chip) | Now (RTX 4050), installed app |
|---|---|---|
| 4 s | 2.4 s | 0.40 s |
| 8 s | 2.4 s | 0.45 s |
| 14 s | 2.6 s | 0.52 s |
| 27 s | 3.8 s | 0.73 s |

Also: model loaded and warmed up at start-up, one reusable GPU state, flash attention, and
Windows "efficiency mode" switched off for the app. A shorter audio window was tried and
**rejected**: 2-4x faster on short clips but it repeated a sentence on a 14 s clip.
The first dictation right after installing a new version took 6.6 s once (the graphics driver
prepares its programs; cached afterwards).

### Accuracy
- The recogniser hint was a bare word list; Whisper copied its style and dropped full stops
  ("2.5 crore rupees Please update"). It is now a punctuated "Glossary: ..." sentence; the
  same clip comes out correctly punctuated.
- AI clean-up never ran before today: Groq had retired `llama-3.1-8b-instant`, so every
  request failed and the rules were used (the safety fallback worked as designed). Default is
  now `qwen/qwen3.8-27b`, and the app picks a current model by itself if one is retired.
- New check: an AI answer that drops a word the rules kept (including a repeated word) is
  rejected, except inside a real self-correction. List numbers are no longer "invented numbers".
- **AI path, all 163 eligible golden fixtures, qwen/qwen3.8-27b: 154 final texts correct
  (94.5%)**, 21 AI answers rejected by the checks (rules used), median 0.30 s, 90% under
  0.51 s. Acceptance target was 90%: **passed**.
- Groq cloud speech recognition is available as an option with its own audio consent (off).
  Measured 0.26-0.41 s on the test clips; on this laptop the local GPU is now as fast.

### Pill
Redesigned as [x] cancel - live waveform - [check] finish, with clickable buttons. Checked in
the installed app: both buttons and 9 animated bars render. Clicking the buttons with a real
mouse during a real dictation: **not run** (needs the owner).

### Stage 3 - meeting notes (first version)
Built: recording of microphone ("Me") and computer sound ("Others", WASAPI loopback) as two
WAV tracks saved every 5 s, consent required, live levels, My Thoughts, transcription on
this computer (split at quiet points), notes from the AI with the evidence check, plain-text
notes with "Not specified" for missing owners/deadlines, copy notes, saved in the database.

| Check | State | Evidence |
|---|---|---|
| Golden topic-organisation test, real recording of computer sound, real AI | **Passed** | 3 topics (sales +12%, marketing Rs 50,000, hiring 3 interviews), decision "review next Monday" confirmed, no invented task, no owner, own note kept, every item time-stamped |
| Scenario 2 with a real Google Meet / Zoom call | **Not run** | Needs the owner |
| 60-minute recording, device unplug | **Not run** | |
| DOCX / PDF export, editing and versions, meeting search, crash recovery, diarization | **Not built yet** | Next |

### Installer
Building works. Upgrading the installed copy silently failed with exit code 2 on the last two
attempts (it had worked earlier the same day); the new program file was copied into place
instead, and its fingerprint matches the build. To investigate before sharing the installer.
## Update 13 November 2026 (night): Hinglish, English accuracy, meetings, web app

All numbers below come from SYNTHETIC speech (Windows voices, including the Indian-English
voices Heera and Ravi reading Roman Hinglish). They compare settings; they are not proof of
accuracy on the owner's own voice. Real recordings are still needed for that.

### Hinglish typed in Roman letters (new default: "English + Hinglish")
12 Hinglish clips (6 sentences x 2 voices), scored against the Roman text:

| Route | Word errors | Letter errors | Output |
|---|---|---|---|
| Whisper set to English | 67% | 40% | translated into English (wrong) |
| Whisper Hindi + Roman example (offline fallback) | 20.5% | 6.9% | Roman Hinglish |
| Whisper Hindi -> AI converts script only (main route) | 12.9% | 4.1% | Roman Hinglish |

Sample: "Bhai aaj ke liye sorry yaar, mera dimaag kaam nahi kar raha tha."
English vs Hinglish is decided per dictation from Whisper's language scores
(`app_asr::is_hinglish`): measured on 48 clips, normal-volume English 12/12 and quiet English
12/12 stay English, normal-volume Hinglish 12/12 goes Hinglish; very quiet noisy Hinglish only
6/12. Alt+Shift+H switches English+Hinglish / English only / Hinglish only.
The AI conversion is checked (no Devanagari left, same words count, same numbers); if the AI
is unavailable or refuses, the offline route is used. Groq's free per-minute limit was hit
during testing; normal use makes far fewer requests.

### English accuracy
- Quiet + noisy versions of the English clips (6% volume plus noise): with the dictionary hint
  the text is identical to the clean clips (remaining "errors" are digits vs words, e.g. "18%"
  vs "eighteen percent"). Without the hint, "EBITDA" became "EVI TDA".
- Beam search (5) was measured and NOT adopted: no gain on clean audio, slightly worse on hard
  audio (13.7% vs 11.8%), and 0.3 s slower.

### Meeting notes benchmark
13-line, 4-voice finance meeting (90 s) played through the speakers and recorded by the
installed app (throwaway data folder, AI off):
- computer-sound track: 90.6-91 s recorded for 90 s played, nothing lost;
- transcript vs script: **1.12% word errors** (179 words); transcription took 3 s;
- with the microphone on as well: the laptop's microphone array suppresses its own speakers,
  so the mic track was near-silent; Whisper's "you / thank you" on near-silence was dropped by
  the guard, so no false "Me" lines appeared. Echo removal (`summary::drop_echo`) is covered
  by a unit test; a real speaker-echo case was not reproduced.
- Golden topic test with the real AI passed earlier today. A real Meet/Zoom call: not run.

### Pill
The black box behind the pill came from the shared style sheet painting the page background.
The pill page now forces a transparent background; checked in the installed app
(background rgba(0,0,0,0)).

### Stage 4 - web app (PWA), first version
`apps/web`: phone/browser dictation with the owner's own Groq key (encrypted with a passphrase
in the browser), English + Hinglish modes, Roman Hinglish conversion, the SAME clean-up rules
and AI checks as the desktop (core compiled to WebAssembly), copy button, local history,
installable, offline shell. Checked in a browser: page and rules load; Groq accepts requests
from a web page; key saved, wrong passphrase refused, right passphrase unlocks.
NOT run: a real dictation (needs the owner's key in the browser), iPhone Safari, Android Chrome.
Publishing: `.github/workflows/deploy-web.yml` builds and deploys to GitHub Pages once Pages is
enabled for the repository.
## Measured accuracy per language

**None yet.** No real recordings have been scored. No accuracy figure is shown anywhere in the app.

## Costs so far

â‚¹0. All tools are free; Groq is used on its free tier with the owner's own key.

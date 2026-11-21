# Status

Last updated: **21 November 2026** (versions 0.4.0 to 0.4.6 "Speak Flow" sections near the end; Stages 5-9 before it; summary in docs/FINAL_REPORT.md). Stages 0, 1 and 2 are built, and GitHub CI passed on all
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
## 14 November 2026: "No speech model installed" - cause and fix

The Claude desktop app is an MSIX package. Programs started from it (the builder's shell) have
AppData redirected to `AppData\Local\Packages\Claude_pzs8sxrjxfjjc\LocalCache`. The earlier
installs and the model copy therefore went into that private copy; when the owner started
Bolkar himself, it used the real AppData folder, which had no model.
Fix (run outside the container through WMI): installer run for real (exit 0, real
`AppData\Local\Bolkar\app-desktop.exe` and Start-menu shortcut), model copied to the real
`AppData\Roaming\app.bolkar.voice\models` (574,041,195 bytes, verified), app restarted from the
real path. Confirming a dictation in the real app was NOT possible: further outside-container
processes were held (likely by the installed antivirus). AI clean-up consent must be given
again in Settings for the real data folder (the Groq key itself is in Credential Manager).
Future builders: install/copy for the owner only via a process outside the container.

## 14 November 2026: Hinglish on the owner's own voice, Android keyboard (Stage 5)

### Hinglish: what was measured on the owner's 5 WhatsApp voice notes
Five real voice notes from the owner (Ogg/Opus, Hinglish with a lot of English, 30 to 103
words each; kept out of git in `bench/my_recordings/owner`). There are **no reference transcripts
yet**, so there is no word-error figure: the routes were compared by reading the output
side by side. Errors named below are real examples from these recordings.

| Route | Result on the 5 notes |
|---|---|
| Local Whisper, Hindi (Devanagari) -> AI script conversion (old main route) | Poor. One note came out as nonsense; others lost words. |
| Local Whisper set to English | Translates the Hindi parts into English. Wrong. |
| Local Whisper, Hindi + Roman Hinglish example prompt (new local route) | Readable Roman Hinglish, English phrases kept. Misspellings and mis-hearings: "baut", "draa karun ga" (try karunga), "jester" (just), "Mathiran", "dosed to ne" (doston ne). In 1 of 5 notes, part of the text still came out in Devanagari. |
| ...plus AI Hinglish spelling polish (new) | Most of those fixed: bahut, try karunga, just, Matheran, doston ne, vishwas. One real mistake made by the AI: a garbled "2-3 akte baad" (hafte) became "ghante" in one run; the prompt now says to leave such words alone. Not re-measured after that prompt change. |
| Groq whisper-large-v3 (cloud, `hi`) -> AI script conversion | Best by far: close to word-for-word on all 5 (e.g. "Main area mein nahi aa paunga", "almost do teen hafte baad hi mil paunga"). Groq answers in mixed Devanagari/Roman whatever the prompt, so the script conversion is required. Remaining slips: "Nikkil", "Mathiran", "raay karunga" (try karunga). |

Plain English through Groq with language `hi`: 3 of 3 English meeting clips came back as
exact English (checked because the Android keyboard uses `hi` in English + Hinglish mode).

Changes made from these results:
- Local Hinglish route: Whisper `hi` + a generic Roman Hinglish example
  (`app_core::llm::ROMAN_HINGLISH_EXAMPLE`, shared by desktop, web and Android). The example
  is everyday wording, not words from the test notes, so the comparison above is not tuned to
  them; a first, more specific example was rejected for that reason.
- Leftover Devanagari is converted by the AI (checked as before).
- New AI Hinglish spelling polish (`spec/prompts/transliterate.md`, checked by
  `llm::parse_polish`: same numbers, word count within 15%, at least 60% of words unchanged,
  no Devanagari; otherwise the unpolished text is used). The raw transcript keeps what the
  recogniser heard; only the text passed on to clean-up is polished.
- Desktop with cloud recognition on: English vs Hinglish is now decided on the laptop first,
  and Groq is asked for `hi` (then script conversion) or `en` accordingly.
- Web app: Hinglish mode sends the Roman example and runs the spelling polish.
NOT run: the new desktop build in the owner's installed app (needs a rebuild + real install);
a scored word-error rate (needs the owner's corrected text for the 5 notes).

### Checks run on 14 Nov 2026
- `cargo fmt --all -- --check`: clean. `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --workspace`: **124 passed, 0 failed, 1 ignored** (includes the new Hinglish
  polish checks and the Android bridge tests on Windows).
- Web app: WebAssembly core rebuilt, `pnpm --filter web build` (type-check + bundle) clean.

### Stage 5 - Android keyboard (first version)
`apps/android` (Kotlin, no extra libraries) + `crates/android` (the shared Rust core through
JNI, so the same clean-up rules, AI checks and Hinglish prompts as desktop and web).
- Keyboard: large microphone key (tap, speak, tap), space, delete (hold = word), enter,
  language key (English + Hinglish / English / Hinglish), globe to go back to the usual keyboard.
- Recognition: the phone's own recogniser (default, no key) or Groq Whisper with the owner's key
  (separate consent for audio). Hinglish route: `hi` + Roman example -> script conversion ->
  spelling polish -> clean-up, each AI answer checked by the Rust core.
- Key stored encrypted with an Android Keystore AES key; audio kept in memory only.
- Set-up screen in large text: turn keyboard on, allow microphone, engine, key, consents,
  language, test box.
- Build: `.github/workflows/android.yml` builds the Rust library for arm64, armv7 and x86_64
  (cargo-ndk), runs the Rust bridge tests and the JVM unit tests (all golden fixtures through
  JNI), builds the APK and keeps it as a workflow artifact; a `v*` tag attaches it to a Release.
Not built on this laptop (no Android SDK or JDK here). GitHub Actions run 14 Nov 2026 (second\ntry; the first failed on a retired SDK package name): Rust bridge tests passed, JVM unit tests\n(all golden fixtures through JNI) passed, APK built (6.9 MB, debug-signed).\nA new Windows installer with the Hinglish changes was also built (not installed; the owner\nruns it, because installs started from the builder's shell land in a private copy).
NOT run: the keyboard on a real phone (WhatsApp, Gmail, Chrome), the phone recogniser, the
in-person recorder (not built yet).

## 14 November 2026 (afternoon): Stage 6 (English + Hinglish accuracy) and Stage 7 (Command Mode)

### Stage 6 - benchmark on 90 clips with known text
New test set (in git: `bench/sets/hinglish30.tsv`, `bench/sets/english15.tsv`): 30 everyday
Hinglish and 15 English sentences, each spoken by the two Windows Indian-English voices
(Heera, Ravi) with `scripts/make-clips.ps1` -> 90 clips. SYNTHETIC speech: these voices
pronounce Roman Hindi badly ("ghar pahunch gaya" comes out roughly as "ge paahyun ch"), so the
Hinglish numbers are pessimistic and mainly compare routes. Scored with `wer`; "tolerant"
also accepts equally normal Hinglish spellings (wo/woh, to/toh, jane/jaane, dunga/doonga; list
in `metrics::HINGLISH_VARIANTS`, different words are never merged).

| Set | Route | Word errors | Tolerant |
|---|---|---|---|
| English, 30 clips | local, app routing (English vs Hinglish per clip) | **0.93%** (3 of 322 words) | 0.93% |
| Hinglish, 60 clips | local, app routing | 41.4% | 36.0% |
| Hinglish, Heera 30 | local + offline spelling rules | 35.4% | 29.3% |
| Hinglish, Heera 30 | local + rules + AI spelling polish | 23.2% | 18.9% |
| Hinglish, Heera 30 | Groq cloud `hi` + AI script conversion | 17.5% | 14.5% |
| Hinglish, Heera 30 | Groq cloud + AI script conversion + AI spelling polish | **13.5%** | **10.4%** |

- 7 of 60 Hinglish clips (all Ravi) were routed as English by the local language check.
- Most remaining cloud errors are words the synthetic voice garbled ("Humari train shaam
  saat baje" spoken so that even a human would mishear). Real speech is the real test.
- The polish prompt (v2) now forbids adding words and the check allows at most one extra
  word; same score within noise (13.8% / 10.8%). It still once turned the garbled "baad
  kaate hai" into "baad mein kaam hai" on synthetic audio: an AI step can still rewrite a
  garbled phrase; the raw transcript is always kept in history.
- New offline Hinglish spelling rules (`app_core::hinglish`, fixtures
  `spec/hinglish/spelling.json`, 10 fix cases + 6 must-not-change): applied on the owner's 5
  real voice notes they made 11 fixes and no wrong change (Baut->Bahut x2, vohan->wahan,
  "aung ba"->aaunga, wishwaaz->vishwas, Pahle->Pehle, zyadha->zyada, mea->mein, "milpaun ga"
  ->milpaunga x2). On the synthetic set they change nothing (those spellings do not occur).
  On by default on every Hinglish route (desktop, web, Android); no AI needed.
- The owner's own 5 notes still have no corrected reference text, so no word-error figure on
  his voice. A draft to correct is in `bench/my_recordings/owner/refs-DRAFT-please-correct.tsv`.
- Not tried: the full Whisper large-v3 model (1.1 GB download), which may be better than
  "turbo" for Hindi. Needs the owner's OK to download.

### Stage 7 - Command Mode and Scratchpad
- **Command Mode** (`Alt+Shift+C`, again to finish, `Esc` cancels): copies the selection
  (clipboard put back afterwards), listens for an instruction, applies it, pastes over the
  selection. Built in, no AI: bulleted list, numbered list, all caps, lowercase, title case,
  sentence case, one paragraph, clean up (English and some Hinglish phrasings, e.g. "bullet
  points bana do"). Lists are pasted with an HTML version, so Word and Google Docs make REAL
  lists. Other instructions ("make it more formal") go to the user's AI and are checked
  (`command::accept_ai_edit`: no new numbers, not wildly longer). Every command is saved in
  history (instruction + selection as the raw text, result separately).
- Tests: `app_core::command` unit tests, `spec/commands/cases.json` (10 commands + 4 that must
  go to the AI), clipboard HTML header test, shortcut state machine test.
- **Real check in Microsoft Word** (`scripts/selftest-word-command.ps1`, release build, spoken
  instruction from a synthetic voice, separate data folder): Word asked through its object
  model after each run. "Make this a bulleted list." -> 1 real list, 3 bullet items;
  "Make it a numbered list." -> 1 real list, 3 items numbered 1. 2. 3.; clipboard restored.
  First version failed 3 of 10 runs (nothing pasted): Word reads pasted HTML late and the old
  clipboard had already been put back. Fix: list pastes wait 1.5 s before the restore, and
  the selection copy is tried twice. After the fix: **12 of 12 runs passed** (6 bullet, 6
  numbered).
- Self-test runs no longer use the one-copy-at-a-time rule, so a test never hands over to
  (or disturbs) the copy the owner is running.
- **Scratchpad** (`Alt+Shift+N`, Ctrl+Win+N was taken): a notes page in the app, saved on this
  computer as you type, Copy all / Clear. Checked: UI type-check and build only; NOT clicked
  through in the running app.
- Home page no longer says "audio never leaves this computer" when cloud recognition is on.
- NOT run: Command Mode in Google Docs (needs the owner's Google sign-in), an AI command end
  to end, a Hinglish spoken command with the owner's voice. NOT built: file tagging, extra
  context awareness beyond the existing per-app style.
- Checks: `cargo fmt --check` and `cargo clippy --workspace -D warnings` clean;
  `cargo test --workspace` **135 passed, 0 failed, 1 ignored**; web and desktop UIs build.
## 14 November 2026 (evening): full Whisper large-v3 tested; speech-dropping bug fixed

The owner approved downloading `ggml-large-v3-q5_0.bin` (1,081,140,203 bytes; SHA-256
d75795ec...8ad1, matches Hugging Face). Same tests as for Turbo:

| Set | Turbo | Large v3 |
|---|---|---|
| English 30 clips (app routing) | 0.93% | **0.62%** |
| Hinglish 60 clips, offline only | 41.4% (Roman) | 85% - writes Hindi letters in 47 of 60, ignores the Roman example |
| Hinglish Heera 30, + AI script conversion + rules + AI spelling | 23.2% (turbo + AI spelling) | **12.1%** (tolerant **9.4%**), best of every route, audio stays on the laptop |
| Speed, 60 Hinglish clips (RTX 4050) | 17.7 s | 59.6 s (about 1 s per clip) |

On the owner's 5 voice notes, Large v3 + AI conversion + spelling gave the most complete and
correct text so far (e.g. "Main area mein nahi aa paunga", "almost do teen hafte baad hi mil
paunga", "Matheran"); remaining slips: "jaisi tabhi" (just abhi), "raay karunga" (try
karunga), "Nikkil", "Sunna" (sun na). No reference text yet, so no score on his voice.

**Bug found and fixed (affects every model):** the English/Hinglish language check ran on the
same Whisper state as the transcription. That left a wrong "no speech" score (0.87 instead
of 0.00) on the first 30-second window, and the hallucination guard then deleted real
speech: Large v3 lost the first 28 s of two voice notes. Language detection now has its own
state (`WhisperEngine::detect_state`). After the fix: nothing dropped on the owner's notes or
the 90 clips; Turbo scores unchanged (it had not been hit on these clips). Real-app checks
after the fix: Word Command Mode 3 of 3, Notepad dictation test PASS (all 4 checks).

App: "Large v3 (best for Hinglish, with AI clean-up on)" added to the model list (download
and checksum handled by the app). Turbo stays the default (faster; Roman Hinglish offline).
Also fixed: the model list showed "Â·" instead of "·" (wrongly encoded characters).
Checks: fmt, clippy clean; `cargo test --workspace` 135 passed, 0 failed.
## 14 November 2026 (night): Stage 8 (install, updates, landing page) and Stage 9 (hardening)

### Stage 8
- **First run:** with no speech model, Home shows "One step before you can dictate" with one
  button that downloads the recommended model (Turbo on GPU builds) and selects it when done.
  Checked: shown in the real app with an empty data folder (window picture taken without
  focusing it). NOT clicked through (a 547 MB download).
- **Updates:** Settings > Updates shows the version, "Check for updates" (asks GitHub's public
  releases list; no personal data) and an opt-in automatic check at start-up (off by
  default). Nothing is installed automatically; a newer version gives a link to the release
  page (only github.com release pages of this repository are opened). Checked: version
  comparison tests; a live request to GitHub returned "No version has been published yet"
  (the repository has no release). Repository name now lives in `/brand.json`.
- **Release builds:** `.github/workflows/release-windows.yml` builds the GPU installer in
  GitHub Actions (Vulkan SDK 1.4.363.1 installed in the job) on a manual run or a `v*` tag;
  on a tag it is attached to the GitHub Release with a SHA-256 list (the APK is attached by
  `android.yml`). Manual run 14 Nov: **passed in 14.5 min**, installer 9,783,290 bytes. That
  CI-built installer was NOT installed here (it would replace the owner's running copy).
  Nothing has been published: a release needs the owner to push a version tag.
- **Landing page:** `apps/web/download.html` (published with the web app at
  `.../download.html`): what it does, downloads, privacy, link to measured results, the
  SmartScreen warning explained. No certification or accuracy claims. Checked in the browser
  pane: renders in light and phone width, no sideways scrolling, links correct. Its download
  buttons point at the latest GitHub Release, which does not exist until the owner publishes.
- Product name and repository now come only from `/brand.json` in the web app too (it had
  the name written in its code).
- **Code signing:** `docs/CODE_SIGNING.md` - free options compared, SignPath Foundation
  recommended, draft application answers. NOT applied for: the application must come from
  the owner. Paid options listed only for the owner to decide.
- Turbo's label no longer says "best accuracy" (Large v3 measured better for Hinglish).

### Stage 9 - hardening, measured on the release build
- **Long run:** 100 dictations of a 20 s recording in one session (`scripts/longrun.ps1`):
  **100 ok, 0 failed, all 100 texts identical**, median 850 ms per dictation (p90 878, max
  1449), app memory 216 MB after warm-up and 218 MB at the end (no growth). Repeated on the
  final build: 50 of 50 ok, identical, median 862 ms.
- **Failure tests** (`scripts/failure-tests.ps1`, release build): no model -> clear "No speech
  model installed" message; corrupt model file -> "could not load model", app ends normally;
  missing recording -> file error; 3 s of silence -> nothing typed; normal speech -> correct
  text. **All pass.**
- **Bug found and fixed:** 3 s of steady hiss produced the invented sentence "So, let's go."
  Whisper itself rated the hiss as speech (no-speech 0.00), so the old guard could not catch
  it. New check (`audio::looks_like_speech`): speech rises and falls with each syllable,
  steady noise does not. Measured: white noise and mains hum 0.1-0.7 dB, every real speech
  clip >= 16.8 dB (owner's notes 46-49 dB), speech in heavy noise down to 3.3 dB; threshold
  2 dB, so no speech is ever thrown away. After the fix: hiss and hum -> nothing typed; all
  speech-in-noise clips still transcribed. Known limit: rumbling ("brown") noise varies
  enough to pass the check, and Whisper still wrote "So" for it.
- **Word Command Mode on the final build:** 9 of 12 runs passed. Failures: 2 x the app
  reported "pasted" but Word's text was unchanged (the same symptom as the earlier 3 of 10,
  which the 1.5 s wait had cured: 12 of 12 earlier today); 1 x Word's selected text was
  replaced by a typed "E" while the app reported "nothing selected". The owner was using the
  laptop during these runs (testing a model) and the test brings Word to the front, so real
  key presses most likely interfered. Not proven either way: the failure may also be real. Focus-taking tests were stopped for the rest of the session;
  to re-run when the owner is away: `scripts/selftest-word-command.ps1` (earlier today 12 of
  12 passed with the owner away).
- Checks: fmt and clippy clean; `cargo test --workspace` **137 passed, 0 failed, 2 ignored**
  (the ignored ones are the live GitHub check and a machine-specific test); web and desktop
  UIs build. GitHub CI: see the run for this commit.
## 14 November 2026 (late): version 0.2.0 - redesigned app (owner's request)

The owner found the app generic and not interactive, and having to "save" settings and the
key repeatedly annoying. Rebuilt the interface (desktop and phone web app) with an original
design system (deep teal + amber, own icons; nothing taken from the reference screenshots,
which show another product).

**Desktop**
- New shell: sidebar with icons and groups, live status chip (ready / setup needed, Groq or
  local, AI clean-up), Light / Follow Windows / Dark switch (saved, applied before first paint).
- **Home**: greeting (optional name), real stat chips (streak, words this week, speaking
  speed), recent dictations by day with copy, live microphone meter + practice box, a setup
  checklist computed from real state, and a "Your setup" card. "How to dictate" moved to its
  own **How to use** page (keycaps, examples, Command Mode phrases).
- **Insights** (new): speaking speed with a gauge against typing at 40 wpm, words dictated,
  time saved (estimate, assumption stated on the card), 17-week activity calendar with current
  and best streak, where you dictate (by app), fixes, learned words, voice edits. Computed only
  from the local history (`insights.rs`, 4 unit tests incl. "empty history shows zeros, never
  made-up numbers"). New database column `fixed` (migration v3) to count corrections.
- **History**: search, filter (all / typed / commands), grouped by day, copy / fix / original /
  delete.
- **Settings**: six sections; every control applies instantly with a short confirmation, no
  Save buttons. **Groq** is one step: paste the key once, the app checks it with Groq and keeps
  it in Windows Credential Manager ("Connected - key ending ****abcd", never asked again); switches
  for Groq speech recognition and AI clean-up. Other AI services kept under "advanced".
- **Owner's defaults**: when a Groq key exists, AI clean-up and Groq speech recognition are on
  (applied once at start-up, logged; both switchable). The best installed speech model is
  selected automatically (Large v3 when AI clean-up is on, else Turbo); a model picked by hand is
  respected. Not applied in test runs with their own data folder.
- Meeting notes, Dictionary, Snippets, Scratchpad restyled; language choice limited to English +
  Hinglish / English / Hinglish (owner's decision). Default window 1280 x 860.

**Phone web app**: same look, big animated microphone with live waveform, recording timer,
result auto-copied, **Share** (straight to WhatsApp), stats, searchable history, light/dark.
The key is remembered on the device with a browser-held non-extractable key: no passphrase on
every visit (a passphrase stays available for shared devices).

**Checks run**
- `cargo fmt`, `cargo clippy --workspace -D warnings` clean; `cargo test --workspace`
  **141 passed, 0 failed**; desktop and web type-check and build.
- Every desktop page looked at in both themes in the browser pane, using development-only
  sample data (`dev-preview.ts`, never in the installed app). Fixed from that review: language
  buttons wrapping, unlabelled gauge marker, cramped shortcut rows, sidebar too tall.
- Real app (release build, throwaway data folder, 4 real dictations): Home showed real values
  (1-day streak, 43 words, 144 wpm) - window captured without focusing it.
- Web app at phone width (375 px): setup screen, main screen, light and dark, opens without a
  passphrase after reload, no sideways scrolling. A fake placeholder key was used and removed.
- Found and fixed while testing: the owner's Groq key lives in Windows Credential Manager, so
  the new default also switched Groq on inside a throwaway test folder; the 4 test dictations
  above (synthetic English sentences, no personal data) were therefore sent to Groq with the
  owner's key (about 8 small requests). Test runs now never apply the Groq default.
- NOT run: clicking through the new Settings in the installed app (the owner was using the
  laptop), the Android keyboard (no visual changes this round), Command Mode re-test.
## 15 November 2026: version 0.3.0 - dictation fix, new Home, Style, better dark theme

### "Dictation does not work at all" - found and fixed
- Owner's log: last dictation 14 Nov 16:30 IST (0.1.0); after 0.2.0 started (01:10) not one
  dictation was logged. His settings: model **Large v3** (picked automatically by 0.2.0) and
  **"Use the graphics card" off** (it was on earlier the same day: the log shows "(GPU)").
- Reproduced with a copy of his database and the same models in the real app: one 4 s English
  clip was still unfinished after 150 s, with the app using about 5.6 processor cores. Cause:
  Large v3 running on the processor (start-up warm-up plus the English/Hinglish check take
  minutes), so key presses waited behind it and nothing appeared to happen.
- Fixes: (1) once, the graphics card is switched back on when one exists (logged); (2) the big
  models are never picked automatically without a graphics card; (3) the warm-up only runs on
  the graphics card; (4) with Groq on and no graphics card, the slow local language check is
  skipped (Groq detects the language); (5) Home, the sidebar and Settings warn when the
  graphics card is off.
- After the fix, same copy of his settings, real app: English 0.59 s and Hinglish 0.82 s
  recognition; on the 0.3.0 installer build 4 more clips (2 English exactly right, 2 Hinglish
  with synthetic-voice errors) in 0.5-2.2 s. Failure tests: all 6 pass.
- Keyboard hook hardened: time-critical thread priority, no disk writes in the hook, and the
  hook is re-attached every 20 s while no shortcut key is held (Windows silently removes a hook
  that answers too slowly, e.g. when the processor is saturated).
- NOT verified: a real Ctrl+Win press. Key presses sent by software from the Claude sandbox
  do not reach the app's hook, and a run started outside the sandbox (WMI) received only some
  key releases. `scripts/selftest-hotkey.ps1` (test-only switch APP_TEST_INJECTED_KEYS, own
  data folder, never types) can be run by the owner from a normal PowerShell window.
- Test runs with a trace (APP_TEST_LOG) never type into any window.

### Design and features
- **Home**: greeting with streak / words / WPM pills; **daily goal** challenge (default 100
  words, set in Settings) with progress and a celebration when reached; rotating **banner** with
  three original illustrations drawn for this app (Hinglish chat, edit by voice, styles; no
  photos or third-party artwork), each with a button; recent dictations, checklist, practice
  box, setup card (now incl. graphics card).
- **Style** page: per kind of app (personal messages, email, documents, everything else) choose
  Casual / Clear / Formal; the AI clean-up uses it (`llm::build_cleanup_styled`, unit test).
  Defaults: casual chat, formal email, clear elsewhere.
- **Sounds**: soft start / done / error cues (Web Audio, no files), switchable.
- **Dark theme** reworked: layered surfaces, softer borders, card highlight, richer gradients.
- Checks: fmt and clippy clean; `cargo test --workspace` **142 passed, 0 failed**; UI builds;
  Home and Style looked at in both themes (browser pane, development sample data).

## 16 November 2026: version 0.4.0 - faster recognition, spoken points, new name "Speak Flow"

### Why dictation felt slow (owner's log, 16 Nov)
- Recognition took 0.9-12.6 s per dictation, while clean-up took about 0.25-0.4 s and typing
  about 0.4 s. Groq itself answers in about 0.28 s from this laptop. The time went into
  (1) uploading uncompressed WAV audio (about 32 KB per second of speech) over a slow uplink,
  (2) a local English/Hinglish check before every upload, (3) two extra AI calls for Hinglish.

### What changed
- **Compressed upload**: audio for Groq is now sent as Ogg/Opus (libopus, 24 kbit/s speech
  mode, own Ogg writer with CRC; `crates/asr/src/opus.rs`, 2 unit tests). WAV is the fallback.
  Measured on the test clips: 145 KB -> 11.5 KB (12.6x smaller). Accuracy unchanged within noise:
  Hinglish raw WER 77.1% (WAV) vs 77.8% (Opus) on the hard synthetic set; English 0.62% vs 0.62%.
- **No local language check with Groq**: with Groq on, the local model is not run first. English
  setting -> Groq `en`; English + Hinglish -> Groq `hi` with a Roman-Hinglish example prompt (and
  the owner's name from Settings as a spelling hint). Hinglish is recognised afterwards from the
  text itself (`hinglish::looks_hinglish`, unit test). If Groq fails the local model is used.
- **One AI call instead of two** for Hinglish in Devanagari: Roman letters and WhatsApp spellings
  in one request (`llm::roman_spelling_prompt`, spec/prompts/transliterate.md). Measured WER
  vs. two calls: 12.46% vs 13.47% (tolerant 10.10% vs 10.44%), 0 answers rejected.
- **No "Inserted" pop-up**: on success the pill just disappears; messages are shown only for
  problems (or when the text could not be typed and is on the clipboard).
- **Spoken points**: "first thing ..., second thing ...", "point one ...", "pehli baat ...,
  doosri baat ..." become a numbered list; a short opener ("So", "Okay") before the list is
  dropped. In hands-free mode (Ctrl + Win + H) two or more separate items become bullet points
  (Settings > Writing > "Hands-free: write as points", on by default). Golden fixtures x-pts-001
  (the owner's own example -> "1. My name is Kunal Kanjwani / 2. My roll number is 52"),
  x-pts-002/003 and two negative cases (x-pts-n01/n02: ordinary sentences stay prose).
- **With Groq on, Large v3 Turbo is preferred** as the local fallback (fast); full Large v3 only
  when everything is local.
- **New name "Speak Flow"** (brand.json, `previousNames: ["Bolkar"]`). The internal app id is
  unchanged, so settings, history and models are kept. The Groq key is now stored under the app
  id and moved automatically from the old name (unit test against the real Windows Credential
  Manager: passes).
- **Home banner photos**: three real photos instead of drawn illustrations, from Unsplash under
  the Unsplash License (free to use, no attribution required; credited here anyway):
  photo-1663743629961-c6134ef1afc9 (man laughing on a phone), photo-1731801613353-bad3f67e956c
  (woman smiling at a screen), photo-1782069327324-57353ef863fc (man with a laptop, mirrored).
  Cropped to 1400x620. A dark gradient keeps the white text readable. Checked in the browser
  pane: light, dark and narrow window.

### Checks run
- `cargo fmt --check` clean; `cargo clippy --workspace --all-targets -D warnings` clean.
- `cargo test --workspace`: **149 passed, 0 failed, 2 ignored**; golden clean-up fixtures
  165 of 171 cases right (the same 6 known, non-must-pass failures as before; all new x-pts
  cases pass).
- Desktop and web front ends build; the release installer `Speak Flow_0.4.0_x64-setup.exe` builds.
- **Real app, release build, copy of the owner's settings** (Groq speech + AI clean-up on, his
  own key; nothing typed anywhere). Recognition time includes the Opus upload:

  | Clip | Recognition | Result |
  |---|---|---|
  | English, 7.8 s (synthetic) | 563 ms | exact |
  | English, 4.2 s (synthetic) | 410 ms | exact |
  | Hinglish h05, 4.9 s (synthetic) | 668 ms | exact ("Mummy ne bola hai ki khana ready hai, jaldi aa jao.") |
  | Hinglish h12, 2.7 s (synthetic) | 399 ms | wrong words and Title Case ("Weekend PE Movie Thekna Chalantya.") - known weak clip of the synthetic set |
  | Owner's points example (Windows voice) | 581 ms | "1. My name is Kunal Kanjwani" / "2. My roll number is 52" |
  | Hands-free, 3 requests (Windows voice) | 1253 ms | 3 bullet points, words right, but a leading "Um" came back as "Am" (Groq answered in Devanagari) |
  | Same two clips after prompt v2 | 547 ms / 977 ms | points unchanged; hands-free: "• Please book the meeting room for Monday." / "• Also, send the agenda to the whole team." / "• And remind me to call the bank tomorrow." - filler removed |

- **Quiet test mode** (owner's request: tests must not pop up while he studies): with
  `APP_SELFTEST_QUIET=1` a self-test shows no pill, no tray icon and plays no sounds; the main
  window now starts hidden and is shown at normal start-up only. The test script watches for any
  visible window of the test process every 100 ms (ignoring the framework's 14x14 transparent
  helper window) and stops the test if one appears; none appeared in the final runs.

## 16 November 2026 (evening): version 0.4.1 - warmer banner photos, old installs removed

### Why the owner still saw "Bolkar" and the old pictures
The owner had installed Speak Flow 0.4.0, but the old Bolkar app (in
`%LOCALAPPDATA%\Bolkar`) was still running in the tray. Both share the internal app id, so the
single-instance check handed every Speak Flow start to the running Bolkar window, which showed
the old name and the old drawn banner. The 0.4.0 source itself has no visible "Bolkar" text
(searched `apps/` and `crates/`; only the internal id `app.bolkar.voice`, the web app's
storage name and its cache name remain, none of them shown to the user).

### What changed
- **Home banner photos** chosen to match the owner's reference picture (warm, softly lit,
  people laughing, dark banner). Four slides now, the fourth explains "Speak in points" and
  opens Settings > Writing. Photos from Unsplash under the Unsplash License (free to use, no
  attribution required; credited here anyway): photo-1722247410946-e1c0e69ecaa1 (woman
  laughing, cropped above a box in her friend's hand), photo-1785435269070-3f980c7b8d65 (woman
  smiling in warm light), photo-1591286751500-eb873051de44 (man with a laptop in warm light,
  mirrored), photo-1761145226797-1e0e1d49a11d (two friends at night). Cropped to 1600 px wide.
- Banner background changed from blue-black to warm black; the highlighted word is warm and
  italic; the photo area is a little wider and the banner a little taller.
- **Bug fixed:** in dark theme the banner button ("Try it now") was dark text on a dark
  background (a dark-theme `.btn` rule overrode it). It is white with dark text in both themes.
- Checked in the browser pane (nothing shown on the owner's screen): all four slides in dark
  theme, light theme, a 900 px wide window, and the "Points settings" button opening Settings
  with the points option visible. The only console errors are the expected ones from desktop-only
  calls in a plain browser.

### Checks run
- `cargo fmt --check` clean; `cargo clippy --workspace --all-targets -D warnings` clean.
- `cargo test --workspace`: **149 passed, 0 failed, 2 ignored**.
- Release installer `Speak Flow_0.4.1_x64-setup.exe` builds (`pnpm tauri build`, which also
  type-checks and builds the front end).

### Install on the owner's laptop
Done at the owner's request ("delete the earlier versions and install the newest version"):
- Stopped the running Bolkar app, backed up history, settings, meetings and audio (not the
  1.6 GB models) to `D:\dev\backup\speakflow-data-2026-11-16`.
- Silent uninstall of Bolkar and of Speak Flow 0.4.0 (exit code 0 each). The Tauri uninstaller
  deletes app data only when its checkbox is ticked, and silent mode never shows it (checked in
  the generated `installer.nsi`). Removed the leftover `%LOCALAPPDATA%\Bolkar` folder, the
  Bolkar Start menu shortcut and the old `HKCU\Software\bolkar\Bolkar` key.
- Models were the same size before and after (1,655,181,398 bytes); the database is still there.
- Silent install of 0.4.1 (exit code 0): installed version 0.4.1, Start menu and desktop
  shortcuts "Speak Flow". The old Android APK in Downloads went to the Recycle Bin; old installers
  in the build folder were deleted.
- Started the new app. Windows does not keep a Tauri window minimized, so the Speak Flow window
  opened once on the owner's screen. Not yet checked by the owner: a real Ctrl + Win dictation.
- Kept on purpose: the data folders `%APPDATA%\app.bolkar.voice` and
  `%LOCALAPPDATA%\app.bolkar.voice` (history, models, settings, WebView cache). Their names come
  from the internal app id and are never shown in the app. Changing the id would mean moving
  1.6 GB of data, so it was left alone.

## 17 November 2026: version 0.4.2 - audit remediation

An independent review of 0.4.1 (ChatGPT, from the review pack: source, installer, screenshots,
audio, logs) scored it 5.4/10 and listed 21 findings plus an improvement prompt. Every finding
was checked against the code first; all were real. What was fixed, how it was tested and what
was not done is in `docs/REMEDIATION_PLAN_0.4.2.md`; the full report is
`docs/0.4.2_FINAL_REMEDIATION_REPORT.md`. In short:

- Meeting transcription no longer passes "en-hinglish"/"hinglish" to Whisper (typed language
  modes; English or Hindi+Roman per piece).
- Dictation is natural prose by default; lists only when spoken ("first ..., second ...",
  "bullet point"); hands-free bullets are now an opt-in setting.
- Meeting notes: a paragraph per topic, honest checks (quote found / check this, tentative
  deadlines, "confirmed" only with agreement), long meetings in windows with retry on rate
  limits, near-live transcript, sentence-level time stamps, "Microphone"/"System audio" labels.
- Meeting library (search, open, rename, correct lines, notes again, transcribe again, export,
  delete, interrupted-recording recovery) and separate retention for meeting audio, history and
  meetings, with real deletion.
- Recorder errors shown instead of swallowed; device choice; all sample formats.
- Diagnostics log (content-free) with export; warning when other dictation apps run.
- Web app: key really deleted, CSP, safer Groq parsing. Android recorder stop fixed.
- Found during testing and fixed: Groq answered one Hinglish clip in capitals only.

### Accuracy, measured 17-18 Nov 2026 (synthetic Windows voices Heera and Ravi; no real voices)

All 102 supplied clips, scored with `wer` (same normalisation both sides). "App route" = the
release build with the owner's settings (Groq whisper-large-v3 + AI clean-up), run quietly one
clip at a time. "Local Turbo" = whisper large-v3-turbo on this laptop's GPU, English/Hinglish
decided per clip (batch tool), raw text before clean-up.

| Set | Clips / words | Route | WER | Tolerant WER | CER | Hindi letters in output |
|---|---|---|---|---|---|---|
| English | 30 / 322 | App route, final text | 0.62% | 0.62% | 0.34% | 0 |
| English | 30 / 322 | Local Turbo, raw | 0.93% | 0.93% | 0.62% | 0 |
| Hinglish (30 sentences x 2 voices) | 60 / 594 | App route, raw | 21.38% | 18.86% | 6.45% | - |
| Hinglish (30 sentences x 2 voices) | 60 / 594 | App route, final text | 20.03% | 17.68% | 6.35% | 0 |
| Hinglish (30 sentences x 2 voices) | 60 / 594 | Local Turbo, raw | 41.41% | 36.03% | 11.94% | 0 |
| Hinglish (6 sentences x 2 voices) | 12 / 132 | App route, final text | 24.24% | 22.73% | 7.06% | 0 |
| Hinglish (6 sentences x 2 voices) | 12 / 132 | Local Turbo, raw | 31.82% | 26.52% | 7.98% | 0 |

Honest notes: the earlier best Hinglish figure (13.5%) was the female voice only on 30 clips;
on all 60 clips the app route is 20.0%. Synthetic voices are not people: these numbers are for
comparing versions, not a promise for real speech. Known bad clip h12 is still wrong ("Weekend
PE Movie Dekne Chalankya."). Median recognition time in the app route: 455 ms (English), 642 ms
and 627 ms (Hinglish sets), upload included.
### Meeting tests (quiet app test)

| Meeting (synthetic) | Audio | Live pass | Final pass | Notes | In library | Marks / removed |
|---|---|---|---|---|---|---|
| English finance review, 13 lines, 4 voices | 87 s | 10 lines, 2.2 s | 3 lines, 0.7 s | 4.6 s | yes | 0 / 0 |
| Hinglish, 15 sentences | 85 s | 4 lines, 2.3 s | 1 line, 0.7 s | 22.7 s (one rate-limit retry) | yes | 7 / 2 |
| Mixed English + Hinglish, 12 sentences | 64 s | 8 lines, 1.4 s | 4 lines, 0.7 s | 63.9 s (rate-limit retries) | yes | 1 / 2 |

English notes: one paragraph per topic, decision "approve 50,000 rupees" kept its condition
(return on investment above three times) and is Confirmed ("Agreed"), owners Priya / Rahul
with "next Friday" / "the 18th", "I am not sure yet" stayed an open question. Each transcript
line matched one spoken sentence (sentence-level time stamps). Mixed meeting: every English
line came out in English and every Hinglish line in Roman Hinglish. The checks removed "3 PM"
(the AI's guess for a mis-heard "saat baje") and, wrongly, "2 days" for "do din" (Hindi number
words are not read as numbers, on purpose: "do" is also English). Hinglish meeting marks are
mostly real problems of the synthetic Hinglish transcript ("subahdaas" for "subah das").
Before retries were added, the free Groq limit made two of these note runs fail; the
transcript was kept each time.
### Checks run

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` | clean |
| `cargo test --workspace` | 179 passed, 0 failed, 2 ignored |
| `pnpm exec tsc --noEmit` (desktop, web) | clean |
| `pnpm build` (web) | built; CSP present in `dist/index.html` |
| `pnpm tauri build` | `Speak Flow_0.4.2_x64-setup.exe` built |
| Desktop UI in the browser preview (sample data) | Meeting record + library + detail, Settings > Privacy and Writing, Home warning checked; no new console errors |
| Web key store in a real browser (dev server) | save / unlock / wrong passphrase / forget: as expected, no records left |
| Android build | not run locally (no Android SDK on this laptop); GitHub CI builds it |

## 19 November 2026: version 0.4.3 - first words kept, better accuracy, smart layout, learning

The owner reported: the first 2-5 seconds of a dictation are lost; accuracy in English and
Hinglish is not good enough; text should be laid out like their picture (paragraphs, bullet
points, or both with labels such as "Key decisions:"); the app should learn from their
corrections; hundreds of dictionary words and snippets; install the newest version.

### First seconds lost: what was found and what changed

- 10 of the owner's 12 most recent dictation recordings began with 2.2 to 4.0 s of exact
  digital zeros: the microphone stream was open but delivering silence-zeros, so the start of
  the speech was never captured (not a recognition problem).
- The recorder now has a watchdog: a stream that errors, delivers nothing for 2 s, or only
  zeros for 4 s is reopened (backoff 0 s, 2 s, 10 s, 60 s). At Ctrl+Win+H a stale stream is
  reopened at once, 1 s of pre-roll is kept, leading zeros are cut, and the pill says
  "Mic starting... speak after the sound" until real audio arrives; the start sound plays only
  then. The diagnostics log records whether the microphone was ready and how many ms of zeros
  were cut.
- Not proven: why the Windows microphone delivers zeros at first on this laptop. The new log
  lines will show it on the owner's next dictations. Unit tests cover the watchdog decisions.

### Accuracy: measured on the owner's own 53 recordings and on the synthetic sets

The owner's 53 dictations have no corrected reference text, so they were judged by reading.
- Groq with language "hi" (the 0.4.2 route) wrote 6 of them in Hindi letters; 2 of those were
  English speech turned into invented Hindi-letter text. Language "en" translates real Hindi
  into English. No fixed language ("auto") was best on English and mixed speech (for example
  "accuracy", "Hinglish", "roll number" right) but wrote 2 real Hindi clips in Hindi letters and
  1 in Urdu letters.
- Synthetic sets (Windows voices, scored with `wer`): English, the same 0.62% in every mode.
  Hinglish 30: "en" 46.46%, "auto" 71.38% raw (34 outputs not in Roman letters), the app's "hi"
  route 21.38%; mixing the two by swapping non-Roman lines 25.76%.
- New route for "English + Hinglish": both requests are sent at the same time; the "auto"
  answer is used only when it is plain English (Roman letters and not Hinglish words),
  otherwise the "hi" answer is used and any part of it in Hindi letters that the local
  language check calls English is recognised again as English for those seconds. So synthetic
  Hinglish gives the same result as 0.4.2, and English speech can no longer turn into Hindi
  letters.
- Release build, quiet app test with a copy of the owner's settings, on 12 of their
  recordings: English ones came back in English (including the long dictation about this bug
  and the neckband research request), Hinglish ones in Roman letters ("Nikhil bhai, aaj area
  mein aayega kya bata de?", "And aapko mujhe ye bhi batana tha ki ..."), the Urdu-letter clip
  now in Roman letters. Remaining faults are the recogniser's hearing, for example "even in
  English or in English. Hinglish." (the raw recogniser text already says so) and "pick a blue
  call". 100% is not reached and is not claimed.

### Smart layout (the owner's picture)

- New default "Smart layout" (Settings > Writing > Layout). For dictations of 3+ sentences or
  40+ words, the AI proposes paragraphs, bullets, numbered steps and spoken labels
  ("Key decisions:", "Next steps:"); `layout::accept_layout` accepts it only if every spoken
  word is kept in the same order (only a few joining words such as "and", "next", "mein" may go,
  spoken numbers never; no heading the speaker did not say). Otherwise the text is typed as
  spoken. Stories and explanations stay paragraphs. Prompt: `spec/prompts/layout.md` v2.
- `layout-eval` (Groq, the owner's model) on 7 dictations, 4 written for the test and 3 of the
  owner's: the meeting text became a paragraph, "The key decisions were:" with 3 bullets and
  "Next steps are:" with 3 numbered steps; the three-part project explanation became a numbered
  list between two paragraphs; the story, the shopping text (too short) and the owner's 3
  dictations stayed as spoken. The Hinglish meeting text was refused at first ("mein" dropped,
  then 5 of 34 words dropped); after allowing "mein"/"toh" and not counting order words that
  numbering replaces, the same layout passes the unit test (not re-sent to Groq).
- Release build, quiet app test on 3 computer-voice recordings of the same texts: meeting -
  paragraph, then "Next steps are:" with 1-3 numbered (the decisions stayed a sentence);
  project - paragraph, numbered 1-3, paragraph; story - one paragraph. All three layout requests
  to Groq succeeded.
- A paragraph, a blank line, then the list; a label stays directly above its items.
- Found in testing: right after the clean-up call, the layout call was often refused by Groq's
  per-minute limit (8,000 tokens per minute for this model on the free plan, read from Groq's
  rate-limit headers). Chat calls now wait out one short "try again" (up to 2.5 s, inside the
  time limit); fake-server test added.

### Learning from corrections, words and snippets

- After a dictation is typed, for 90 s the app reads the same text box (Windows UI
  Automation; same app only; never password boxes) every 4 s. If the owner changes a word, the
  pair is learned: clear fixes replace automatically, risky ones only steer recognition. Only the
  count is logged. Switch: Settings > Writing > "Learn from my corrections". The word-pair logic
  has unit tests; reading a real app's box was not tested by hand, because that would show on
  the owner's screen.
- Word packs added once to every install: study, tools, personal (300+ terms: cost and management accounting, finance, tax and law; apps and AI
  tools the owner uses; words heard in the owner's dictations), plus 90+ starter snippets
  ("insert ..." triggers). No personal name ships in the app: signature snippets use the name
  set in Settings ("Your name") and are left out when none is set. Tests: sizes, no duplicates, risky everyday words
  kept hint-only.

### Checks run (19 Nov 2026)

| Command | Result |
|---|---|
| `cargo fmt --all -- --check` | clean |
| `cargo clippy --workspace --all-targets -- -D warnings` (and with `--features dev-tools`) | clean |
| `cargo test --workspace` | 197 passed, 0 failed, 2 ignored |
| `pnpm exec tsc --noEmit` (desktop, web) | clean |
| Desktop UI in the browser preview | Settings > Writing shows the four Layout choices and "Learn from my corrections"; only the known "no Tauri in a browser" listener errors in the console |
| `pnpm tauri build` | `Speak Flow_0.4.3_x64-setup.exe` built |
| Install on the owner's laptop | not done from this session: installs started from Claude's sandbox land in Claude's private storage, not the owner's Windows profile. The owner runs the installer (see HOW_TO_USE) |

## 20 November 2026: version 0.4.4 - fixes from the independent audit of 0.4.3

The independent audit of 0.4.3 (ChatGPT, from the full review pack) scored it 6.2/10 and gave
a remediation prompt; both are in `docs/audit-0.4.3/`. Every finding was checked in the code
first. Plan: `docs/AUDIT_REMEDIATION_PLAN.md`; evidence: `docs/V0.4.4_TEST_REPORT.md`,
`SECURITY_REGRESSION_REPORT.md`, `MEETING_REGRESSION_REPORT.md`, `ACCURACY_ENGLISH.csv`,
`ACCURACY_HINGLISH.csv`; owner's hands-on list: `RELEASE_CHECKLIST.md`.

- **Key-safe provider addresses (audit P0):** parsed URL; plain http only for localhost or a
  loopback IP; no user info, query or fragment; checked again before every request carrying the
  key; redirects never followed. Every address from the audit tested.
- **Meeting loop (audit P0):** cause found by A/B on the same WAV - the 400-character dictionary
  hint, full of the 346 pack words added in 0.4.3, made Whisper repeat one line 5 times and lose
  3 turns. Hints now use only the user's own and learned words; a looping piece is recognised
  again without the hint; 3+ identical lines in a row are collapsed. English test meeting:
  20.67% -> 2.23% WER, all 13 turns once.
- **Decisions (audit P0):** never "Confirmed" next to "check this"; the line before is cited when
  it holds the condition. Audio sources are no longer listed as participants.
- **Correction learning:** pinned to the window and text box it typed into; stops when the focus
  moves; reads a few lines around the cursor; COM set up once; "Forget learned words".
- **Smart layout:** only and/also/then/finally/lastly/aur/phir and order words may be left out,
  and only where a new line starts (0.4.3 also allowed "we", "will", "is", "hai", "mein" ...).
  One bullet style ("•") everywhere. Prompt v3.
- **English + Hinglish routing:** an answer is used as English only if the Hindi-mode answer and
  the local language check agree; when the AI step is limited, the Roman answer of the parallel
  request is used instead of Hindi letters.
- **Groq free daily limit (found during the benchmark):** 200,000 tokens per day for the
  clean-up model, about 70-100 dictations. When it is reached, the next Groq model (own
  allowance) is tried once, for dictation and meeting notes. Seen working in a self-test.
- Smaller: upload counts per dictation in the diagnostics log; consent text states the fan-out;
  the keyboard hook never blocks; Android recorder released only after its reader ends;
  Settings text fixed (1 s pre-roll, no "never cut off" promise).

### Measured (release-build self-tests, synthetic voices)

| Set | WER | Tolerant | Note |
|---|---|---|---|
| English, 30 clips / 322 words | 0.62% | 0.62% | unchanged |
| Hinglish A, 60 / 594 | 20.37% | 17.68% | run 1, before the routing fixes |
| Hinglish B, 12 / 132 | 31.82% | 30.30% | run 1, worse than 0.4.2 (24.24%): causes fixed (see test report) |
| Hinglish B, 6 clips after the fixes | 30.30% -> 21.21% | 27.27% -> 18.18% | same 6 clips; full re-run not run (Groq daily limit used up) |
| Meeting, English 87 s | 20.67% -> 2.23% | | all 13 turns |
| Meeting, mixed 64 s | 26.36% -> 24.55% | | Hinglish lines still misspelt by the local recogniser |

Checks: `cargo fmt` clean, `clippy -D warnings` clean (also with dev tools), `cargo test`
210 passed / 0 failed / 2 ignored, `tsc` clean (desktop, web). Not run: everything in
`RELEASE_CHECKLIST.md` (needs the owner at the keyboard), real-voice accuracy (no references),
the full Hinglish re-run after the fixes.

## 20 November 2026 (evening): version 0.4.5 - small AI clean-up requests

After the owner's dictionary grew to 1,564 words, the real diagnostics log showed clean-up
requests of 12-19 KB (every dictionary word was listed in the request) and "no answer in time" /
"free limit reached" on several dictations. 0.4.5 sends only the words that matter for the
dictated text (`dictionary::relevant_terms`: words that occur in it, then look-alikes with the
same first four letters, at most 40). Every word still corrects spellings on this computer as
before. Test: 2,005 terms -> the right 4 for a sentence (exact, then a near match "kaggel" ->
"Kaggle"), at most 40. Checks: see the commit; request sizes in real use to be read from the
owner's diagnostics log after installing.

## 21 November 2026: version 0.4.6 - long dictations complete, faster

Problem (owner): in dictations of several minutes, words in the middle went missing. Cause,
measured on 14 of the owner's own long recordings (50-135 s): sending the whole recording to
Whisper in one request silently skipped whole stretches (46 words in a row in one, 20 in
another). Fix: recordings over 40 s are cut at quiet moments into pieces of at most ~29 s, all
sent at the same time (at most 6 in flight, retried once after 2 s, a piece still refused is
recognised on this computer); a piece that comes back with far too few words for its speech is
redone in two halves. Hinglish pieces get the Hindi-mode second pass per piece. Also: the
dictionary is one lookup table instead of one pattern per word (5,000 words: build 1.31 s ->
6.3 ms, apply 117 ms -> 0.08 ms); AI time limits grow with text length (2.5 s + 12 ms per word,
at most 9 s); clean-up falls back to the second Groq model when the first hits its limit.

Measured with the release build (quiet self-test, test profile, nothing typed or shown):

| Test | Before (0.4.5) | 0.4.6 |
|---|---|---|
| Owner's 14 long recordings, words returned (old app vs new) | 2 recordings lost 37-59 words (98 vs 157, 120 vs 163) | all within 3 words of the piece-by-piece reference; 1.2-4.1 s recognition each |
| Synthetic English, 5.6 min, 881 words | WER 3.97%, 4 deletions, 11.8 s | WER 3.29%, 1 deletion, 4.7 s |
| Synthetic English + Hinglish, 3.9 min, 458 words | WER 35.37% | WER 6.33% (5.02% with Hinglish spelling variants), 13.9 s |

The owner's recordings have no corrected reference text, so for them only word counts are
compared, not accuracy. Checks: `cargo fmt` clean, `clippy -D warnings` clean, `cargo test`
217 passed / 0 failed / 3 ignored. Not run: `RELEASE_CHECKLIST.md` (needs the owner at the
keyboard); real dictations of 5-10 minutes (none recorded yet; the longest tested is the 5.6 min
synthetic one).

Dictionary: 1,000 prepared words (800 English, 200 Hinglish) and 2,400 new ones (2,055 English =
86%, 345 Hinglish) checked against the owner's existing 1,568 words (no repeats of terms or
spoken forms). Plain English phrases are added as hints only (they help recognition and the AI
keep them; they never change capitals); only acronyms, special spellings and words with a spoken
form auto-correct. The database is backed up to Downloads before each addition.

## Measured accuracy per language

**Real voices: no scored figures yet.** The owner's 5 voice notes have no corrected reference
text (see above). Synthetic-voice figures for all 102 supplied clips are in the 0.4.2 section
above. No accuracy figure is shown anywhere in the app. Only English and Hinglish are worked on, by the
owner's decision (14 Nov 2026); no other Indian languages are being added or measured.

## Costs so far

â‚¹0. All tools are free; Groq is used on its free tier with the owner's own key.

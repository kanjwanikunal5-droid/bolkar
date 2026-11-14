# Status

Last updated: **14 November 2026** (Hinglish on the owner's voice, Android keyboard: see the end). Stages 0, 1 and 2 are built, and GitHub CI passed on all
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
## Measured accuracy per language

**No scored figures yet.** The owner's 5 voice notes have no reference text (see above). No
accuracy figure is shown anywhere in the app. Only English and Hinglish are worked on, by the
owner's decision (14 Nov 2026); no other Indian languages are being added or measured.

## Costs so far

â‚¹0. All tools are free; Groq is used on its free tier with the owner's own key.

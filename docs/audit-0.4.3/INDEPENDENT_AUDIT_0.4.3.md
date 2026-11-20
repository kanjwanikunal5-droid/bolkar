# Speak Flow 0.4.3 — Independent Engineering Audit

**Review date:** 19 November 2026  
**Release:** v0.4.3  
**Commit reported in review pack:** `98c4751`  
**Application:** Speak Flow for Windows (Tauri 2 + Rust + React/TypeScript), with PWA and Android keyboard components

> **Overall verdict: 6.2/10 — a promising and substantially improved engineering beta, but not ready to be trusted as a dependable meeting-recording and documentation product.**

This is a source-and-evidence audit of the supplied package. It is not a complete penetration test or a live test on the owner's Windows laptop. Findings below distinguish observed release-run failures, source-level defects, documented test evidence, and untested risks.

---

## 1. Executive assessment

Speak Flow 0.4.3 is materially better than 0.4.1. The 0.4.1 review scored 5.4/10. The new release addresses much of the previous remediation list: the source now has a natural-prose default, a more capable smart-layout pipeline, meeting-library lifecycle operations, retention settings for meetings, live/near-live meeting transcription, correction learning, larger vocabulary/snippet packs, and expanded evidence validation. The UI is cohesive and the application has a real Rust core rather than being a thin web wrapper.

However, the evidence does not support calling this a 9/10 or 10/10 application yet. Two findings dominate this review:

1. **The English meeting release-pipeline run is materially corrupted.** The 87.1-second run repeats one sentence five times and omits three important utterances (hiring introduction, GST filing risk/deadline, and Rahul's GST-return task). Against the included reference script, a simple normalized token edit-distance calculation gives 22.1% WER (40 edits / 181 reference words). The exact WER may change with number normalization, but the omissions and repetition are obvious.
2. **The advanced AI endpoint check is bypassable.** `set_ai` checks custom HTTP URLs with `starts_with("http://localhost")`, then the provider client attaches the saved API key as a Bearer token. A crafted hostname such as `http://localhost.attacker.example` passes the check even though the actual host is external. This can expose the stored key over unencrypted HTTP if a user configures an untrusted endpoint.

Other important issues:

- Meeting summaries can render a decision as **Confirmed** while simultaneously adding “check this” because a required number is absent from the cited evidence.
- The correction-learning feature does not pin its watch to the original text element. It stores only the process ID and reads whichever UI Automation element is focused in that process. The documentation says “same text box”; the implementation does not enforce that.
- `CoInitializeEx` is called every four seconds by the correction watcher and is never balanced with `CoUninitialize`.
- The Instant Start privacy/help text says 0.3 seconds are retained in memory, while the implementation holds a one-second pre-roll buffer.
- The first-speech loss workaround is plausible, but its root cause remains unproven and its real use on the owner's current Windows profile is not demonstrated in this pack.
- Hinglish remains route-dependent: the documented local app-routed score is 41.4% WER on 60 synthetic clips; best reported specialised scores of 12.1–13.5% are on a 30-clip Heera subset with additional conversion/spelling stages. There is no scored real-voice benchmark.
- The current meeting test has serious recognition failures even though the app has 197 passing automated tests.

The application is not a failure. It is a genuine product with thoughtful implementation. But the release must be judged by what the user receives, not by how many Rust tests pass or how polished the screenshots look.

## 2. Scope, evidence and limits

### Materials reviewed

- Full v0.4.3 source archive: Rust workspace, Tauri desktop application, React/TypeScript UI, web/PWA, Android keyboard, tests, schemas, prompts and documentation.
- Production installer, supplied as two parts.
- Dark/light screenshots for the full UI and narrow Home layout.
- Two short UI walkthrough videos.
- Developer build/test logs, status report, known-bugs list, privacy and security notes, and diagnostics log.
- 30 English and 72 Hinglish audio clips with reference transcripts.
- Seven dictation release-pipeline runs and two meeting release-pipeline runs, including audio, JSON, raw transcript and final output.
- Laptop specifications.

### What was independently verified

- Reassembled `part0 + part1` into the installer. SHA-256 calculated independently:

  `B97E36F06F4B4FA0FC9F9D6953A26680FAFD5F717ABAB2D34E8D52B356F50E9B`

  This matches the hash in the review guide. It verifies file integrity against the supplied hash; it does not prove the installer is benign or that the program works.
- Inspected the source code and the line-level paths listed in this report.
- Inspected all seven dictation JSON outputs and both meeting JSON/Markdown outputs.
- Compared the English meeting transcript with the reference dialogue in `scripts/make-meeting.ps1`.
- Reviewed the screenshot contact sheet and both walkthrough frame sheets. The walkthroughs are UI/sample-data demos, not proof of actual live audio operation.
- Reviewed the documented test results: 197 passed, 0 failed, 2 ignored. These are developer-supplied results, not tests I re-ran.

### What I did not independently execute

This environment does not have `cargo`, `rustc`, `pnpm`, the Windows runtime, the owner's microphone, or the downloaded Whisper model/runtime. I therefore did **not** independently run `cargo test`, compile the Windows app, launch the installer, press the global hotkey, test the real microphone, run Whisper on the 102 supplied corpus clips, or reproduce Groq API calls. The raw and final outputs in the real-pipeline pack are the release-build results supplied for review.

The two screen recordings explicitly use sample data. The real speech evidence is in `10-real-pipeline-runs.zip`.

## 3. Scores

| Area | Score | Rationale |
|---|---:|---|
| English dictation accuracy | **8.0/10** | Good synthetic benchmark reported at 0.62% WER on 30 clips; no corrected real-voice reference set. |
| Hinglish dictation accuracy | **5.0/10** | The documented 90-clip synthetic benchmark shows 41.4% WER for the local app-routed route (36.0% tolerant). A specialised Large-v3 + conversion/rules/AI-spelling route reached 12.1% WER on only the Heera 30-clip subset; real-owner recordings remain unscored. |
| Dictation speed | **8.0/10** | Release runs show low-second latency for 6–25s clips; cloud request count and AI stages vary. Local GPU numbers are promising. |
| Smart layout / formatting | **7.0/10** | Mixed layouts work in several release runs, but meeting decisions are inconsistent and the word-preservation validator wrongly treats semantic words such as `will`, `we`, `our`, `is` and Hinglish `hai` as droppable connectors. |
| Meeting transcription | **4.0/10** | The English release run has repeated speech and missing content; Hinglish output has conspicuous phonetic errors. |
| Meeting summaries | **5.5/10** | Structure and evidence references are useful, but a contradicted “Confirmed / check this” decision is unsafe. |
| UI/UX and design | **8.5/10** | Consistent, attractive dark/light UI and good navigation. Screenshots and videos are sample-data renderings, not runtime proof. |
| Reliability | **5.5/10** | Main dictation flow is promising; meeting transcript regression and unverified mic cold-start remain serious. |
| Security and privacy | **6.0/10** | Good key storage and restricted Tauri capabilities, but endpoint validation is bypassable and correction learning is broader than documented. |
| Code quality | **7.5/10** | Clear modules, typed structures, tests and documentation; several lifecycle and validation defects remain. |
| Architecture | **7.5/10** | Sensible shared Rust core and separate UI, ASR, providers, learning and summary modules. Audio/meeting pipeline still needs stronger invariants. |
| Tests and test strategy | **7.0/10** | 197 passing tests, golden fixtures and real release-pipeline outputs; critical real-user workflows remain untested and one release-pipeline run fails badly. |
| Release readiness | **4.8/10** | Unsigned installer is a trust/distribution issue; meeting accuracy and API endpoint security need fixes before broad release. |
| **Overall** | **6.2/10** | Product judgement, not a mathematical average. Core dictation is credible; meeting reliability and security regressions prevent a higher score. |

### Why the overall score is 6.2, not 8 or 9

A dictation tool can be useful even when it occasionally needs correction. A meeting-notes tool that duplicates one person's task five times while omitting a compliance deadline and its owner can create operational harm. Your product promises both. The meeting pipeline therefore materially lowers the overall rating.

## 4. What improved since 0.4.1

The following improvements are real and worth keeping:

1. **Natural prose is now the default.** `ListFormat::Smart` and the new sentence-linking heuristics avoid the old “two sentences equals bullets” behaviour.
2. **Smart layout now understands mixed structures.** It can place an introductory paragraph, key points, numbered steps and a closing paragraph in one output. The project dictation in the release-pipeline pack is a good example.
3. **Meeting history exists.** The app now has a meeting library with reopen, transcribe, export and delete paths, addressing the earlier incomplete lifecycle.
4. **Meeting retention has a dedicated setting and lifecycle logic.** This is an improvement over 0.4.1, though it still needs device-level validation.
5. **Live/near-live transcript code exists.** The release-pipeline test records live and final segment counts and timing.
6. **Summary evidence checks are more advanced.** There are checks for referenced segment IDs, owners, deadlines, numbers, quotes, hedges and decision language.
7. **Correction learning, dictionary packs and snippets are implemented.** Pack tests verify 300+ unique words, 90+ snippets, no accidental personal-name insertion, and safe everyday-word handling.
8. **Credential storage is thoughtfully implemented.** Windows uses Credential Manager, Android uses Android Keystore, and the web app encrypts the stored key. The web key-removal implementation correctly deletes both IndexedDB records.
9. **The test suite is materially larger.** The supplied report records 197 passing tests, zero failures and two ignored tests.
10. **UI consistency has improved.** The screens show a clear product structure across Home, History, Meeting Notes, Scratchpad, Dictionary, Snippets, Style, Insights and Settings.

Do not discard this work. The right next step is targeted repair of the existing v0.4.3 codebase.

## 5. Release-pipeline results

### 5.1 Instant dictation runs

| Run | Input | Release output | Assessment |
|---|---|---|---|
| `a-prose` | 24.5s English explanation | One natural paragraph | Good. Connected explanation is preserved as prose. |
| `b-list` | 11.8s, explicitly enumerated three priorities | Three numbered items under an introduction | Good. |
| `c-bullets` | 6.3s, “Bullet point… Next point…” | Two `-` bullet lines | Semantically correct, but marker differs from smart layout's `•` marker. |
| `d-hinglish` | 17.5s Hinglish business explanation | Improved Roman Hinglish spelling | Useful correction, but AI cleanup rejected its candidate because it dropped `hamne`; rules fallback was used. Conservative fallback is safer than dropping speech, but shows the AI validation is sometimes too restrictive for Hinglish. |
| `meeting` | 21.9s launch discussion | Intro paragraph + “Next steps” numbered list | Partial. “Key decisions” remain inside a single paragraph instead of becoming a distinct decision list as requested. |
| `project` | 22.7s three-part explanation | Intro paragraph + numbered steps + closing paragraph | Good. This is the clearest evidence that mixed formatting can work. |
| `story` | 19.9s narrative | One natural paragraph | Good. It does not force bullets into a narrative. |

Measured recognition-stage totals in the logs for the seven runs range from approximately **0.52s to 2.88s**. These are timings for those specific cloud-backed runs, not a guaranteed user latency SLA. In English + Hinglish mode, multiple requests run in parallel and additional cleanup/layout calls may follow.

### 5.2 English meeting run — major failure

Files:

- Input audio: `runs/audio/meet-en-system.wav`
- Release output: `runs/meetings/meet-en.json`
- Reference speech: `scripts/make-meeting.ps1:5–18`

The reference contains 13 dialogue turns, including:

- Sales grew 12%.
- Gross margin fell from 41% to 38% because of freight costs.
- Marketing asked for Rs 50,000 subject to ROI remaining above 3x.
- Three financial-analyst interviews were pending.
- Priya would finish interviews by next Friday.
- GST deadline on the 20th, with a penalty if missed.
- Rahul would prepare the GST return by the 18th.
- Power BI dashboard readiness was unresolved.
- Follow-up review next Monday.

The v0.4.3 transcript instead contains:

- “Pria will finish the interviews by next Friday.” repeated **five times** between 00:45 and 01:05.
- No “Third, hiring” / three pending interviews statement.
- No GST deadline or penalty statement.
- No Rahul GST-return action item.
- “PowerBA” instead of “Power BI”.
- “Pria” instead of “Priya”.

I compared the combined expected script with the supplied transcript using a simple case-insensitive, punctuation-stripped word tokenizer and standard token edit distance. Result: **40 edits / 181 reference words = 22.1% WER**. This is an independently calculated, straightforward score; the number can vary if the benchmark normalizer treats digits, percentages and number words as equivalent. It does not change the substantive failure: one line is repeated five times and multiple complete statements disappear.

The v0.4.2 status file reports a prior 1.12% WER on a 179-word version of this meeting benchmark. This looks like a major regression, but it must be confirmed with a controlled A/B run using the exact same WAV, model, settings and normalization in 0.4.2 and 0.4.3. Do not assume the Whisper model alone regressed; the segmentation, live/final merge, input track, chunk offsets or self-test harness could be responsible.

#### Likely code weakness to investigate (not yet a proven root cause)

- `crates/asr/src/guard.rs:63–104` collapses repeated phrases inside a single string.
- `crates/asr/src/guard.rs:117–129` calls `collapse_repeats` separately for each ASR segment. It does not compare adjacent segment records from the same audio source.
- `apps/desktop/src-tauri/src/library.rs:80–88` appends live segments, and `:477–492` appends final segments and sorts them. There is no general same-source consecutive-duplicate guard between those stages.
- `crates/core/src/summary.rs:573–605` removes microphone segments that echo system audio; it deliberately leaves system-audio segments untouched. That is appropriate for cross-track echo, but it does not catch same-track repeated ASR results.

This explains why the current guard can allow repeated identical segment text through, but it does **not** prove why the recognizer emitted five segments for one utterance. The engineering team needs to inspect the actual chunk audio, word/segment timestamps, model output and the self-test's live/final merge. Do not blindly delete repeated phrases: people can legitimately repeat themselves. First determine whether the audio regions overlap or whether the ASR has hallucinated duplicated text.

### 5.3 Meeting summary decision consistency

In `meet-en.json`, the first decision is:

> Approve an additional Rs 50,000 for the festive marketing campaign, provided the ROI stays above 3x.

The evidence attached to this decision is only `s6`, which says “Agreed. Let us approve the 50,000 rupees with that condition.” The `3x` condition occurs in `s5`, not `s6`. The validator correctly adds a flag that the number 3 is missing from the cited segment. However, it leaves `status: confirmed` and the Markdown renders:

> **Confirmed:** ... provided the ROI stays above 3x. *(check this)*

That is internally contradictory. The summary writer should either cite both `s5` and `s6` and validate the condition, or downgrade the decision to Proposed/Needs review. A reader should never have to interpret “Confirmed” and “check this” at the same time.

Source: `crates/core/src/summary.rs:1016–1060`, particularly `:1027–1053`.

### 5.4 Mixed Hinglish meeting run

`meet-mixed.json` is useful because it demonstrates the current Roman Hinglish path under mixed input. It also demonstrates why the system is not ready to claim robust Hinglish meeting transcription. Examples in the actual output include:

- `Is Maheen ka budget toda tight hai? Kharcha kam karna paghlega.`
- `Paati toh banti hai?`
- `Doctor ne bola hai ki do din araam kana hai.`
- `Humori train Shamsath Baje ki hai, station time P.E. Pahyunchna.`
- `Mane Naya laptop order ki hai, kal deliver hoga.`

These are visibly malformed spellings/recognitions. The supplied run does not include a matching exact reference dialogue for this mixed meeting, so I will **not** invent a WER score. The summary turns the Maheen sentence into “budget is tight, requiring expense reduction”; that may be what the speaker intended, but the transcript is too uncertain to present that interpretation without a review marker.

### 5.5 Timing / “live” meeting transcript

The 87-second English run reports 11 live lines, three final lines, 2.442s live ASR time, 0.770s final ASR time and 3.254s notes time. The 64-second mixed run reports eight live lines, four final lines, 1.648s live, 0.877s final and 3.111s notes.

However, `apps/desktop/src-tauri/src/meeting.rs:496–510` deliberately waits until at least **25 seconds of new audio** exists before processing a live chunk. `library.rs:72–88` polls the controller every 10 seconds. That means the first transcript update cannot appear immediately; a user may wait roughly 25 seconds plus polling and transcription time. This is near-live chunking, not word-by-word streaming. The UI should set expectations honestly and never show “Live” as if it were instant.

The transcript labels `System audio` and `Microphone` identify the captured audio sources, not real speakers. The privacy documentation correctly says this. However, the meeting overview currently puts `System audio` in the `Participants` list. That is a category error: an audio source is not a participant. Show “Audio sources captured” separately and mark actual participants as “Not identified” until diarization is available.

## 6. Accuracy: English and Hinglish

### English

The latest documented synthetic result is **0.62% WER on 30 English clips**, with 322 reference words. That is promising. However:

- The set is synthetic Windows TTS, not spontaneous speech.
- The current pack does not include a new, independently scored 0.4.3 English corpus run.
- The owner's real voice recordings do not have corrected transcripts, so no real-voice WER can be calculated.
- The 87-second release meeting transcript is a serious counterexample to relying on the dictation score alone.

**Assessment:** strong synthetic dictation baseline, insufficient real-world validation, and the meeting pipeline has a serious regression or test-harness defect.

### Hinglish

The available status results report approximately **20.03% final WER** for the 60-clip synthetic set A (Roman Hinglish; a tolerant spelling metric is also reported at 17.68%), and **24.24%** on the additional 12-clip set B. The latest 0.4.3 route fixes some language-routing mistakes on 12 of the owner's recordings, but there is no corrected ground truth for the owner's voice and no real-voice WER.

The status also records that seven of 60 Hinglish clips in an earlier local routing route were treated as English, and that the 0.4.3 route uses two parallel recognition requests plus English rescue when the no-fixed-language output looks plain English. That approach is understandable, but it increases duplicate audio uploads and request consumption.

**Assessment:** better than the earlier 41.4% local-only route, but roughly 20% synthetic WER remains too high for reliable transcription. Do not describe Hinglish as “high accuracy” until corrected natural-speech tests confirm it.

### Important benchmark gap

The pack contains 30 English and 72 Hinglish WAVs, while the status benchmark reports 30 English + 60 Hinglish clips from a separate 90-clip set and additional 30-clip Heera route comparisons. It does not publish one complete, per-clip, final-release-route WER table for all 102 files supplied in this review pack. The reported figures vary materially by route: local app-routed Hinglish 41.4% WER (36.0% tolerant); local + rules 35.4%; Heera subset with AI spelling 23.2%; Groq cloud + conversion + spelling 13.5%; Large-v3 + conversion + rules + spelling 12.1% on the Heera 30 subset (9.4% tolerant). Large-v3 offline-only wrote Devanagari on 47/60 clips. Ask the developer to run the exact v0.4.3 release build against all 72 Hinglish files and 30 English files, preserve per-clip raw/final outputs, use one documented normalizer, and produce separate results by route, speaker and difficulty. Do not confuse the specialised best subset result with default-route accuracy.

## 7. Source-code findings

Severity definitions:

- **P0:** immediate stop-ship issue or severe data/security compromise.
- **P1:** must fix before broad release; meaningfully impacts security, correctness or data integrity.
- **P2:** important reliability, privacy or UX defect; fix in the next release.
- **P3:** polish, maintainability or future capability.

### P1 — Custom endpoint validation can expose the saved API key

**Files:**
- `apps/desktop/src-tauri/src/lib.rs:653–659`
- `apps/desktop/src-tauri/src/providers.rs:388–395`

The code permits a custom URL if it starts with `https://`, `http://localhost`, or `http://127.0.0.1`. This is a string-prefix check, not a parsed hostname check. The provider request then adds `Authorization: Bearer <saved-key>`.

Examples that begin with the permitted prefix but are not loopback hosts:

- `http://localhost.attacker.example/v1`
- `http://localhost@attacker.example/v1` (the actual host is `attacker.example`)
- `http://127.0.0.1.attacker.example/v1`

A user who pastes a crafted/custom endpoint can cause the app to send its stored provider credential to an external server over plain HTTP. This requires endpoint configuration; it is not a zero-click remote exploit. It is nevertheless a real security validation defect because the code explicitly intends to allow HTTP only for local services.

**Fix:** parse with a URL library. For `http`, require the parsed host to be exactly `localhost` or an IP address for which `is_loopback()` is true (`127.0.0.1`, `::1`). Reject user-info, unexpected query/fragment, and malformed URLs. Keep HTTPS validation separate. Disable automatic redirects for credential-bearing requests or validate every redirect destination before forwarding authentication. Add regression tests for the malicious examples above.

### P1 — Meeting transcript has no cross-segment repetition guard

**Files:**
- `crates/asr/src/guard.rs:63–104, 117–129`
- `apps/desktop/src-tauri/src/library.rs:80–88, 477–492`
- `apps/desktop/src-tauri/src/controller.rs:833–895`

`collapse_repeats` operates on a single string. `kept_segments` calls it per segment. The live/final meeting pipeline merges and sorts segment records but does not run a same-source repeated-segment/overlapping-chunk check. The actual release run has the same sentence at five adjacent timestamps.

**Fix:** instrument the recognizer's chunk boundaries and compare audio ranges, segment offsets and normalized text across adjacent pieces. Detect exact or near-exact duplicate segment runs from the same track, especially when their timestamps imply overlap/reprocessing. Do not blindly deduplicate genuinely repeated speech. Mark suspicious duplicates as uncertain or exclude only when overlap/retry provenance proves duplication. Add an exact fixture derived from `meet-en`.

### P1 — Meeting decision can be both Confirmed and “check this”

**File:** `crates/core/src/summary.rs:1016–1060`

The number/evidence assessment can produce `check = NeedsReview` and a note, while the code only downgrades a `Confirmed` decision if the cited transcript contains explicit disagreement, hedging or no agreement words. It does not downgrade merely because a required number is missing from the cited evidence. The result is rendered as Confirmed plus a warning.

**Fix:** if the evidence assessment flags a material mismatch (amount, ratio, date, owner, condition, contradiction or unsupported claim), never render `Confirmed`. Either attach all transcript segments that support each part of the claim and pass the validator, or downgrade to `Proposed`/`Needs review`. Ensure the Markdown renderer cannot display the contradictory combination.

### P1 — Smart-layout validator can accept meaning-changing word deletion

**File:** `crates/core/src/layout.rs:21–83, 142–190`

The module comment claims that only list connectors may be dropped, but the `CONNECTORS` allow-list includes words that carry essential meaning: `are`, `is`, `were`, `was`, `will`, `be`, `our`, `my`, `we`, `have`, `has`, `that`, `which`, plus Hinglish words such as `hai`, `hain`, `mein` and `toh`. The acceptance validator then permits up to three dropped connector words unconditionally; beyond that, it rejects only when both the count exceeds three and the share exceeds 12%.

This is not a safe preservation rule. Removing `we` or `our` can change ownership or responsibility; removing `will` can change a commitment; removing `was`/`is` can change tense or status; dropping `that`/`which` can alter clause structure; removing `hai`/`hain` can damage Hinglish grammar. The supplied release runs do not demonstrate that this defect has already changed a user's meaning, so this is a **source-level semantic-integrity risk**, not an observed corrupt output. But the validator's guarantee is weaker than its comments claim.

**Fix:** Replace the broad connector list with a tiny, explicitly justified set of truly disposable spoken list-introducers (for example, only ordinal/list scaffolding when it is replaced by a list marker). Never permit deletion of negation, subjects, pronouns, possessives, auxiliaries, modal verbs, tense markers, conditions, names, numbers, dates, amounts, or meaning-bearing Hinglish words. Prefer a typed content-block transformation that retains the original sentence text verbatim, rather than dropping words to create layout. Add adversarial English and Roman-Hinglish tests where deleting one such token must reject the layout.

### P1/P2 — Correction learning does not guarantee it reads the same text box

**Files:**
- `apps/desktop/src-tauri/src/learn_watch.rs:28–43, 47–73`
- `apps/desktop/src-tauri/src/controller.rs:374–382, 1484–1488`
- UI copy: `apps/desktop/src/SettingsPage.tsx:411–418`

`Watch` stores a process ID, inserted text and timing, but not the original window handle, UI Automation element runtime ID, AutomationId or control identity. Each poll calls `GetFocusedElement()` and verifies only that its process ID matches. If the user moves focus to another text field in the same process, the watcher can read that other element. It calls `DocumentRange().GetText(20_000)`, which can return a large portion of a document/editor, not just the dictated phrase.

The feature is described in the privacy document/UI as reading “the same text box”. The current implementation enforces “some focused element in the same process”, which is weaker. The full text is not stored or sent according to the code/docs; however, it is read transiently and can be the wrong field. This is a privacy-scope issue and could also learn an incorrect word pair if matching text exists elsewhere.

**Fix:** capture the original target's HWND and UIA element identity at insertion time. Re-check both on every poll, and stop if the element identity changes. Prefer tracking the inserted range or exact text selection if supported. Read the smallest relevant range, not a 20,000-character full document. If the target app cannot expose a stable identity, disable learning for that app and show a note. Consider making learning opt-in rather than on by default until this is corrected.

### P2 — COM initialization is not balanced

**File:** `apps/desktop/src-tauri/src/learn_watch.rs:47–52`

`focused_text()` calls `CoInitializeEx` every four seconds and discards the result. It never calls `CoUninitialize`. On a thread where initialization succeeds repeatedly, COM initialization increments its per-thread count; each successful call should be balanced. `RPC_E_CHANGED_MODE` is also silently ignored.

**Fix:** initialize COM once on the watcher/controller thread using a scoped RAII guard and uninitialize once at thread exit, or balance every successful call. Handle `S_FALSE` and `RPC_E_CHANGED_MODE` explicitly. Add a repeated-poll integration test.

### P2 — Instant Start privacy copy understates the audio pre-roll

**Files:**
- `apps/desktop/src/SettingsPage.tsx:187–188`
- `apps/desktop/src-tauri/src/audio.rs:28–30, 120–132, 144–159`

Settings says “Only the last 0.3 s is kept in memory”. The implementation defines `PRE_ROLL_MS = 1000` and allocates a one-second ring buffer. The mismatch is visible in the Settings screen.

**Fix:** either reduce the actual buffer to 300ms if it still captures the user's first syllable, or update the text to say “up to 1 second”. Document that this is an in-memory pre-roll only, not saved audio. Ensure the retention is bounded and cleared after dictation.

### P2 — First-word loss is not yet proven fixed in real usage

**Files:** `apps/desktop/src-tauri/src/audio.rs:28–38, 113–159, 197–215, 303–363`; ignored test around `:469–476`.

The watchdog/reopen/pre-roll/“Mic starting…” change is a sensible response to 10 of 12 recordings starting with 2.2–4.0 seconds of digital zeros. But the status explicitly says the Windows driver cause is unknown, and the real mic-start latency test is ignored. A one-second ring is shorter than the prior 2.2–4.0-second zero window; the intended protection is that the app waits for a usable stream and signals the user before speech begins. This must be proved in actual use.

The `last_sound` detector treats any non-zero sample as sound (`audio.rs:144–150`), which can be triggered by low-level noise. It is not a voice detector. Consider a sustained RMS/noise-floor test rather than exact nonzero samples, while preserving quiet speech.

**Fix:** run 100 cold starts, 100 warm starts, several microphone devices, muted/unmuted transitions, Bluetooth connect/disconnect and app contention. Record when the key is pressed, when audio stream opens, first nonzero sample, first above-noise-floor sample and actual first word. Don't claim “first word never cut off” until these tests pass.

### P2 — Two bullet-marker formats remain

**Files:**
- `crates/core/src/pipeline/structure.rs:163–179`
- `crates/core/src/layout.rs:198–232`
- Release outputs: `runs/dictation/c-bullets.json` and `runs/dictation/meeting.json`

Rule-based spoken commands produce `- ` bullets. Smart layout normalizes markers to `• `. They are semantically equivalent but visually inconsistent in the same app. More importantly, the `layout` normalizer is not run on text that already contains line breaks, so spoken-list paths can bypass its marker normalization.

**Fix:** use one canonical internal list-block format and one renderer. Do not rely on regex-based string cleanup for different pipeline paths.

### P2 — Smart layout does not reliably create the expected meeting decisions list

**Files:** `apps/desktop/src-tauri/src/controller.rs:1430–1436`; `crates/core/src/layout.rs:101–109, 142–190`; `spec/prompts/layout.md`; release output `runs/dictation/meeting.json`.

The meeting-style dictation in the actual release run keeps “The key decisions were that we will launch…” as one prose sentence and only formats “Next steps are” as a numbered list. Your reference image expects a paragraph followed by a separate “Key Decisions” section with distinct bullets and then next steps. The `layout-eval` status reports an earlier successful test, but the actual release run is inconsistent. The smart layout threshold (3+ sentences or 40+ words) and word-preservation validator may also reject otherwise useful layout proposals.

**Fix:** create deterministic golden fixtures for paragraphs + key decision bullets + action lists in English and Hinglish. Verify the exact release pipeline, not only a layout-eval prompt. If the model proposal is rejected, keep prose and surface the reason in diagnostics; do not silently claim the layout passed.

### P2 — Meeting live transcript is delayed by the 25-second chunk gate

**Files:**
- `apps/desktop/src-tauri/src/meeting.rs:496–510`
- `apps/desktop/src-tauri/src/library.rs:72–88`

`pieces_to_take()` returns no live work until at least 25 seconds of new audio exists. The poll loop checks every 10 seconds. A short meeting may have no live transcript until Stop. Longer meetings can wait roughly 25 seconds plus poll and recognition time for the first update.

**Fix:** either label the feature “near-live, typically updates after a chunk” or reduce the chunk threshold and use overlap-aware incremental transcription. Test latency and duplicates at 10s, 20s, 30s, 60s and 10-minute boundaries.

### P2 — Meeting overview calls audio sources “Participants”

**Files:** inspect meeting view construction and `crates/core/src/summary.rs`; observed in `runs/meetings/meet-en.json` and `meet-mixed.json`.

The JSON has `participants: ["System audio"]` and the Markdown renders “Participants: System audio”. The privacy document correctly explains that “Microphone” and “System audio” are input sources, not people. The UI/summary should not label an audio source as a participant.

**Fix:** separate `audio_sources` from `participants`. Use “Participants: Not identified” until reliable speaker diarization exists.

### P2 — Hinglish polish falls back on common code-switched speech

**Files:** `runs/dictation/d-hinglish.json`; `logs/diagnostics-log-from-test-runs.txt`; cleanup validator in `crates/core/src/llm.rs` (search for the dropped-word acceptance check).

The raw text has `hamne`, but the AI cleanup proposal was rejected for dropping that word. It falls back to the deterministic rules, preserving the word but leaving some grammar/spelling imperfections. This is safer than dropping meaningful content, but indicates that the current validator does not sufficiently distinguish a meaningful word from a legitimate transliteration/cleanup change in mixed-language text.

**Fix:** create a Hinglish-specific tokenization and preservation policy with Roman-script variants, while preserving numbers, names, negation, modality, dates and important verbs. Every accepted cleanup must keep all meaningful content; every fallback must be logged as a non-content diagnostic reason.

### P2 — Android recorder stop has a possible thread/release race

**File:** `apps/android/app/src/main/java/app/voice/Recorder.kt:97–117`

The code joins for two seconds, interrupts and joins for another 500ms if still alive, but then releases the `AudioRecord` regardless of whether the reader thread is still alive. The docstring states the reading thread has finished before release, but the implementation does not enforce that postcondition.

**Fix:** do not release/copy while the reader thread remains alive. Use a cancellable read strategy or a bounded shutdown state that surfaces a failure instead of proceeding with a possibly active thread. Add instrumentation/integration tests on a physical Android device.

### P3 — Hotkey hook callback takes a blocking mutex

**File:** `apps/desktop/src-tauri/src/hotkey/mod.rs:108–141`, especially `:123–130`.

The low-level keyboard hook callback takes a blocking mutex. If another thread holds this lock longer than expected, the callback can stall and Windows may silently remove a low-level hook that exceeds its timeout. The state-machine work appears small, so this is a reliability hardening issue rather than a proven failure.

**Fix:** keep the hook callback constant-time; use an atomic state machine or `try_lock` with a safe pass-through fallback. Never perform blocking work or network operations in the hook callback.

### P3 — No global local-only network kill switch

**Files:** Settings/privacy architecture; `docs/SECURITY_CONTROLS.md` explicitly lists this as not built.

The app supports local Whisper and a “none” AI provider, but does not offer an enforceable setting that blocks all network calls (ASR, cleanup, updater and model download) while enabled.

**Fix:** add an explicit “Local-only mode” that disables cloud ASR and AI, prevents upload calls at the provider boundary, and shows an unmistakable status. Make the local-only choice auditable in settings and logs.

### P3 — Unsigned installer

**Finding:** the installer is unsigned; this is documented. It is not a vulnerability in the code and the supplied hash matches. Windows SmartScreen can still warn, which reduces user trust and complicates distribution.

**Fix:** consider code signing when distributing outside a small trusted test group. Until then, publish SHA-256, source tag/commit, build provenance and release notes for every binary.

## 8. Security and privacy assessment

### Good controls worth keeping

- Desktop API keys are stored in Windows Credential Manager rather than SQLite or plaintext configuration files.
- Android uses Android Keystore.
- PWA keys are encrypted with AES-GCM, with the non-extractable device-key limitation explicitly documented. `forgetKey()` correctly deletes both the encrypted Groq record and device-key record (`apps/web/src/keystore.ts:26–35, 131–134`).
- Tauri capabilities are narrow: the WebView does not receive generic file, shell or network access.
- Diagnostics redact key-shaped values and do not include dictation text.
- The app records user consent before cloud text cleanup and separates cloud ASR audio consent from AI cleanup text consent.
- The privacy documentation explains that English + Hinglish cloud mode sends two parallel audio-recognition requests and can send a third rescue request.
- Meeting audio is described as local-only; only meeting transcript text is sent to the configured AI provider for notes.
- Retention logic is restricted to files inside the app data directory.

### Weaknesses / risks

1. **Custom endpoint URL validation bypass** — P1, detailed above.
2. **Correction learning reads a process-focused UIA element rather than a pinned element** — P1/P2 privacy-scope mismatch.
3. **Cloud request fan-out** — English + Hinglish mode may send the audio twice in parallel and once more for rescue. AI spelling, cleanup and smart layout can add more text requests. This is disclosed in the privacy docs but should be made conspicuous at the point of consent, including the possibility of multiple audio uploads, costs and provider retention. A user should not assume one dictation equals one upload.
4. **No enforced local-only network switch** — a user who wants zero network traffic has to rely on individual settings/provider selection.
5. **No independent penetration test or security certification** — the project correctly states that it has no SOC 2, ISO 27001 or HIPAA certification. Do not claim compliance.
6. **No code signing** — distribution/trust risk, not proof of maliciousness.
7. **Correction-learning default** — enabled by default per the documentation. Because the watcher is broader than its “same text box” claim, make it opt-in or fix the scope before enabling by default.

### Security items I did not find evidence of

I did not establish a confirmed remote-code-execution path, secret hardcoded in the checked source, or direct PWA IndexedDB key-deletion bug. The web key removal now deletes both records. This is not a penetration-test certificate; it only means those specific issues were not found in the inspected paths.

## 9. UI/UX and product assessment

### What works

- The application has a clear sidebar and product areas: Home, Insights, History, Scratchpad, Meeting Notes, Dictionary, Snippets, Style, How to use and Settings.
- Dark and light themes are consistent and readable.
- Settings are grouped logically (General, Speech, Groq & AI, Writing, Privacy & data, About & updates).
- Meeting setup makes the microphone, system audio and participant-consent switch visible.
- The meeting library screen exposes reopen/retranscribe/export/delete workflows in the UI walkthrough.
- Paragraph, point and mixed-layout support are now described in settings.

### What is weak

- The screen captures and walkthrough videos use sample data. Do not use the charts, history counts or meeting output as proof of real usage.
- The sample meeting overview labels “System audio” as a participant.
- The Instant Start setting has inaccurate 0.3s memory copy.
- The live transcript experience is actually chunked with a minimum 25-second gate; the product needs to communicate that honestly.
- The UI does not yet show evidence that a meeting's transcript is trustworthy. Important numbers, names and tasks need an inline review/uncertainty experience.
- The meeting interface should make it effortless to correct transcript lines and regenerate only affected summary sections. The walkthrough shows a corrected line state, which is promising, but a live manual correction workflow was not tested.

## 10. Comparison with major alternatives

This is a feature/documentation comparison using current official product material. I have not run the competitor apps side by side on the same laptop and audio corpus, so I am not making comparative accuracy claims.

### Wispr Flow

Wispr Flow documents cross-app dictation, fillers and self-corrections, dictionary/snippets, context-aware formatting and a Windows Notetaker. The Notetaker docs describe live transcripts, speaker labels, typed personal notes, editable saved meetings, summaries and meeting search/follow-up capabilities. Speak Flow has a useful local-first option and Roman Hinglish focus, but it is behind on complete meeting transcript reliability, speaker identity and cross-meeting retrieval/action workflows. Sources: [Wispr Flow features](https://wisprflow.ai/features), [Notetaker Windows announcement](https://wisprflow.ai/post/notetaker-on-windows), [live transcript documentation](https://docs.wisprflow.ai/articles/4091106439-the-live-transcript-in-notetaker-beta), [meeting notes editor](https://docs.wisprflow.ai/articles/9406970664-Meeting-notes-and-the-editor-in-Notetaker).

### Typeless

Typeless documents natural speech-to-polished writing, personalization, dictionary learning, bulk dictionary import, and thought reorganization. Speak Flow has custom packs and correction-learning code, but Typeless provides a more mature personalization narrative and a stronger “organize my thoughts” interaction. Speak Flow should focus on faithful mixed formatting rather than trying to make every sentence sound AI-rewritten. Sources: [Typeless Windows release notes](https://www.typeless.com/help/release-notes/windows), [personalization](https://www.typeless.com/help/quickstart/personalization), [dictionary/history](https://www.typeless.com/help/quickstart/history-and-dictionary).

### Superwhisper

Superwhisper documents Windows dictation in any app, offline local models, context-aware formatting and file transcription. Speak Flow's differentiator is a targeted English/Roman-Hinglish pipeline and its meeting-note plus correction-learning feature set. Superwhisper is a useful benchmark for local model selection, app-context modes and file transcription. Source: [Superwhisper for Windows](https://superwhisper.com/windows).

### Windows Voice Typing / Voice Access

Windows Voice Typing uses Win+H and online speech recognition; Windows Voice Access supports voice dictation and PC control offline on supported Windows 11 systems. Speak Flow must offer more than a custom hotkey to justify installation: reliable Roman Hinglish, vocabulary, cleanup, structured output, app-aware formatting and recoverable meeting notes. Sources: [Windows Voice Typing](https://support.microsoft.com/en-us/accessibility/windows/use-voice-typing-to-talk-instead-of-type-on-your-pc), [Voice Access text dictation](https://support.microsoft.com/en-us/accessibility/windows/voice-access/dictate-text-with-voice).

### Product gaps worth considering later

- True speaker diarization (not just Microphone/System audio source labels).
- Meeting-wide search and “ask across my meetings”.
- Follow-up email/task drafting grounded in transcript evidence.
- Import and transcribe existing audio/video files.
- Better per-app context for writing style, names and surrounding text, with an explicit privacy control.
- Word-level or reliable phrase-level confidence/review for numbers, names, dates and action owners.
- More robust model fallback / local-only mode.

Do not build these before the current meeting regression and API endpoint issue are fixed.

## 11. Prioritised remediation roadmap

### P0 — Stop-ship before next public release

1. Fix and test the custom `base_url` host validation; prove that malformed `localhost` prefixes cannot receive a saved Bearer key over HTTP.
2. Reproduce the meeting duplicate/missing-content failure from the exact supplied WAV, trace live/final chunking and segment offsets, and fix it. Add the exact meeting audio as a regression fixture if licensing permits; otherwise store the expected segments/transcript fixture and keep the audio private.
3. Fix the Confirmed + Check this decision conflict. A decision with unsupported amount/condition/owner/deadline must not be rendered as confirmed.

### P1 — Required for a reliable 0.4.4

4. Pin correction learning to the actual original UIA element and balance COM init/uninit. If stable element identity is unavailable, stop learning for that control.
5. Produce a fresh v0.4.3/v0.4.4 English + Hinglish benchmark with all 102 synthetic clips, per-clip outputs and standard normalization. Keep real speech separate and get exact transcripts for a small consented set of owner recordings.
6. Add cross-segment repeated-speech detection that is timestamp/audio aware and does not delete legitimate repetitions.
7. Improve Hinglish cleanup/recognition without translating into English or silently guessing garbled words. Preserve uncertainty where the recognizer is unsure.
8. Verify first-word capture with real hardware: cold starts, warm starts, mute/unmute, Bluetooth and competing microphone apps. Fix the 0.3s vs 1.0s privacy copy.
9. Stabilize smart-layout rules: paragraph + key decisions bullets + numbered next steps must pass as an exact golden output in both English and Roman Hinglish; unify bullet markers.
10. Fix the `CONNECTORS` allow-list in `crates/core/src/layout.rs:21–83` and acceptance threshold at `:142–190`; never permit deletion of subjects, pronouns, possessives, auxiliaries, modals, tense, conditions or meaning-bearing Hinglish tokens. Add adversarial token-preservation tests.
11. Separate audio-source labels from participant names in meeting UI and summaries.

### P2 — Reliability and product quality

12. Reduce or honestly label live transcript latency (25s minimum new audio plus polling); test 10s/30s/60s and long sessions.
13. Add device unplug/reconnect, sample-rate change, microphone permission loss, system-audio device change, and disk-full handling tests.
14. Fix Android recorder shutdown postcondition and add physical-device test evidence.
15. Keep hotkey callback non-blocking.
16. Add visible upload count / data-route disclosure before cloud mode; explain two/three audio requests, text cleanup, layout and Hinglish spelling calls.
16. Add a global local-only mode.
17. Sign the installer for wider distribution when practical.

### P3 — Future differentiation

18. Speaker diarization.
19. Search and Q&A across meetings.
20. Audio/video file transcription.
21. Per-app context and style with privacy controls.
22. Improved export, edit and review workflows.

## 12. Acceptance criteria for 0.4.4

A release must not be marked complete just because unit tests pass. It must demonstrate:

### Security
- `http://localhost.attacker.example`, `http://localhost@attacker.example`, `http://127.0.0.1.attacker.example`, IPv6 loopback and valid loopback endpoints have explicit test outcomes.
- API credentials are never sent to unapproved hosts or redirected hosts.
- Correction learning reads only the same pinned text element and stops when focus changes.
- COM initialization is balanced.

### English and Hinglish
- Run every supplied 30 English + 72 Hinglish clip through the exact release build.
- Save raw transcript, final transcript, model/route, request count, per-clip latency and error diagnostics.
- Report English and Hinglish WER/CER separately, and use one disclosed normalizer.
- Add a corrected natural-speech reference set; never report real speech accuracy without ground truth.
- Preserve Roman Hinglish. Do not translate Hindi phrases into English unless translation is explicitly requested.

### Dictation layout
- One continuous explanation stays a paragraph.
- A genuine spoken list becomes bullets or numbered items.
- A meeting-style mixed dictation yields paragraph + “Key decisions” bullets + numbered “Next steps” + closing paragraph when those labels/content were spoken.
- A story remains natural prose.
- No content word, number, name, date, negation or condition is silently dropped.
- All list paths use one canonical marker/style renderer.

### Meeting pipeline
- Re-run the 87-second English meeting; no sentence may be duplicated unless it was actually spoken more than once.
- The “Third, hiring” discussion, three interviews, Priya's deadline, GST deadline/penalty and Rahul's return task must all be preserved.
- Confirm WER against the reference script with number normalization documented.
- The Rs 50,000 / ROI >3x decision must cite the condition segment and approval segment; it must not be “Confirmed” if evidence is incomplete.
- Mixed Hinglish transcript must be manually checked against an exact reference script.
- Meeting UI labels audio sources and participants correctly.
- 60-minute recording, pause/resume, interruption recovery and device unplug/reconnect must be tested on Windows.

### Live transcript and performance
- Record exact time-to-first-live-text. A 25-second gate must be visible in the product description or reduced.
- No missing/duplicated sample ranges between live and final passes.
- Report GPU/CPU, RAM, model, startup and warm-run timings on the HP Victus.

### Release evidence
- `cargo fmt`, `cargo clippy`, `cargo test`, TypeScript checks and `tauri build` pass.
- Report skipped/ignored tests honestly.
- Install the actual release build on the owner's real Windows profile and test hotkeys, Notepad, Word, Gmail/Chrome, VS Code and Meeting Notes.
- Provide test logs and checksums. Do not report unrun tests as passed.

## 13. Final release decision

**Do not treat 0.4.3 as a finished public product.** It is a substantial beta and a good foundation. The next release should focus on correctness, security and test evidence—not additional feature count.

My central recommendation is simple:

> First make sure the app never loses the beginning of a sentence, never fabricates or repeats meeting content, never mislabels an unverified decision as confirmed, and never sends an API key to the wrong endpoint. Then polish everything else.

The path to 10/10 is not more screens. It is measured, reproducible reliability across real English and Roman-Hinglish speech, with meeting notes that can safely be acted upon.

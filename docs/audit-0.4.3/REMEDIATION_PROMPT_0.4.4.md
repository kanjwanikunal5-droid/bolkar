# MASTER REMEDIATION PROMPT — SPEAK FLOW 0.4.4

## Objective

You are working inside the **existing Speak Flow v0.4.3 codebase** (Git tag `v0.4.3`, commit reported as `98c4751`). Your task is to fix the verified defects in the independent v0.4.3 audit, improve the current application, run meaningful tests, and prepare a safe **v0.4.4** release.

**Do not rebuild the application from scratch. Do not replace the current Rust/Tauri/React architecture merely for preference. Do not discard the owner's existing work.** Preserve the existing product, data, settings, recordings, models, API credentials, application identity and user experience unless a change is required to fix a defect.

The app supports only **English and Hinglish (Roman-script Hinglish)** by product decision. Do not add unrelated language support. Do not automatically translate Hinglish into English or Devanagari.

The owner has a Windows 11 HP Victus laptop with Ryzen 7 7445HS, 16 GB RAM and an NVIDIA RTX 4050 laptop GPU. Optimize for this actual machine, but maintain safe CPU fallback.

This is a remediation task, not a feature-count competition. Correctness, data integrity, privacy and trust come first.

---

# 0. REQUIRED SAFE WORKFLOW

Before changing code:

1. Inspect the repository, current Git status, current branch, current tag and exact commit. Confirm this is the expected v0.4.3 source.
2. Create a dedicated branch such as `fix/v0.4.4-audit-remediation`.
3. If Git history is unavailable, create a complete timestamped source backup outside the project directory.
4. Create `AUDIT_REMEDIATION_PLAN.md` mapping each finding to source files, planned changes, tests and release criteria.
5. Read the complete previous audit report supplied alongside this prompt. Do not rely only on this prompt's summary.
6. Do not overwrite user data, `%APPDATA%` data folders, SQLite databases, recordings, downloaded models, or Credential Manager entries during development tests.
7. Use a separate throwaway app data directory for all self-tests.
8. Never use the owner's real API key in unit tests or logs. Use a local fake server or a test-only key.
9. Do not install or replace the owner’s current production application without explicit permission. Build the installer and provide an installation/test checklist.
10. Never report a test passed if it was not actually executed. Clearly separate automated tests, release-build self-tests, and manual Windows tests.

Preserve the internal application identifier `app.bolkar.voice` unless an intentional, fully migrated and backward-compatible identifier change is separately justified. The product branding can say Speak Flow while the data directory/keychain namespace remains unchanged for compatibility.

---

# 1. P0 SECURITY FIX — CUSTOM PROVIDER URL VALIDATION

## Problem

Inspect:
- `apps/desktop/src-tauri/src/lib.rs` around lines 653–675 (`set_ai`)
- `apps/desktop/src-tauri/src/providers.rs` around lines 378–405 (`chat_once`)

The current code checks custom URLs with string-prefix tests equivalent to:

- `starts_with("https://")`
- `starts_with("http://localhost")`
- `starts_with("http://127.0.0.1")`

The request client then adds the saved API key as a Bearer token.

This incorrectly allows URLs such as:

- `http://localhost.attacker.example/v1`
- `http://localhost@attacker.example/v1`
- `http://127.0.0.1.attacker.example/v1`

These are external hosts, not loopback. The app can therefore send a stored API key over cleartext HTTP to an unapproved server if a user configures a crafted endpoint.

## Required fix

Use a real URL parser. Do not validate URLs using `starts_with`, string splitting or substring matching.

Rules:

1. For `https`, accept a syntactically valid HTTPS URL subject to the provider's normal hostname/certificate verification.
2. For `http`, accept only when the parsed host is exactly `localhost` (case-insensitive, with optional port) or an IP address for which the standard IP library returns `is_loopback() == true`.
3. Correctly handle IPv4 and IPv6 loopback, including `[::1]`.
4. Reject user-info (`username:password@host`), fragments and malformed URLs. Define clearly whether query parameters are allowed; safest default is reject unless the provider requires them.
5. Do not accept `localhost.example`, `localhost@evil.example`, or any hostname that merely begins with `localhost`.
6. Disable redirects on credential-bearing requests, or manually follow redirects only after revalidating the destination and ensuring credentials are not forwarded to a different origin.
7. Never include a full URL containing secrets in diagnostics.

## Required tests

Add tests proving:

- `https://api.groq.com/openai/v1` is accepted.
- `http://localhost:11434/v1` is accepted.
- `http://127.0.0.1:11434/v1` is accepted.
- `http://[::1]:11434/v1` is accepted.
- `http://localhost.attacker.example/v1` is rejected.
- `http://localhost@attacker.example/v1` is rejected.
- `http://127.0.0.1.attacker.example/v1` is rejected.
- `http://192.168.1.10:11434/v1` is rejected unless there is a separate explicit trusted-network configuration (do not silently classify LAN as local).
- Invalid schemes and malformed URLs are rejected.
- A redirect from an allowed HTTPS origin to an untrusted origin cannot receive the Bearer token.

This is a stop-ship issue. Do not mark the release safe until these tests pass.

---

# 2. P0 MEETING TRANSCRIPT FAILURE — REPRODUCE AND FIX BEFORE OTHER MEETING POLISH

## Actual failing evidence

The v0.4.3 release run `runs/meetings/meet-en.json` for an 87.094-second English meeting contains the same line:

`Pria will finish the interviews by next Friday.`

five times, and omits:

- “Third, hiring. We have three interviews pending for the financial analyst role.”
- the GST filing deadline on the 20th and penalty risk
- “Rahul will prepare the GST return by the 18th.”

It also writes `Pria` for `Priya` and `PowerBA` for `Power BI`.

Against `scripts/make-meeting.ps1` lines 5–18, a simple case-insensitive, punctuation-stripped token edit-distance calculation produced 40 edits / 181 reference words = 22.1% WER. The exact figure may vary with number normalization, but the missing sentences and repeated utterance are undeniable.

The 0.4.2 report had a prior 1.12% result on a similar meeting benchmark. Determine whether this is a model regression, audio/loopback capture difference, live/final overlap, chunk-offset defect, self-test harness problem, or segment aggregation bug. Do not guess.

## Investigate these paths

- `crates/asr/src/guard.rs` (`collapse_repeats`, `kept_segments`)
- `apps/desktop/src-tauri/src/controller.rs` (`transcribe_meeting`, chunk offsets, timestamps)
- `apps/desktop/src-tauri/src/meeting.rs` (`pieces_to_take` and chunk splitting)
- `apps/desktop/src-tauri/src/library.rs` (`live_loop`, `stop_meeting`, merge live and final segments)
- `crates/core/src/summary.rs` (`drop_echo`)
- `scripts/make-meeting.ps1`
- the exact `meet-en-system.wav` and any mic track supplied in the fixture

## Required investigation

1. Verify the exact WAV and duration used in the failing 0.4.3 run.
2. Run the same source WAV through a standalone recognizer path that bypasses meeting chunking.
3. Run the same WAV through the 0.4.3 meeting chunk/live/final pipeline.
4. Log for each ASR piece: track, input sample start/end, piece offset, recognizer segment start/end, transcript text, pass type (live/final), model/language, no-speech score, and whether segment comes from a retried or reprocessed piece.
5. Prove every source sample range is covered exactly once or intentionally overlapped with deduplication.
6. Check that live and final passes do not reprocess the same source range.
7. Compare adjacent same-track segments for exact/near duplicates and overlapping timestamps.
8. Confirm whether the repeated “Pria...” is present in the source audio or introduced by the recognizer/chunk merge.
9. If the source audio contains one utterance, the stored transcript must not contain five copies.
10. If a recognizer result is suspicious, preserve the raw output in the diagnostic artifact and mark it uncertain rather than silently presenting it as fact.

## Fix expectations

- `collapse_repeats` currently operates inside one string. `kept_segments` calls it separately for each segment. Add a timestamp-aware cross-segment guard after aggregation, not only a text-level function per segment.
- Do not blindly remove natural repeated speech. Use source track, timestamps, exact/near text and chunk provenance. If there is uncertainty, surface a review flag.
- Ensure `live.segments` and `final.segments` cannot duplicate any source sample range or same transcript segment.
- Do not discard complete speech sections to solve repetition.
- Add the failing meeting as a regression fixture (audio kept in test data only if distribution rights permit; otherwise use a fixture of expected segmented outputs plus a private local audio test).
- Include the 13 reference utterances in exact order.

Acceptance: the release build must recover the hiring discussion, Priya deadline, GST risk/deadline and Rahul action. It must not duplicate any utterance. Report WER with the normalizer documented and compare the same exact WAV with 0.4.2 if available.

---

# 3. P0 MEETING SUMMARY CONSISTENCY — NO “CONFIRMED + CHECK THIS”

Inspect `crates/core/src/summary.rs` around `validate()` lines 1016–1060 and the Markdown renderer.

The v0.4.3 output has a decision:

`Approve an additional Rs 50,000 ... provided ROI stays above 3x.`

Evidence points only to the segment “Agreed. Let us approve the 50,000 rupees with that condition.” The `3x` condition exists in the prior speaker turn. The validator adds a flag that number 3 is missing, but leaves `status = confirmed`; Markdown outputs “Confirmed” and “check this” at once.

## Required fix

- Validate all material subclaims in a decision separately: action, amount, condition, ratio, owner, deadline and status.
- Attach the evidence for the agreement segment and the condition segment (or reject the condition as unsupported).
- Any unresolved material mismatch forces status to `proposed` or `needs_review`, never `confirmed`.
- Markdown, JSON and UI must use the same canonical status; no renderer may re-label a warning item as confirmed.
- A status transition must be covered by tests for number missing, wrong number, wrong amount, condition omitted, disagreement, tentative language and missing agreement.
- If a decision is “confirmed with condition”, render the condition adjacent to the confirmed decision and cite both segments.

Add regression test using the actual 50,000 / ROI >3x case from `meet-en.json`.

---

# 4. CORRECTION LEARNING — SCOPE IT TO THE ORIGINAL ELEMENT AND BALANCE COM

Inspect:
- `apps/desktop/src-tauri/src/learn_watch.rs`
- `apps/desktop/src-tauri/src/controller.rs` around correction-watch creation/polling
- `crates/core/src/learn.rs`
- `apps/desktop/src/SettingsPage.tsx` learning toggle and copy

## Current problems

`Watch` stores only the PID, inserted text and timer. `focused_text(pid)` calls `GetFocusedElement()` and verifies only that the new focused element belongs to the same process and is not a password box. It does not verify that this is the exact textbox where the dictation was inserted. It can call `DocumentRange().GetText(20_000)`, reading a large amount of the currently focused document/editor. The feature's claim that it reads “the same text box” is not enforced by the implementation.

Also, `CoInitializeEx` is called once every polling cycle (about every four seconds for up to 90 seconds) and is never balanced by `CoUninitialize`. The return code is ignored.

## Required fix

1. Capture original target identity at insertion: PID, HWND, UIA AutomationId/RuntimeId or another stable element identity, control type and process/window identity.
2. On every poll, verify the same window and same UIA element. Stop immediately when focus or element identity changes.
3. Read only the smallest text range containing the inserted dictation and subsequent correction. Do not pull the entire 20,000-character document if avoidable.
4. If a control cannot provide a stable identity or text range, disable correction learning for that control and explain why.
5. Never inspect password fields or secure controls; test standard and custom secure controls.
6. Initialize COM once on the correct thread with an RAII lifetime, or correctly balance each successful initialization. Handle `RPC_E_CHANGED_MODE`.
7. The user's text must never be persisted, logged or uploaded. Store only the specific approved word-pair correction.
8. Add UI controls to pause/disable learning and clear learned pairs. Consider making the feature opt-in until the element-scoping guarantees are tested.
9. Test a sequence: type dictation in Field A, correct one word; change focus to Field B in the same app; type unrelated sensitive text; verify Field B is never read or learned. Also test editor documents with >20k characters.
10. Add manual validation evidence using Notepad and Word. Do not claim it works only because `learn::find_edited` unit tests pass.

---

# 5. MICROPHONE START / FIRST-WORD RELIABILITY AND PRIVACY COPY

Inspect `apps/desktop/src-tauri/src/audio.rs` and `apps/desktop/src/SettingsPage.tsx`.

Current implementation:
- `PRE_ROLL_MS = 1000` (one second), `audio.rs:28–30`.
- Settings copy says only the last `0.3 s` is kept in memory, `SettingsPage.tsx:187–188`.
- The stream watchdog handles no data after 2 seconds and exact zeros after 4 seconds, with backoff.
- `last_sound` is updated when any sample is not exactly zero, `audio.rs:144–150`.
- Leading exact zero samples are removed, `audio.rs:354–363`.
- The original issue affected 10 of 12 personal recordings with 2.2–4.0 seconds of exact zeros. The root cause is still unproven.

## Required fix and tests

1. Fix the 0.3s vs 1.0s settings text, or intentionally change the buffer and demonstrate first-word capture remains reliable.
2. Replace “any nonzero sample means sound” with a tested adaptive energy/noise-floor signal check (do not make quiet speech disappear).
3. Show “Mic starting — wait for ready sound” while the stream is not ready. Do not play the ready sound until the recorder can capture voice at a stable level.
4. Keep pre-roll memory bounded and clear it after the session. Ensure the app does not save audio while idle.
5. Test at least 100 cold starts and 100 warm starts on the owner's actual laptop. Include default mic, external USB mic, headset/Bluetooth if available, muted mic, no-input device, and microphone contention with other dictation apps.
6. Log timing markers only (no audio/text): hotkey, stream open, first callback, stable signal, first VAD-positive frame, recording start, stop.
7. Run the currently ignored mic-start test manually and include results in the report.
8. Do not claim “first word never cut off” until the test passes. If the user speaks before ready, explain whether the audio is buffered or discarded.

---

# 6. HINGLISH ACCURACY AND SAFE CLEANUP

The product supports English and Roman-script Hinglish. Preserve this exact product scope.

Available reference benchmarks from previous runs:
- English synthetic set: 30 clips, reported 0.62% WER.
- Hinglish synthetic set A: 60 clips, roughly 20.03% final WER (tolerant spelling score also reported).
- Hinglish synthetic set B: 12 clips, 24.24% WER.
- Owner's 53 personal dictations/voice notes have no corrected transcripts, so they are not scoreable yet.

The 0.4.3 status does not show a fresh 0.4.3 end-to-end score for every supplied audio clip. Do not claim that the new release has improved WER unless you run it and publish results.

## Required benchmark

Run the exact release build of the new branch on all 30 English + 72 Hinglish clips. For every clip save:
- exact reference transcript,
- raw ASR result,
- final result,
- selected route/model/language,
- language detection decision,
- all AI transformation stages and whether each was accepted/rejected,
- request count and upload bytes,
- total latency and each-stage latency.

Score and publish separately:
- English WER and CER.
- Hinglish strict Roman WER and CER.
- Hinglish tolerant spelling WER using a versioned list of approved spelling variants.
- Error breakdown by speaker, sentence, speech speed, language mix and quiet/noisy conditions.

Do not combine English and Hinglish into one number. Do not compare synthetic speech numbers with real voice as if they were the same distribution.

## Real speech

Ask the owner to prepare corrected references for at least 10–20 short, consented English and Hinglish voice clips. Keep these separate from synthetic benchmarks. Do not silently use a guessed transcript as ground truth.

## Hinglish safety

- Keep Hindi words in Roman letters when Roman Hinglish is the selected output mode.
- Do not translate the Hindi portion into English.
- Do not convert it into Devanagari unless user explicitly selects that output mode (if such a mode exists).
- If an unclear word cannot be recovered, preserve the raw phrase or mark it for review instead of inventing a plausible word.
- Preserve negation, amounts, dates, time, names, conditionals, subjects, ownership and tense.
- The AI spelling polish must not “repair” a garbled phrase into a different phrase with changed meaning.
- Fix the `hamne` rejection only if a safe Roman-Hinglish equivalence rule can prove the word was not removed. Do not loosen preservation checks globally.

---

# 7. SMART LAYOUT — PRESERVE NATURAL PARAGRAPHS AND USE LISTS ONLY WHEN WARRANTED

The owner's formatting reference image defines four patterns:

1. Paragraph + points: explanatory paragraph, then genuine key decisions/action items.
2. Points only: when the speaker actually dictates a list.
3. Paragraph only: story, explanation or continuous thought.
4. Complex structure: topic heading, paragraph, subheadings, bullets, numbered actions.

Principle: **Organize information, not every sentence.**

Existing relevant code:
- `crates/core/src/pipeline/structure.rs`
- `crates/core/src/layout.rs`
- `spec/prompts/layout.md`
- `apps/desktop/src-tauri/src/controller.rs` around layout and list formatting
- `SettingsPage.tsx` Writing > Layout

## Required fixes

1. Use a typed internal block representation: heading, paragraph, bullet list, numbered list, decision block, action block, callout/uncertain claim. Avoid multiple string-only formatting code paths.
2. Render all bullet items with one marker style. Rule-based spoken list commands and AI smart layout must use the same block type and renderer.
3. Do not split paragraphs simply at full stops or short pauses.
4. Do not make a bullet list simply because there are two or more sentences.
5. Use paragraph + list together when the speaker first explains a topic and then states distinct items.
6. Only create a heading when the speaker explicitly says the label or when a meeting-summary schema intentionally supplies a section heading. Do not invent section facts.
7. A sequence such as “first…, second…, third…” can become a numbered list.
8. Spoken “bullet point…” commands can become bullets.
9. Natural narratives and explanations must remain paragraphs.
10. A sentence with “however”, “because”, “although”, “which means”, “lekin”, “isliye”, etc. often continues a thought. Do not assume it is an independent bullet.
11. In English + Hinglish, use language-aware sentence and connector heuristics. Avoid Latin word-boundary errors on Roman Hinglish.
12. Keep raw transcript separately from formatted output.
13. The layout acceptance validator must preserve all important words in order. In particular, remove meaning-bearing tokens such as `we`, `our`, `my`, `will`, `is`, `was`, `have`, `has`, `that`, `which`, `hai`, `hain`, `mein` and `toh` from the droppable connector allow-list. No threshold may permit even one protected-token deletion.
14. When layout output fails validation, preserve the full original text and explain that layout was skipped; do not silently trim it.

## Critical source-level flaw to fix: connector allow-list can delete meaning-bearing words

Inspect `crates/core/src/layout.rs:21–83` and `:142–190`. The current `CONNECTORS` list includes `we`, `our`, `my`, `will`, `is`, `are`, `was`, `were`, `have`, `has`, `that`, `which`, and Roman-Hinglish tokens such as `hai`, `hain`, `mein`, `toh`. The validator allows up to three of these tokens to be dropped without rejection. This can alter subject/ownership, tense, modality, condition or Hinglish grammar. The fact that a token is a grammatical connector does not make it semantically disposable.

Required changes:
1. Remove subjects, pronouns, possessives, auxiliaries, tense markers, modal verbs, negation, conditions, numbers, dates, amounts, names and meaning-bearing Hinglish tokens from any droppable list.
2. Keep any allowed omissions exceptionally narrow and documented. Only permit a spoken list-introducer/ordinal when the output clearly replaces it with the corresponding numbering/list marker and the content item remains unchanged.
3. Prefer preserving the original utterance text verbatim inside typed paragraph/list blocks. Formatting should change block boundaries and markers, not silently rewrite or delete semantic words.
4. If a proposed layout loses a protected word, reject it and fall back to the original text with a diagnostic reason.
5. Do not merely raise/lower the global percentage threshold. The protected-token check must be unconditional, even if only one critical token is removed.

Add adversarial tests proving that layouts removing any of these are rejected: `we`, `our`, `my`, `will`, `is`, `was`, `have`, `has`, `not`, `only`, `if`, `unless`, `that`, `which`, `hai`, `hain`, `mein`, `toh`, dates, names and amounts. Also test safe conversion of `first/second/third` into numbered list markers without dropping the actual item text.

## Required golden tests

Build exact, deterministic cases for:
- single paragraph with many sentences;
- two paragraphs in one explanation;
- paragraph + key decisions bullets;
- paragraph + numbered next steps;
- bullets only;
- numbered process only;
- narrative/story only;
- English financial explanation with numbers and caveats;
- Roman Hinglish explanation with `lekin`, `kyunki`, `isliye`, `toh`, `mein`, `humne`;
- spoken command “bullet point…”;
- spoken enumeration “first, second, third”;
- negative/conditional statements (“we will not approve unless…”).

The release output for a test meeting must look like:

**Financial Performance**

Revenue increased by 12% compared with the previous quarter, primarily from the western region. However, gross margin fell from 41% to 38% because freight costs increased.

**Key Decisions**
- Approve the campaign budget only if ROI remains above 3x.
- Review the dashboard readiness next week.

**Next Steps**
1. Priya completes the interviews by next Friday.
2. Rahul prepares the GST return by the 18th.
3. Review the discussion again next Monday.

This is an illustrative layout fixture. The words/decisions/tasks must be generated only when they are actually supported by the source transcript. Do not copy this sample into arbitrary user dictations.

---

# 8. MEETING SUMMARY EVIDENCE AND CONTENT SAFETY

Relevant sources:
- `crates/core/src/summary.rs`
- `spec/schemas/meeting_summary.schema.json`
- `spec/prompts/summary.md`
- `apps/desktop/src-tauri/src/library.rs`
- `apps/desktop/src/Meetings.tsx`

The current evidence validator checks that cited segment IDs exist and attempts to verify quotes, amounts, names, owners, dates and uncertainty. Keep this. Improve it; do not remove it to make summaries look cleaner.

## Required behaviour

- A confirmed decision needs transcript evidence of both the proposal and agreement, plus evidence for all material amounts/conditions.
- A proposed item must not become confirmed merely because someone said “okay” if the scope/condition is not clear.
- If a number, owner, amount, date or condition is absent from the cited text, mark the item “Needs review” or drop that specific unsupported detail.
- Never assign action owners by inference.
- Never invent deadlines.
- Distinguish “discussed”, “proposed”, “agreed”, “approved”, “rejected”, “deferred” and “open question”.
- Do not treat an open question (“I am not sure yet”) as a decision.
- Do not call an item a risk merely because the LLM thinks it sounds risky; require transcript support.
- Keep an evidence link/timestamp for each key item.
- Show unclear numbers/names as uncertain and offer replay/edit.
- Preserve original transcript and all user edits. Regenerating notes must never overwrite manually edited text without confirmation.

## Participants vs audio source

The current output says `Participants: System audio`. This is wrong. System audio and microphone are capture sources. Until diarization exists, show:
- Audio sources: System audio, Microphone.
- Participants: Not identified (unless user supplied attendee names).
Do not pretend the app has identified people based on audio source labels.

## Meeting workflow tests

1. Re-run the 87-second English finance meeting and verify all 13 reference turns.
2. Add an exact reference transcript for the mixed English/Hinglish meeting fixture.
3. Test no-audio, one-track, two-track and echo conditions.
4. Test 10-second, 1-minute, 10-minute and 60-minute meetings.
5. Test pause/resume, device unplug/reconnect and app interruption.
6. Confirm interrupted recordings can be recovered and re-transcribed.
7. Confirm all history/delete/retention paths work on actual files, not just database metadata.
8. Test export Markdown/TXT/DOCX/PDF if available.

---

# 9. LIVE TRANSCRIPT LATENCY AND CHUNK MANAGEMENT

Current behaviour waits for at least 25 seconds of new audio (`meeting.rs` around `pieces_to_take`) and polls around every 10 seconds (`library.rs` live loop). This is near-live, not instant.

Choose one:

- Improve incremental transcription to provide first feedback sooner with overlap-aware chunks; or
- Explicitly call it near-live and show the expected chunk delay.

Do not remove chunk overlap/context without benchmarking. Ensure no speech disappears at chunk boundaries and no phrase is double-counted. Instrument each range with start/end offsets and stable source IDs.

Target measurable latency: first transcript update under 15 seconds on the owner's GPU where practical, but accuracy and no-loss take priority. If not achieved, report the actual observed latency and label the feature honestly.

---

# 10. AUDIO SOURCE / MIC DEVICE / FIRST-WORD TESTING

- Add an explicit microphone selector for Instant Dictation, not only a default-device assumption, if feasible within the current audio design.
- Persist device identity safely and handle device IDs changing after reconnect.
- Keep separate system audio and microphone tracks for meetings.
- A source label is not a speaker name.
- Test default device, USB mic, headset, Bluetooth, device switch mid-recording, mute/unmute, permission denial and no device.
- Never silently continue with an unavailable source.
- Show exactly which source(s) are recording.
- Make recording permission/participant consent visible.
- Do not auto-record meetings.

---

# 11. DIAGNOSTICS / CLOUD COST AND PRIVACY TRANSPARENCY

## Diagnostics

- Add clear per-stage timing: microphone startup, recorded audio duration, ASR route, chunk processing, cleanup, layout, insertion, meeting notes generation.
- Log model and provider names but never API keys, audio or transcript text.
- Log number of audio upload requests and text upload requests per dictation, total bytes, and fallback reason.
- Redact secrets from endpoint URLs and error bodies.
- Include a “Copy diagnostics” action that does not include user content.

## Cloud request fan-out

In English + Hinglish mode the current design can send recognition audio twice in parallel, sometimes three times for English rescue. Subsequent spelling, cleanup and smart-layout steps add text calls.

- Display this plainly in settings/consent.
- Explain that cloud ASR sends the audio to the user's chosen provider and may consume multiple requests.
- Provide a local-only mode that disables all outbound processing calls.
- When rate-limited, preserve the speech and show which stages fell back to local rules. Do not claim AI cleanup/layout ran when it did not.
- Add a configurable per-session request/cost budget and a warning before heavy processing.

---

# 12. ANDROID AND PWA HARDENING

## Android recorder

Inspect `apps/android/app/src/main/java/app/voice/Recorder.kt` around `stop()`.

The code must not release the `AudioRecord` or return a PCM buffer while the reader thread is still alive. Use a shutdown signal that unblocks `read`, join and verify termination, and return a visible error if shutdown cannot be completed safely. Test repeated start/stop cycles and device interruptions on a physical Android phone.

## PWA key storage

Keep the present AES-GCM/IndexedDB design and the clear threat-model explanation. The key deletion currently removes both `groq` and `devicekey` records; add an automated browser test to prove this. Maintain a strict CSP and no third-party scripts. Treat same-origin XSS as a credential risk; do not claim non-extractable keys defend against malicious JavaScript running in the app origin.

---

# 13. HOTKEY / CLIPBOARD / COMMAND MODE

- Keep keyboard hook callbacks short and non-blocking.
- Avoid blocking mutex acquisition in a low-level Windows hook callback; use atomic state or safe try-lock fallback.
- Test Ctrl+Win, Ctrl+Win+H, double tap, reverse modifier order, Escape cancel, other Ctrl+Win OS shortcuts, injected keystrokes, self-injected paste and hotkey conflicts with Wispr Flow/Typeless/Genspark.
- Test target-window changes between start and paste.
- Test Notepad, Word, Gmail/Chrome, Google Docs, VS Code, Windows Terminal and elevated apps.
- Verify clipboard preservation for text, HTML, image and unsupported formats. Do not claim universal clipboard restoration if only selected formats are preserved.
- Check actual Windows Win+V clipboard history, not only the exclusion marker.
- Command Mode must never paste into a different window after target focus changes.

---

# 14. RELEASE VALIDATION ON THE OWNER'S MACHINE

Before release, install the v0.4.4 candidate on the owner's real Windows user profile using the actual production installer (not a Claude/VS Code sandbox AppData location).

Record evidence for:

- Global shortcuts in real keyboard use.
- First word captured after both cold and warm mic starts.
- Dictation into Notepad, Word, browser text field, VS Code and Windows Terminal.
- Command Mode and selection replacement.
- Correction learning in at least two apps, including focus change to another field.
- A real English meeting and mixed Hinglish meeting.
- Pause/resume and device change.
- Reopen, edit, re-transcribe, export and delete meeting.
- Retention deleting actual audio files after the configured expiry.
- Offline/local-only mode with network disabled.
- Installation, upgrade from 0.4.3, and uninstall preserving user data as designed.

Do not call sample-data UI previews live acceptance tests.

---

# 15. REQUIRED TEST REPORT

Produce:
- `AUDIT_REMEDIATION_PLAN.md`
- `V0.4.4_TEST_REPORT.md`
- `ACCURACY_ENGLISH.csv`
- `ACCURACY_HINGLISH.csv`
- `MEETING_REGRESSION_REPORT.md`
- `SECURITY_REGRESSION_REPORT.md`
- `RELEASE_CHECKLIST.md`

Run and report:
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `pnpm exec tsc --noEmit` for desktop and web
- relevant front-end unit tests/build
- `pnpm tauri build`
- exact release-pipeline self-tests
- manual Windows tests that require the owner

List passed, failed, skipped and not-run tests separately. Never silently exclude a failure. Explain every ignored test.

---

# 16. RELEASE / VERSIONING

- Keep all work on the remediation branch until acceptance criteria pass.
- Create a database migration only if schema changes require it. Migrations must be idempotent, backward-compatible, and tested on a copy of a v0.4.3 database.
- Never delete recordings, history, learned words, dictionary entries, snippets, API keys or models during upgrade.
- Preserve `app.bolkar.voice` storage/key namespace for compatibility.
- Build v0.4.4 installer and portable build only after tests.
- Publish SHA-256 for each installer.
- The installer remains unsigned unless the owner provides a signing certificate. Do not pretend it is signed.
- Provide rollback instructions to return to 0.4.3 while preserving user data.

---

# FINAL INSTRUCTION

Make the existing Speak Flow application substantially more reliable. Do not start over. Do not add shiny features while P0 defects remain. Fix the URL credential-leak path, the corrupted meeting transcript, and contradictory confirmed decisions first. Then fix correction-learning scope, first-word proof, Hinglish accuracy, the smart-layout connector deletion flaw, layout consistency and the remaining reliability issues.

A 10/10 rating is not something to write into the README. It has to be earned through reproducible tests on actual speech and actual Windows usage. Report the honest result, even if it is not yet 10/10.

# Remediation plan 0.4.2

Source: the independent audit of 0.4.1 (`Speak_Flow_0.4.1_Audit_Report.md`, 17 Nov 2026) and its
improvement prompt. Every finding was first checked against the code; all were confirmed. Status
and evidence are as of 17 Nov 2026. "Unit" = automated test; "App" = quiet self-test of the
release build (no window, nothing typed, copy of the owner's settings); "Not run" = not tested.

| # | Finding (severity) | Source | Fix | Test | Status |
|---|---|---|---|---|---|
| 1 | Meeting passed `en-hinglish` / `hinglish` to Whisper as a language code (P1) | `controller.rs::transcribe_meeting`, `crates/asr` | Typed `LanguageMode` -> `RecognitionPlan` -> `RecogniserLanguage` (`crates/core/src/language.rs`); per ~30 s piece "en" or "hi" (+ Roman example), never the setting string. Dictation also reads the setting through the typed mode | Unit (3); App: English, Hinglish and mixed meetings | Fixed |
| 2 | Hands-free made every sentence a bullet; default on (P1) | `controller.rs`, `lib.rs` settings | New setting `list_format`: Natural (default) / Auto-detect / Always. Natural = lists only from spoken enumerations or "bullet point". Auto uses a conservative independent-items test. Old `handsfree_points` is ignored | Unit: audit cases A-D + classifier tests; App: A-D dictated through the release build | Fixed |
| 3 | Meeting topics forced into bullets (P1) | schema, `summary.rs`, prompt | Topic = `narrative` paragraph + optional `key_points`; renderer writes a paragraph; prompt v2 forbids one bullet per sentence; old summaries still load | Unit (render test); App: English meeting notes had 0 bullets per topic | Fixed |
| 4 | Meetings saved but not reachable; no delete (P1) | `db.rs`, `api.ts`, UI | Meeting library: list + search (title, transcript, notes), open, rename, correct a line (original kept), write notes again (new version, old kept), transcribe again, export Markdown/text, delete one, delete all; interrupted recordings recovered at start-up | Unit (DB lifecycle, export names); App: saved meetings appear in the library; UI checked in browser preview | Fixed (DOCX/PDF export and editing the AI notes themselves: not built) |
| 5 | Retention did not delete meeting audio (P1) | `db::expire_audio` | Separate settings: dictation audio, meeting audio, dictation history, meetings. Files deleted first; DB cleared only when gone; failures logged; only files inside the data folder; active recording untouched | Unit (DB queries, deletion guard); real ageing on disk: Not run | Fixed |
| 6 | Evidence check claimed more than it proves (P1) | `summary::validate`, UI wording | Quotes checked word for word (pieces joined by "..." each checked); every number in an item checked; shared-word check; hedge kept ("maybe" -> tentative deadline; certain claim from hedged line -> "check this"); "confirmed" needs agreement words, else "proposed"; UI says "Quote found" / "Check this", never "verified" | Unit (18 incl. 9 adversarial cases from the prompt); App: English meeting | Fixed (meaning is still not proven; a second AI verifier was not added) |
| 7 | Long meetings: one summary call (P1) | `stop_meeting` | Windows of ~12,000 characters, one call each, deterministic merge (topics joined, duplicates kept once), then one validation; partial failure keeps the rest and says which part is missing; transcript always saved; "Write notes again" retries | Unit (windows + merge); App: 64-87 s meetings (one window); 10/30/60 min: Not run | Fixed (per-window progress is not saved across a crash during note writing) |
| 8 | No live transcript (P1) | `Meetings.tsx`, `lib.rs` | Near-live transcript while recording on the graphics card (new lines about every 30 s, marked provisional); without GPU it says the transcript is made after the meeting. The final pass only does what is left | Unit (piece selection); App: live pass + final pass on 3 meetings | Fixed (near-live, not word-by-word) |
| 9 | "Me"/"Others" labels imply people (P1/P2) | `lib.rs`, UI | Labels "Microphone" / "System audio" everywhere; prompt says they are sources | UI checked | Fixed |
| 10 | Timestamps per 30 s chunk (P2) | `transcribe_meeting` | Whisper's own segment times + piece offset (`guard::kept_segments`) | App: English meeting lines matched the 13 spoken sentences | Fixed |
| 11 | Recorder ignored write/flush errors (P2) | `meeting::drain` | Errors stop that track, keep the saved part, and show in the app and the log | Code; disk-full test: Not run | Fixed |
| 12 | Startup writer error shown as time-out (P2) | `meeting::Recording::start` | Every start-up error returned through the ready channel | Code | Fixed |
| 13 | Default devices only; F32/I16 only (P2) | `meeting.rs` | Device lists and choice (remembered); all CPAL sample formats via one generic converter | UI in preview; real devices: Not run | Fixed (no automatic switch when a device is unplugged; pause/resume not built) |
| 14 | History save errors swallowed (P2) | `controller.rs` | Pill says "Text inserted, but it could not be saved in History"; logged | Code | Fixed |
| 15 | No diagnostics log (P2) | - | `diag.rs`: content-free log (1 MB, redacts keys), open/export/delete in Settings | Unit (redaction) | Fixed (on by default, see deviations) |
| 16 | Clipboard marker errors ignored; partial write (P2) | `insert/clipboard.rs`, `insert/mod.rs` | Marker failures logged; previous clipboard restored if writing the dictation fails | Existing clipboard tests | Fixed |
| 17 | PWA `forgetKey` wrote `undefined` (P2) | `apps/web/src/keystore.ts` | Real IndexedDB `delete` of both records | Browser test (save, unlock, wrong passphrase, forget -> no records) | Fixed |
| 18 | PWA threat model / CSP (P2) | `keystore.ts`, web build | Threat model documented; CSP on the built page (inline theme script moved to a file); malformed Groq answers handled; model choice remembered per key | Built page loads under CSP without errors | Fixed |
| 19 | Android recorder stop race (P2) | `Recorder.kt` | Stop unblocks read, joins the thread, handles negative read codes, catches `startRecording` | CI compile only; phone: Not run | Fixed in code |
| 20 | Context awareness is coarse (P2 feature gap) | - | Not built (large, privacy-sensitive) | - | Not done |
| 21 | Installer unsigned (P2) | - | Needs a paid certificate: owner's decision. SHA-256 published | - | Not done |
| 22 | Old app id `app.bolkar.voice` (P3) | - | Kept on purpose (audit agrees) | - | No change |
| 23 | Privacy docs contradictory | `docs/*` | PRIVACY, SECURITY_CONTROLS, HOW_TO_USE rewritten to match the code | Review | Fixed |
| 24 | Other dictation apps may grab the keys | - | Home warns when Wispr Flow, Typeless, Genspark Speakly, Superwhisper etc. are running | UI in preview | Fixed (custom shortcut UI not built) |
| 25 | Hinglish answers in capitals (found in this round's app test) | Groq output | `pipeline::fix_all_caps` | Unit; App re-test | Fixed |

## Deviations from the improvement prompt, and why

- **Consent:** the prompt prefers separate switches for audio upload and text clean-up. They
  are already separate in Settings > Groq & AI. Connecting a Groq key still turns both on in one
  step, because the owner asked for exactly that in v0.2.0; the screen says plainly what each
  sends, and either can be switched off.
- **Diagnostics log on by default** (the prompt says opt-in): it contains no speech content,
  keys or titles, and a log that is off cannot explain the first failure. It can be switched off
  and deleted in Settings.
- **Hindi number words** ("das" = 10) were not added to the number checks: "do" (two) is also a
  common English word and would weaken the guard against invented numbers.

## Not done (needs the owner, hardware, money or a larger project)

Physical Ctrl + Win tests and the 50-dictation stress test; a real Zoom/Meet/Teams meeting;
10/30/60-minute recordings; unplug/disk-full tests; human-voice benchmark with corrected
references; Android on real phones; code signing; custom shortcut UI; deeper context awareness;
editing the AI notes themselves; DOCX/PDF export; pause/resume.

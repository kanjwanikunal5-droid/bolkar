# Security controls

**This project holds NO security or compliance certifications.** It is not SOC 2, ISO 27001 or
HIPAA certified, and has no agreements with any provider about data retention. Those are audits
and contracts of organisations, not features of code. What follows are plain technical controls,
with how each was checked. Updated for 0.4.2 (17 Nov 2026).

## In place

| Control | How | Tested |
|---|---|---|
| Audio stays local unless Groq speech is on | whisper.cpp on this computer; audio is uploaded only with the separate "Speech recognition by Groq" consent; meeting audio is never uploaded | Code review |
| Text leaves only after consent | AI clean-up (dictation text, meeting transcript for notes) runs only when a provider is chosen, a consent row is recorded and (where needed) a key exists | Consent storage tested; `ai_config` itself: code review only |
| Keys only in Windows Credential Manager | `credentials.rs`; never in the database, files or logs | Round-trip test against the real Credential Manager |
| Every upload logged locally | `events` table, shown under "Data that left this computer" | Database test |
| AI cannot add facts to dictations | `llm::accept` rejects new numbers, names, e-mails, links, rewrites, over-long answers, damaged snippets | Unit tests |
| Meeting notes checked against the transcript | `summary::validate`: cited lines must exist; owners, deadlines, numbers and quotes must occur in them; hedged deadlines marked tentative; unsupported "confirmed" decisions shown as proposed; mismatches marked "Check this". Meaning is not proven and the app says so | 17 unit tests incl. adversarial cases |
| AI time limit | 2.5 s for dictation clean-up, then the built-in rules; 90 s per meeting-notes window | Fake-server test (dictation) |
| Provider addresses | Must be `https://`, or `localhost` for a model on this computer | Code |
| Dictations out of clipboard history | `ExcludeClipboardContentFromMonitorProcessing` + `CanIncludeInClipboardHistory=0` + `CanUploadToCloudClipboard=0`; a marker that cannot be set is logged | Clipboard test (marker present); Win+V checked by eye: not run |
| Clipboard restored only if untouched | Sequence-number check; previous clipboard put back if writing the dictation fails | Clipboard test |
| No paste into the wrong window | Foreground checked at key-down, before and right before the paste | Self-test |
| No typing into admin apps | Elevation check, then copy-only fallback | Code; manual test not run |
| Model files verified | SHA-256 against fixed values; damaged downloads deleted | Unit test |
| Retention deletes only app files | `remove_app_file` refuses paths outside the data folder; DB entries cleared only after the file is gone | Database tests; file deletion: code review |
| Diagnostics without content | Log lines carry categories, timings and error texts only; key-shaped strings are removed (`diag::redact`) | Unit test |
| Web app script policy | Content-Security-Policy on the built page: own scripts + WebAssembly only, network only to itself and api.groq.com, no plugins, no `<base>` | Build output checked |
| Web app key removal | `forgetKey` deletes the IndexedDB records (not just their values) | Code review; browser test not run |
| No secrets in the repository | `.gitignore`; gitleaks scans the full history in CI | CI |
| Web views have no file, shell or network access | Tauri capability grants only the app's own commands | Config |

## Not in place

| Control | Note |
|---|---|
| Code signing | Needs a paid certificate (owner's decision). Until then Windows SmartScreen warns; SHA-256 of each installer is published with the release. See `docs/CODE_SIGNING.md`. |
| Automatic updates | Not built. The app only checks GitHub and opens the release page; nothing is downloaded or installed by itself. |
| Local-only switch blocking all network use | Not built. |
| Penetration test | Not done. |

## Without code signing

Windows SmartScreen will warn on the installer ("More info -> Run anyway"). The release page
lists the installer's SHA-256 so the download can be checked.

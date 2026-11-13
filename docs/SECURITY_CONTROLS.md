# Security controls

**This project holds NO security or compliance certifications.** It is not SOC 2, ISO 27001 or
HIPAA certified, and has no agreements with any provider about data retention. Those are audits
and contracts of organisations, not features of code. What follows are plain technical controls.

## In place

| Control | How | Tested |
|---|---|---|
| Audio never leaves the device | Speech recognition runs locally (whisper.cpp); no code uploads audio | Code review |
| Text leaves only after consent | AI clean-up runs only when a provider is chosen, a consent row is recorded and (where needed) a key exists | Consent storage tested; `ai_config` itself: code review only |
| Keys only in Windows Credential Manager | `credentials.rs`; never in the database, files or logs | Round-trip test |
| Every upload logged locally | `events` table, shown under "Data that left this computer" | Database test |
| AI cannot add facts | `llm::accept` rejects new numbers, names, e-mails, links, rewrites, over-long answers, damaged snippets | 8 unit tests |
| AI time limit | 2.5 s, then the built-in rules | Fake-server test |
| Provider addresses | Must be `https://`, or `localhost` for a model on this computer | Code |
| Dictations out of clipboard history | `ExcludeClipboardContentFromMonitorProcessing` + `CanIncludeInClipboardHistory=0` + `CanUploadToCloudClipboard=0` | Clipboard test |
| Clipboard restored only if untouched | Sequence-number check | Clipboard test |
| No paste into the wrong window | Foreground checked at key-down, before and right before the paste | Self-test |
| No typing into admin apps | Elevation check, then copy-only fallback | Code; manual test not run |
| Model files verified | SHA-256 against fixed values; damaged downloads deleted | Unit test |
| No secrets in the repository | `.gitignore`; gitleaks scans the full history in CI | CI |
| Web views have no file, shell or network access | Tauri capability grants only the app's own commands | Config |

## Planned

| Control | Stage |
|---|---|
| Local-only switch blocking all network use | 9 |
| Text retention timer (90 days) and "Delete all data" | 3 |
| Signed update manifests | 8 |
| Log export with secrets removed | 9 |

## Without code signing

Until a signing certificate is approved, Windows SmartScreen will warn on the installer. The
README will explain the "More info -> Run anyway" step honestly rather than hide it.

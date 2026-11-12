# Security controls

**This project holds NO security or compliance certifications.** It is not SOC 2, ISO 27001 or
HIPAA certified, and has no agreements with any provider about data retention. Those are audits
and contracts of organisations, not features of code. What follows are plain technical controls.

## In place today

| Control | How |
|---|---|
| No secrets in the repository | `.gitignore` excludes key and keystore files; CI runs a gitleaks secret scan on every push |
| No network use in the core | `crates/core` has no HTTP dependency; it compiles for wasm32 with no I/O |
| Reproducible dependency set | `Cargo.lock` is committed |
| CI has read-only repository permission | `permissions: contents: read` in the workflow |
| Cleanup cannot invent content | Deterministic rules only remove or reformat; golden fixtures enforce it |

## Planned (not built yet)

| Control | Stage |
|---|---|
| API keys in Windows Credential Manager / Android Keystore | 2, 5 |
| Per-provider consent screen with data-policy note | 2 |
| Local-only mode | 2 |
| Local upload log ("Data that left this device") | 2 |
| Dictations excluded from Windows clipboard history and cloud clipboard | 1 |
| Retention timers and "Delete all data" | 2, 3 |
| Model downloads from fixed addresses with SHA-256 checks | 1 |
| Signed update manifests | 8 |
| Logs exportable with secrets removed | 9 |

## Without code signing

Until a signing certificate is approved, Windows SmartScreen will warn on the installer. The
README will explain the "More info -> Run anyway" step honestly rather than hide it.

# Code signing (Stage 8)

**Status: not signed. Nothing has been applied for yet: the application must come from the
owner.** Until the installer is signed, Windows SmartScreen shows "Windows protected your PC"
on download; users choose "More info" -> "Run anyway". The landing page says so.

## Why it matters

An unsigned installer from a new publisher triggers SmartScreen and some antivirus programs
(on the owner's laptop, outside-the-app installs were held, likely by the installed
antivirus). A signature from a recognised certificate removes the "unknown publisher" text
and builds reputation over time.

## Options (checked against the cost guardrail: everything must stay free)

| Option | Cost | Fits? |
|---|---|---|
| SignPath Foundation (free code signing for open-source projects) | Free for qualifying open-source projects | **Recommended.** The project is public, MIT-licensed, and builds its installer in GitHub Actions, which is how SignPath works. |
| Microsoft Trusted Signing (Azure) | Monthly fee | No - costs money. Ask the owner first. |
| Buying an OV/EV certificate | Yearly fee | No - costs money. Ask the owner first. |
| Self-signed certificate | Free | No - Windows does not trust it; it does not remove the warning. |

SignPath's exact eligibility rules and form change over time: read their current open-source
page before applying. Typical requirements are an OSI-approved licence, a public repository,
published releases, and a build that runs in CI.

## Before applying (owner)

1. Publish a first release, so there is something to sign: bump the version in
   `apps/desktop/src-tauri/tauri.conf.json` and `Cargo.toml` if needed, then push a tag
   `v0.1.0`. `.github/workflows/release-windows.yml` and `android.yml` attach the Windows
   installer and the APK to the GitHub Release.
2. Make sure the repository has a README that explains what the app does (the landing page
   text in `apps/web/download.html` can be reused).

## Draft answers for the application form

- **Project name / URL:** the name from `/brand.json`; `https://github.com/` + the
  `repository` value in `/brand.json`.
- **What it is:** a free, local-first voice dictation app for Windows (Tauri, Rust, React),
  with an Android voice keyboard and a web app. Speech is recognised on the user's own
  computer by default; optional AI features use the user's own API key.
- **Licence:** MIT.
- **What is signed:** the Windows NSIS installer (`*-setup.exe`) and the app executable inside
  it, built by GitHub Actions from tagged commits (`release-windows.yml`).
- **Build:** fully automated in GitHub Actions on `windows-latest`; no manual build steps.
- **Who maintains it:** the repository owner.

## After approval (developer task)

Add the SignPath signing step to `release-windows.yml` after "Build installer", following
SignPath's GitHub Actions instructions, with their organisation/project ids stored as GitHub
Actions secrets (never in the repository). Then check on a clean Windows account that the
installer shows the signed publisher name.

## Android

The APK built by `android.yml` is signed with the Android debug key: fine for installing on
the owner's phone, not for Google Play. Publishing on Google Play needs a one-time developer
fee (ask the owner first) and a private release key stored as a GitHub secret.

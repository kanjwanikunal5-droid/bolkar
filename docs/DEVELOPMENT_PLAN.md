# Development plan

Stages run in order. A stage is complete only when its acceptance tests were actually run;
anything not run is listed as "not run" in [STATUS.md](STATUS.md).

| Stage | Contents | Acceptance (short) |
|---|---|---|
| 0 | Repo, CI, benchmark harness, hardware probe, golden fixtures, research delta | CI green; STATUS.md has measured numbers |
| 1 | **Windows dictation MVP** — push-to-talk, hands-free, pill, local whisper.cpp, LIGHT cleanup, clipboard insertion, history, tray | Dictate into Notepad, Word, Gmail (Scenario 1), VS Code, Terminal; clipboard restored; not in Win+V; 50 dictations without crash |
| 2 | LLM cleanup providers, consent, styles, app rules, dictionary packs, snippets, Fix popup, 150+ fixtures | >= 95% fixtures deterministic; LLM path never adds a number or name |
| 3 | Meeting notes (desktop): two tracks, live transcript, My Thoughts, summary with evidence, exports | Scenario 2; 60-minute recording; evidence checks pass |
| 4 | Web PWA on GitHub Pages | Public link works in Chrome and iPhone Safari |
| 5 | Android keyboard + in-person recorder; APK on GitHub Releases | Dictate into WhatsApp, Gmail, Chrome on the phone |
| 6 | Indic languages, Hinglish and Sindhi scripts, benchmarks on real recordings | Per-language results table from our own measurements |
| 7 | Command Mode, context awareness (opt-in, text only), scratchpad, file tagging | "make this a bulleted list" works in Word/Docs |
| 8 | Installer, updater, signing application, landing page | Fresh Windows account installs and dictates in 3 minutes |
| 9 | Hardening, failure tests, final report | Long-run and failure tests logged |

Stage 1 is the heart of the product and must be solid before anything else is built on it.

## Decisions carried into later stages

- Scratchpad shortcut: Ctrl+Win+N is taken on the owner's laptop; choose another in Stage 7.
- Light theme needs a darker accent teal than `#0FA3A3` to reach 4.5:1 contrast.
- Android unit tests and TypeScript lint/tests join CI when that code exists (Stages 4 and 5).

## Cost guardrail

Everything is designed to cost ₹0. Stop and ask the owner before: paid certificates, Apple
Developer membership, the Play Console fee, paid API usage, paid CI runners.

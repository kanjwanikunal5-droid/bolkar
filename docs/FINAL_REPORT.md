# Final report (Stages 0-9), 14 November 2026

Plain summary for the owner. Every number here was measured; the details, dates and commands
are in [STATUS.md](STATUS.md). Anything not tested says so.

## What exists

| Part | State |
|---|---|
| **Windows app** | Dictation into any app (hold Ctrl+Win), hands-free, clean-up rules, optional AI clean-up with your own Groq key, dictionary, snippets, history with original words kept, meeting notes, Command Mode (Alt+Shift+C), Scratchpad (Alt+Shift+N), first-run setup, update check. Installed and used on the owner's laptop. |
| **Speech models** | Turbo (default, fast, Roman Hinglish offline) and Large v3 (best measured Hinglish, needs AI clean-up on, about 3x slower). |
| **Web app** | Phone/browser dictation with your own key, published on GitHub Pages, plus a landing page. |
| **Android keyboard** | Builds in GitHub Actions, unit tests pass there. NOT yet tried on a phone (owner will install at the end). |
| **Release pipeline** | GPU Windows installer and APK built in GitHub Actions; attached to a GitHub Release when the owner pushes a version tag. Nothing published yet. |

## Measured results (highlights)

- **English:** 0.62% word errors with Large v3, 0.93% with Turbo (30 synthetic clips, 322
  words). Earlier, a finance meeting played through the speakers: 1.12%.
- **Hinglish:** best route (Large v3 or Groq cloud, plus AI script conversion and spelling
  polish) about 12-13.5% word errors on 30 synthetic clips (9.4-10.4% when equally normal
  spellings like wo/woh are accepted). The synthetic voices pronounce Hindi badly, so real
  speech should do better. On the owner's 5 real voice notes the output was nearly word for
  word, but **there is no score on his voice yet**: it needs his corrected text
  (`bench/my_recordings/owner/refs-DRAFT-please-correct.tsv`).
- **Speed:** about 0.3 s (Turbo) to 1 s (Large v3) per short dictation on the RTX 4050;
  0.85 s for a 20 s recording.
- **Stability:** 100 dictations in a row, 0 failures, identical text, no memory growth.
- **Command Mode in Word:** real bulleted and numbered lists, 12 of 12 with the owner away;
  9 of 12 on the final build while the owner was using the laptop (see "Open problems").

## Bugs found by testing and fixed today

1. Language check corrupted the "no speech" score, and real speech was deleted (up to 28 s).
2. Steady hiss produced an invented sentence ("So, let's go.").
3. Word missed list pastes when the clipboard was restored too early.
4. A test copy of the app handed over to the owner's running copy.
5. Wrongly encoded characters ("Â·") in the app's text.

## Open problems and things not done

- **Accuracy is not 100%** and no speech system reaches that; the goal is the lowest measured
  error. Next step that would help most: the owner corrects the draft text of his 5 notes
  (and records more), so errors on his own voice can be measured and targeted.
- Command Mode in Word failed 3 of 12 times on the final build while the owner was using the
  computer; re-test when the laptop is free (`scripts/selftest-word-command.ps1`).
- Not tested: Command Mode in Google Docs, an AI-only command end to end, the Scratchpad
  clicked through, the first-run download clicked through, the Android keyboard on a phone,
  the CI-built installer installed on a clean Windows account (Stage 8 acceptance "fresh
  account installs and dictates in 3 minutes" is NOT yet run).
- Not built: Android in-person recorder, file tagging, auto-installing updates.
- Not signed: the installer shows a SmartScreen warning until a code-signing application
  (free, SignPath; see `docs/CODE_SIGNING.md`) is made by the owner and approved.
- Rumbling background noise can still produce a stray "So".

## What the owner needs to do

1. Publish a version when ready: push a tag `v0.1.0` (this creates a public GitHub Release
   with the Windows installer and the APK).
2. Apply for free code signing (draft answers in `docs/CODE_SIGNING.md`).
3. Correct the draft text of the 5 voice notes so accuracy on your own voice can be scored.
4. Keep AI clean-up on if you use Large v3.

## Costs

₹0. All tools are free; Groq is used on its free tier with the owner's own key.

# How to use

This covers what exists today: **Windows dictation**. Meeting notes, the dictionary, snippets,
cloud providers, Android and the web app are not built yet (see [STATUS.md](STATUS.md)).

## Start the app

There is no installer yet (Stage 8). For now the app is started from the project folder:

```
pnpm desktop:dev
```

or, after `pnpm desktop:build`, by running the built program. A small icon appears near the
clock; closing the window keeps the app running there. Right-click the icon and choose **Quit**
to stop it.

## First time: get a speech model

Open **Settings -> Speech model** and press **Download** on one model. Models are large files
and stay on your computer.

| Model | Size | Use it when |
|---|---|---|
| Large v3 Turbo | 547 MB | You have a dedicated graphics card. Most accurate. |
| Small | 181 MB | A balance. |
| Base | 57 MB | Fastest, least accurate. |

## Dictate

1. Click where you want the text (an email, a chat box, a document).
2. **Hold `Ctrl` + `Win`** and speak. A small pill at the bottom of the screen shows it is listening.
3. **Let go.** The text is typed where your cursor is.

You can pause while holding the keys; nothing stops until you let go.

| Keys | What happens |
|---|---|
| Hold `Ctrl` + `Win` | Dictate while held |
| Double-tap `Ctrl` + `Win`, or `Ctrl` + `Win` + `H` | Hands-free: keeps listening until you press it again (20-minute limit) |
| `Esc` while listening | Cancel, nothing is typed |
| `Alt` + `Shift` + `Z` | Paste the last dictation again |
| `Alt` + `Shift` + `X` | Copy the last dictation |

## What gets cleaned up (Light mode)

- "um", "uh" and similar are removed.
- Self-corrections with a number, time, date or amount: "at 2 PM... actually make that 3 PM" -> "at 3 PM."
- "First, ... Second, ..." becomes a numbered list.
- Say "new line", "new paragraph", "full stop", "comma", "question mark" or "bullet point".

Choose **Literal** in Settings to keep every word exactly.

## If something goes wrong

- **Nothing was typed, the pill says "Copied".** You switched windows, or the app you were in runs
  as administrator. The text is on the clipboard: press `Ctrl` + `V`.
- **Wrong words.** Open **Dictation history**: every dictation is kept with the original
  recogniser output next to the cleaned text. Copy it from there.
- **"No microphone found" / "Microphone in use".** Check the microphone in Windows sound settings.
- **First word cut off** (common with Bluetooth headsets): switch on
  "Keep the microphone ready" in Settings.

## Privacy

Speech is recognised on your computer. Nothing is sent to the internet. Dictations are kept out
of Windows clipboard history (Win + V). Recordings are kept for 7 days so a dictation can be
replayed, then deleted; you can switch this off in Settings.

## Change the language or text size

**Settings -> Dictation -> Language** and **Settings -> Text size**. Accuracy in each language
has not been measured yet, so no quality figures are shown.

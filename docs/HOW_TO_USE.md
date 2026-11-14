# How to use

This covers what exists today: **Windows dictation**, the dictionary, snippets and optional AI
clean-up. Meeting notes, Android and the web app are not built yet (see [STATUS.md](STATUS.md)).

## Start the app

Double-click the installer (`Bolkar_0.1.0_x64-setup.exe`). It installs for your Windows user
only, needs no administrator rights, and adds **Bolkar** to the Start menu. Windows may show
"Windows protected your PC" because the installer is not code-signed yet: click **More info ->
Run anyway**.

Start it from the Start menu. A small icon appears near the clock; closing the window keeps the
app running there. Right-click the icon and choose **Quit** to stop it.

## First time: get a speech model

On this laptop the best model (Large v3 Turbo) is already in place. On another computer, open
**Settings -> Speech model** and press **Download** on one model. Models are large files and
stay on your computer.

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
| `Alt` + `Shift` + `F` | Fix the last dictation (the app learns new words from your fix) |

While you dictate, a small pill appears at the bottom of the screen: **✕** cancels, the
moving waveform shows it hears you, **✓** finishes (useful in hands-free mode).

## English and Hinglish

The default language is **English + Hinglish**: speak English and you get English; speak
Hindi mixed with English and you get Hinglish in Roman letters, the way you type in WhatsApp
("Bhai aaj ke liye sorry yaar, mera dimaag kaam nahi kar raha tha."). Press `Alt` + `Shift` +
`H` to switch between English + Hinglish, English only and Hinglish only (the pill shows which).
If you speak very quietly in a noisy place, choose the mode yourself: automatic detection is
less reliable there.

**Best Hinglish accuracy:** turn on AI clean-up and, in Settings, cloud speech recognition
(Groq, your own free key). On your own voice notes that combination came out almost word for
word; the offline route gets the meaning right but misspells more words (baut, karun ga).
With AI clean-up on, Hinglish spellings are also tidied (bahut, karunga, vishwas).

**Best Hinglish without sending your voice anywhere:** in Settings choose the speech model
**Large v3** and turn on AI clean-up (only the text goes to Groq). It is about 3 times slower
than Turbo (about 1 second per dictation) and needs AI clean-up to write Roman letters.

## Edit text by voice (Command Mode)

1. Select some text in any app (Word, Gmail, Notepad...).
2. Press `Alt` + `Shift` + `C`. The pill says "Say what to do".
3. Say the instruction, then press `Alt` + `Shift` + `C` again (`Esc` cancels).

Works without AI: "make this a bulleted list", "make it a numbered list", "all caps",
"lowercase", "title case", "sentence case", "make it one paragraph", "clean this up" (also
"bullet points bana do"). In Word and Google Docs lists become real lists. Anything else
("make it more formal", "make it shorter", "translate to English") needs AI clean-up turned on.
Undo with `Ctrl` + `Z` in the app, as usual.

## Scratchpad

Press `Alt` + `Shift` + `N` from anywhere: the app opens a notes page with the cursor ready.
Hold `Ctrl` + `Win` and speak to fill it. It is saved on this computer as you type.

## Android keyboard (type by voice in WhatsApp, Gmail, any app)

1. Download the APK from the project's GitHub page (Actions or Releases) and open it on the
   phone. Android asks to allow installing from your browser or Files app: allow it once.
2. Open the app and follow the steps on screen, top to bottom: turn the keyboard on, allow
   the microphone, choose the speech engine, paste your Groq key (optional), pick the language.
3. In any app, tap a text box, switch to the keyboard (globe key or the keyboard icon at the
   bottom), tap the big microphone key, speak, and tap it again. The text is typed for you.

Keys: globe = back to your normal keyboard, language key = English + Hinglish / English /
Hinglish, space, delete (hold to delete a word), enter.
For Hinglish in Roman letters choose **Groq** and tick **AI text steps**. The phone's own
recogniser needs no key but writes Hindi in Hindi letters unless AI text steps are on.

## On your phone (web app)

Open https://kanjwanikunal5-droid.github.io/bolkar/ in Chrome (Android) or Safari (iPhone).
The first time, paste your Groq key and choose a passphrase (the key is stored encrypted in
that browser only). Tap **Speak**, talk, tap **Stop**, then **Copy** and paste into WhatsApp or
any app. Use the browser's "Add to Home screen" to install it like an app. Phones cannot let a
web page type into other apps, so copy and paste is the way.

## Meeting notes

1. Open the app -> **Meeting notes**. Give a title, choose **My microphone** and/or
   **Computer sound** (for Zoom, Google Meet, Teams), tick the consent box, **Start recording**.
2. Type your own notes in **My thoughts** while you listen; they are kept exactly as written.
3. **Stop and make notes.** The recording is transcribed on this computer; with AI clean-up
   on, you get topics, decisions, action items (owner and deadline "Not specified" unless
   someone said them), numbers and next steps, each with a time. **Copy notes** to paste them
   anywhere. Use headphones if you can.

## Teach it your words

- **Dictionary:** add words the way you want them written (for example `EBITDA`, heard as
  "ebit da"). Finance and Tech word packs are already switched on.
- **Fix:** after a dictation, press `Alt` + `Shift` + `F` (or **Fix** in Dictation history),
  correct the text and save. The app offers to add the changed words to your dictionary.
- **Snippets:** a phrase of two or more words, such as "insert my address", is replaced by the
  saved text.

## Optional: better clean-up with your free Groq key

1. Create a key at https://console.groq.com/keys (free account).
2. In the app: **Settings -> AI clean-up -> Service: Groq**, paste the key, read the note,
   tick the box to agree, **Save**, then **Test**.

The key is kept in Windows Credential Manager. Only the text of a dictation is sent, never the
audio. Every answer is checked; if it adds anything you did not say, or takes longer than
2.5 seconds, the built-in rules are used instead. Everything sent is listed under
**Settings -> Data that left this computer**. Choose **Off** to stop it at any time.

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
- **First word cut off:** make sure "Keep the microphone ready" is on in Settings (it is on by
  default). With it off, the microphone needs about 1-2 seconds to start.
- **Microphone icon always showing in the taskbar:** that is "Keep the microphone ready". The
  app holds only the last 0.3 seconds in memory and saves nothing until you press the keys.

## Privacy

Speech is recognised on your computer and audio never leaves it. Text is sent only if you
switch on AI clean-up. Dictations are kept out
of Windows clipboard history (Win + V). Recordings are kept for 7 days so a dictation can be
replayed, then deleted; you can switch this off in Settings.

## Change the language or text size

**Settings -> Dictation -> Language** and **Settings -> Text size**. Accuracy in each language
has not been measured yet, so no quality figures are shown.

## Updates

Settings > Updates shows your version. Press **Check for updates** to ask GitHub whether a newer
version is published; you get a link to the download page. Automatic checking at start-up is
off unless you turn it on. Nothing is ever installed without you.

# How to use

This covers the **Windows app** (dictation, dictionary, snippets, AI clean-up, meeting notes).
The app was called Bolkar until version 0.3.0; it is now **Speak Flow**.

## Start the app

Double-click the installer (`Speak.Flow_0.4.0_x64-setup.exe` or similar). It installs for your
Windows user only, needs no administrator rights, and adds **Speak Flow** to the Start menu.
**Coming from Bolkar?** Uninstall Bolkar first (Settings -> Apps); your history, models,
settings and Groq key are kept and picked up by Speak Flow. Windows may show
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
moving waveform shows it hears you, **✓** finishes (useful in hands-free mode). When the text
has been typed the pill simply disappears; it only shows a message when something went wrong.

## Speak in points

Say "first thing ..., second thing ..." (or "point one ...", "pehli baat ..., doosri baat ...")
and you get a numbered list, with filler words removed. Example: "So first thing my name is
Kunal, second thing my roll number is 52" becomes

```
1. My name is Kunal
2. My roll number is 52
```

Say "bullet point ..." / "next point ..." for bullets. Everything else stays a **natural
paragraph**: connected sentences are never cut into one bullet each. If you want hands-free
dictation (`Ctrl` + `Win` + `H`) to turn short, separate requests into bullets by itself, choose
Settings -> Writing -> "Lists in hands-free mode" -> **Auto-detect lists** (or **Always use
bullets**). The default is **Natural writing**.

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

1. Open the app -> **Meeting notes** -> **Record**. Give a title, choose the **Microphone** and/or
   **System audio** (for Zoom, Google Meet, Teams) and, if you like, which device, switch on
   the permission switch, **Start recording**. Use headphones if you can.
2. With the graphics card in use, the **transcript so far** appears while you record (new lines
   about every 30 seconds). Without it, the transcript is made when you stop, so the computer
   stays fast during the call.
3. Type your own notes in **My thoughts** while you listen; they are kept exactly as written.
4. **Stop and make notes.** The rest is transcribed on this computer (meeting audio never leaves
   it). With AI clean-up on, you get a short paragraph per topic, decisions (Confirmed only when
   people agreed, otherwise Proposed), action items (owner and deadline "Not specified" unless
   someone said them; "tentative" when they said "maybe"), numbers and next steps. Each item
   has its time; click it to jump to the transcript line. **Quote found** means its exact words
   are in the transcript; **Check this** means a number, quote or word did not match. The app
   checks links, quotes, numbers, names and dates, not the meaning, so read important items
   against the transcript. Long meetings are summarised in parts and joined.
5. **Past meetings** lists every saved meeting (search by title, transcript or notes). Open one
   to rename it, correct a transcript line (the original is kept), **Write notes again**,
   **Transcribe again**, **Save as Markdown / text** (to Downloads\Speak Flow) or **Delete** it
   with its audio. If the app closed during a recording, the meeting appears as
   **Interrupted**: open it and choose **Transcribe recording**.

"Microphone" and "System audio" are the two sound sources, not people.

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

Without Groq, speech is recognised on your computer and audio never leaves it. With **Groq
speech recognition** on, the audio of each dictation is sent to Groq; with **AI clean-up** on,
the text of each dictation (and a meeting's transcript, for its notes) is sent. Meeting audio
always stays on this computer. Dictations are kept out of Windows clipboard history (Win + V).

Settings -> Privacy & data sets how long things are kept: dictation audio (7 days by default),
meeting audio (30 days), dictation history and meetings (forever unless you choose a limit). It
also lists every upload, and has the **diagnostics log**: a small file with what happened
(steps, timings, errors), never what you said. **Save diagnostics for support** puts a copy in
Downloads\Speak Flow to send to whoever helps you.

If another dictation app (for example Wispr Flow or Typeless) is running, Home shows a
warning: it may use the same keys. Quit it from the system tray if Ctrl + Win misbehaves.

## Change the language or text size

**Settings -> Dictation -> Language** and **Settings -> Text size**. Accuracy in each language
has not been measured yet, so no quality figures are shown.

## Settings and Groq

Settings save themselves the moment you change them; there is no Save button. To connect
Groq: Settings > Groq & AI, paste your key once, press Connect. The key is checked and kept in
Windows Credential Manager; you never type it again. Groq speech recognition and AI clean-up
then switch on (best accuracy); either can be switched off there. The sidebar switch changes
light / dark / follow Windows.

## Updates

Settings > Updates shows your version. Press **Check for updates** to ask GitHub whether a newer
version is published; you get a link to the download page. Automatic checking at start-up is
off unless you turn it on. Nothing is ever installed without you.

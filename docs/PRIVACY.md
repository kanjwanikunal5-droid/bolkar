# Privacy

What the app does with your voice and text, as built (version 0.4.2). Updated 17 Nov 2026.

## Principles

1. **Local by default.** Without Groq, speech is recognised on this computer and nothing leaves it.
2. **Cloud only by your choice.** A cloud service is used only after you connect it and agree
   to what is sent. Audio and text are separate switches.
3. **Your own keys.** The app ships with no API keys. People you share the app with get none of
   your keys or data.
4. **No telemetry.** The app does not report usage to anyone.
5. **No hidden recording.** The pill shows whenever a dictation is listening; a meeting shows
   "Recording" and needs your consent switch first.

## What leaves this computer, and when

| You turned on | What is sent | To |
|---|---|---|
| Nothing (default without a key) | Nothing. Speech is recognised by whisper.cpp on this computer. | - |
| Groq speech recognition | The **audio** of each dictation (compressed) | Groq |
| AI clean-up | The **text** of each dictation; for meeting notes, the meeting **transcript text**; Hinglish lines in Hindi letters, to be written in Roman letters | The provider you chose (Groq by default; a local Ollama model keeps text on this computer) |
| Update check | Only a request for the latest version number | GitHub |
| Model download | Only the request for the model file (checked by SHA-256) | Hugging Face |

Meeting **audio** is always recognised on this computer; it is never uploaded. Every upload is
listed in Settings > Privacy & data ("Data that left this computer").

## What is stored on this computer, and for how long

All of it is in `%APPDATA%\app.bolkar.voice` (the folder name is the app's internal id).

| Data | Kept | Setting |
|---|---|---|
| Dictation audio | 7 days by default (1, 7, 30 or 90) | Delete dictation audio after |
| Dictation text (original recogniser text and cleaned text, separately) | Forever by default (or 30/90/365 days) | Delete dictation history after |
| Meeting audio (two WAV files per meeting) | 30 days by default (1 day to 1 year, or forever) | Delete meeting audio after |
| Meeting transcript, your notes, AI notes (all versions) | Forever by default (or 30/90/365 days) | Delete meetings after |
| Diagnostics log (no speech content) | At most 1 MB; delete any time | Diagnostics log |

Retention runs at start-up and every 6 hours. A file is marked deleted only after it is really
gone; a failure stays listed and is written to the diagnostics log. Only files inside the app's
data folder are ever deleted, and a meeting that is being recorded is never touched.
A single meeting (or all meetings) can be deleted at once from Meeting notes > Past meetings,
with its audio, transcript and notes.

## Keys

- Desktop: Windows Credential Manager (`app.bolkar.voice/api-key/<provider>`), never in the
  database, files or logs. The diagnostics log removes anything that looks like a key.
- Android: Android Keystore.
- Web app: IndexedDB, encrypted (AES-GCM) either with your passphrase or with a
  non-extractable device key. This protects the key at rest; it does not protect against
  malicious script running inside the web app's own page (cross-site scripting). The page
  therefore loads no third-party scripts and has a strict Content-Security-Policy. On a shared
  computer, use the passphrase option. "Remove key" deletes both records.

## Things to know about cloud providers

- Free tiers have rate limits and their terms can change. Read the provider's privacy policy
  before you connect it.

## Recording other people

The app always asks you to confirm that the participants know they are being recorded and
never records automatically. Laws on recording conversations differ by place; this is not
legal advice. The track labels "Microphone" and "System audio" are sound sources, not people:
the microphone can pick up others in the room, and system audio is everything the computer plays.

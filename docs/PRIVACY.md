# Privacy

This describes the design the app is being built to. Items marked *(planned)* are not built yet;
today the repository contains only offline text-processing code and command-line tools.

## Principles

1. **Local by default.** Audio and text stay on your device.
2. **Cloud only by your choice.** A cloud provider is used only after you enable it and accept a
   consent screen naming the provider, what is sent, and the provider's data-use policy *(planned)*.
3. **Your own keys.** The app ships with no API keys. People you share the app with get none of
   your keys or data.
4. **No telemetry.** The app does not report usage to anyone.
5. **No hidden recording.** A visible indicator shows whenever the microphone is live *(planned)*.

## What the code does today

- `crates/core` and `bench` make no network connections and read no audio.
- The `probe` tool reads hardware details and prints them; it sends nothing anywhere.

## Planned controls

- Local-only switch that blocks all network use except model downloads from fixed addresses
  verified by checksum.
- Local log of every upload (provider, size, purpose), shown under "Data that left this device".
- Retention timers: audio 7 days, dictation text 90 days, meetings until deleted; delete single
  items or everything.
- Keys stored in Windows Credential Manager / Android Keystore, never in files or logs.
- Screen-context feature off by default, text-only, never screenshots, never password fields.

## Things to know about cloud providers

- Gemini's free tier: Google uses the content to improve its products.
- Free tiers have rate limits and their terms can change.

## Recording other people

The app will always show a consent reminder before a meeting recording and will never record
automatically. Laws on recording conversations differ by place; this is not legal advice.

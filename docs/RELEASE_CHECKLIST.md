# Release checklist - Speak Flow 0.4.4

Automated checks and quiet self-tests are recorded in `V0.4.4_TEST_REPORT.md`. The items below
need the owner at the laptop, on the real Windows profile (not the developer's sandbox). Each is
**not run** until the owner ticks it. About 30 minutes in total.

## Install / upgrade
- [ ] Quit Speak Flow (tray icon > Quit). Run `Speak Flow_0.4.4_x64-setup.exe`. Windows may say
      "Windows protected your PC": More info > Run anyway (the installer is not code-signed).
- [ ] Open the app: History, Dictionary (your words and "learned" words), Snippets and Meeting
      notes > Past meetings still show your old data.
- [ ] Settings > About shows 0.4.4.

## First words (the 0.4.3 fix, still to be proven in real use)
- [ ] 10 times after the laptop was idle for a few minutes: press Ctrl + Win + H, start speaking
      as soon as the start sound plays, say "One two three four five ...". The first word is
      there each time. Note any miss.
- [ ] 10 times in a row quickly (warm): same check.
- [ ] Once with Wispr Flow / Typeless running (they may hold the microphone).
- [ ] Help > Diagnostics log: lines "start: microphone was ready" / "opened now" and any
      "recording began with N ms of silence-zeros (cut)" - send these lines (no speech in them).

## Dictation into apps
- [ ] Notepad, Word, Chrome (Gmail), VS Code, Windows Terminal: hold Ctrl + Win, speak, release.
- [ ] A long dictation with "the key decisions were ... next steps are first ... second ...":
      paragraph + label lines + bullets/numbers, every word kept.
- [ ] "Bullet point milk bullet point bread": lines start with "•".

## Learning from corrections (needs a real text box)
- [ ] In Notepad: dictate a sentence with a misheard word, correct that word in place within 90 s,
      then dictate again: the word is now right (Dictionary shows it as "learned").
- [ ] Dictate into box A of a web form, click box B and type something private, wait 10 s:
      nothing from box B is learned (Dictionary unchanged).
- [ ] Settings > Writing > "Forget learned words": learned entries are gone, your own stay.

## Meetings
- [ ] A real 5-10 minute English call (Meet/Zoom) and a mixed English + Hinglish one: no line
      repeated, nothing important missing, notes' decisions cite the right lines; "Participants:
      not identified", "Audio sources: Microphone, System audio".
- [ ] Pause/resume if used; unplug and replug headphones during a recording.
- [ ] Reopen, edit a line, re-transcribe, export, delete a meeting.

## Security spot check
- [ ] Settings > Groq & AI > a custom provider with address `http://localhost.example.com/v1`:
      it is refused with a message.

## Rollback
If 0.4.4 misbehaves: uninstall it (Settings > Apps > Speak Flow > Uninstall; your data stays in
`%APPDATA%\app.bolkar.voice`) and run `Speak Flow_0.4.3_x64-setup.exe` from the v0.4.3 GitHub
release. 0.4.4 made no database schema change, so 0.4.3 reads the same data.

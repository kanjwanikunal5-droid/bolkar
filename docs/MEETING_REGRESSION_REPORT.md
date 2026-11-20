# Meeting regression report - 0.4.4

Date: 20 Nov 2026. All runs: the release build in its quiet self-test mode
(`APP_SELFTEST_MEETING`), a fresh test profile (never the owner's data), local Whisper
large-v3-turbo on the RTX 4050 for meeting audio, Groq for the notes. WER by the project's `wer`
tool (`app_core::metrics::normalise_for_scoring`: lower case, punctuation removed, "50,000" =
"50000"; number words and digits are NOT treated as equal, so "twentieth" vs "20th" counts as an
error).

## 1. The failure (audit 5.2)

0.4.3, English finance meeting (87.1 s, `meet-en/system.wav`, 13 turns, reference
`scripts/make-meeting.ps1`): "Pria will finish the interviews by next Friday." five times at
45-65 s; "Third, hiring...", the GST deadline/penalty and "Rahul will prepare the GST return by
the 18th" missing.

## 2. Cause, found by A/B on the same WAV

| Run (same WAV, same build 0.4.4-dev, same model) | Result |
|---|---|
| Test profile with the 346 pack words (hint = first 400 characters of the dictionary) | the 0.4.3 failure reproduced exactly: line repeated 5 times, 3 sentences lost |
| Same profile, dictionary emptied (no hint) | all 13 turns, in order, "Priya" correct |

So the recogniser's "initial prompt" (a glossary of dictionary words) triggered Whisper's
looping failure. It appeared in 0.4.3 because 0.4.3 added 300+ pack words to every dictionary,
and the hint takes the first 400 characters of it. The chunking, the live/final merge and the
echo filter were not the cause: with no hint the same pipeline produced every turn exactly once,
and the live and final passes did not overlap (each source range is read once; `next[]` offsets).

## 3. Fix

1. Recognition hints use only the user's own and learned words, never the packs; at most 200
   characters for meetings and 240 for dictation (`controller.rs`, `own_words_hint`).
2. If a piece still comes back with the same line three or more times in a row, it is recognised
   again without the hint, and that answer is used when it has no loop (`guard::has_loop`).
3. A run of 3+ identical lines in a row that remains is collapsed to one and logged; two copies
   are kept, because people do repeat themselves (`guard::collapse_line_loops`).

## 4. Results after the fix

| Meeting | 0.4.3 | 0.4.4 | Notes |
|---|---|---|---|
| English, 87 s, 179 words, test profile with packs | 20.67% WER, 1 line x5, 3 turns lost | **2.23% WER**, all 13 turns once | left: "PowerBA" (Power BI), "20th"/"18th" vs "twentieth"/"eighteenth" |
| English, same, every pack word marked as the user's own (full 200-char hint) | - | **2.23% WER**, all 13 turns once | no loop |
| Mixed English + Hinglish, 64 s, 110 words (reference rebuilt from the 12 corpus sentences it was made of, each match checked by hand) | 26.36% (25.45% tolerant) | 24.55% (23.64% tolerant) | English lines exact; Hinglish lines carry the local recogniser's misspellings |

## 5. Notes and decisions (audit 5.3, 5.5)

- The Rs 50,000 decision now cites the condition line ("only if ... above three times") and the
  approval line; it stays Confirmed only because both are cited and every detail is found. A
  decision with a detail that is not in its lines is shown as proposed: never "Confirmed" with
  "check this" (unit test from this exact case).
- "Participants: System audio" is gone: notes say "Participants: not identified" and list
  "Audio sources: System audio, Microphone" separately.
- Live transcript: still updated about every 30 s (25 s of new audio per piece), as the screen
  already says ("New lines appear about every 30 seconds"). Not shortened: shorter pieces cost
  accuracy and were not measured.

## 6. Not tested (needs the owner)

A real call (Meet/Zoom), 10/60-minute recordings, pause/resume, unplugging a device, a real
mixed-language meeting with an exact reference.

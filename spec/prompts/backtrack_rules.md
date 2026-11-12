# Backtracking rules (deterministic)

- **Version:** 1
- **Implemented in:** `crates/core/src/pipeline/backtrack.rs`
- **Golden cases:** `spec/fixtures/cleanup_core.json` (ids `bt-*`, `hg-002`)

Backtracking means handling a spoken self-correction: "3 PM... actually, 4 PM" becomes "4 PM".

## Principle

Deleting something a person really meant is worse than leaving a correction for them to tidy.
So a correction is applied only when it is clear. When it is not clear, **both parts are kept**
and the span is reported in `uncertain_spans`.

## Cue words

`actually`, `no wait`, `sorry`, `I mean`, `correction`, `scratch that`, `no no`,
Hindi/Hinglish `nahi nahi`, `matlab`. A cue may be followed by `make that` / `make it`.

A cue only counts when it **opens a clause** (it follows `. , ; : ! ? -` or `...`).
"I actually like it" and "I'm sorry about that" are not corrections.

## What gets replaced

1. **Like for like.** If the correction starts with a value (time, percentage, money amount,
   date, day, number), the most recent value *of the same kind* in the sentence before the cue
   is swapped. Works for every cue.
   - "Let's meet at 2 PM... actually make that 3 PM" -> "Let's meet at 3 PM."
   - "Meeting 3 baje hai, nahi nahi, 4 baje hai." -> "Meeting 4 baje hai."
   - Words left over after the value must either begin with a comma (they are appended) or
     repeat what already follows the swapped value. Otherwise it is not a like-for-like
     correction: "It costs 5 dollars. Actually, 3 people came yesterday." is left alone.
2. **Restart.** Only for the strong cues (`sorry`, `no wait`, `no no`, `nahi nahi`, `I mean`,
   `correction`). If the correction's first two words also occur in the previous sentence and
   both versions contain a value of the same kind, the sentence continues from that point.
   - "Revenue increased by 15 percent. Sorry, it increased by 12 percent." ->
     "Revenue increased by 12 percent."
3. **Scratch that.** Removes the previous sentence and keeps what follows.
   - "Send the report to Rahul. Scratch that, send it to Priya." -> "Send it to Priya."

Only the sentence directly before the cue can change. Corrections can chain
("2 PM, actually 3 PM, no wait, 4 PM" -> "4 PM").

## Known limits (handled by the LLM path when enabled)

- Corrections that replace a name or a phrase with no number/date in it
  ("send it to Rahul, sorry, Priya") are kept as spoken.
- Spelled-out times ("three thirty") are not recognised as times.
- Plain "no" is not treated as a cue; it is too common in ordinary speech.

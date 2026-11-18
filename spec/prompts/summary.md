# Meeting summary system prompt

- **Version:** 2 (17 Nov 2026: topic paragraphs, quotes, honest status; version 1 made every
  idea a bullet)
- **Used by:** meeting notes (Stage 3).
- **Output schema:** `spec/schemas/meeting_summary.schema.json`
- **Placeholders:** `{output_language}`, `{script}`.

## System prompt

```
You produce evidence-grounded meeting notes from a transcript. The transcript is a list of segments: {id, start, end, speaker, text}. The user's own notes ("my_thoughts") are higher-priority context but must never be presented as something another person said.
HARD RULES:
- Every item (topic narrative, key point, decision, action item, risk, question, number) MUST cite >=1 segment id in "evidence" that directly supports it. If you cannot cite evidence, leave the item out. Where you can, add "quote": a few exact words copied from a cited segment.
- Never invent owners, deadlines, numbers, names or decisions. If the owner or deadline is not explicitly stated in the cited segments, output null (shown as "Not specified"). Keep hedges: "maybe next Friday" stays "maybe next Friday".
- A decision is "confirmed" only if the transcript shows agreement ("agreed", "let's do that", "final", "we decided"); a suggestion, a question or a disputed point is "proposed".
- "speaker" values are sound sources ("Microphone", "System audio"), not people. Never turn them into names, and never guess who said something.
- Organise by TOPIC, not chronology. For each topic write "narrative": one short connected paragraph that explains what was discussed, keeping reasons, conditions and caveats ("only if the budget is approved"). Use "key_points" only for separate, distinct takeaways (usually 0-3); never one bullet per sentence. Do not repeat decisions or action items inside topics.
- Do not add follow-up tasks that nobody stated.
- Write in {output_language} using {script}. Hinglish stays in Roman letters. Keep exact figures ("sales up 12%", "Rs 50,000 additional marketing budget").
- If the input has a "part" field, summarise only those segments; other parts are summarised separately and merged later.
- Use [] or null for empty sections.
Return ONLY JSON matching meeting_summary.schema.json.
```

## Long meetings (implemented in the app)

The transcript is split into windows of about 12,000 characters (`summary::windows`). Each
window is summarised with the same prompt plus a "part" field; segment ids stay those of the
whole meeting. The window summaries are merged (`summary::merge`: same-title topics combined,
repeated items kept once) and then checked once against the whole transcript. If a window
fails, the notes say which part is missing and the transcript is always kept; the notes can
be written again later from the Past meetings list.

## Checks applied in code after the model answers (`summary::validate`)

- Every cited segment id exists; items with none are removed.
- Every `owner` appears word for word in a cited segment, otherwise it is set to null.
- Every `deadline` appears in a cited segment, otherwise null; with "maybe", "might",
  "shayad" etc. in the cited lines it is shown as "(tentative)".
- Every value in `numbers` appears in a cited segment, otherwise it is removed; every number
  inside any other item must appear too, otherwise the item is marked "check this".
- A `quote`, when given, must be found word for word in a cited segment ("quote found");
  otherwise "check this".
- An item that shares no words with its cited lines, or that sounds certain where the speaker
  hedged, is marked "check this".
- A "confirmed" decision needs words of agreement and no hedging or disagreement in the cited
  lines, otherwise it is shown as "proposed".
- These checks prove links, quotes, numbers, names and dates, not meaning. The app therefore
  says "source linked", "quote found" or "check this", never "verified".

## Golden test (topic organisation)

Transcript: "First we discussed the sales performance. Sales increased by 12 percent. Then we
talked about the marketing budget. The marketing team needs another 50,000 rupees. Then we
discussed the hiring process. Three interviews are pending. Finally, we decided to review
everything next Monday."

Expected: three topics (Sales: +12%; Marketing: additional Rs 50,000 requested; Recruitment:
3 interviews pending), one confirmed decision ("review next Monday"), **no invented tasks**,
every owner null.

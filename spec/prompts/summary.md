# Meeting summary system prompt

- **Version:** 1
- **Used by:** meeting notes (Stage 3).
- **Output schema:** `spec/schemas/meeting_summary.schema.json`
- **Placeholders:** `{output_language}`, `{script}`.

## System prompt

```
You produce evidence-grounded meeting notes from a transcript. The transcript is a list of segments: {id, start, end, speaker, text}. The user's own notes ("My Thoughts") are provided separately and are higher-priority context but must not be presented as things other people said.
HARD RULES:
- Every point, decision, action item, risk, question and number MUST cite ≥1 segment id in "evidence" that directly supports it. If you cannot cite evidence, do not include the item.
- Never invent owners, deadlines, numbers, names or decisions. If the owner or deadline is not explicitly stated in the cited segments, output null (the UI shows "Not specified").
- A decision is "confirmed" only if the transcript shows agreement (e.g., "let's do that", "agreed", "final", "we decided"); otherwise "proposed".
- Use speaker labels exactly as given (e.g., "Me", "Others", "Speaker 2"); never guess real names unless the name is stated in the transcript as that person's own self-identification or as the explicit owner.
- Organize discussion by TOPIC, not chronology. Merge duplicates. Never combine unrelated points into one paragraph.
- Do not add follow-up tasks that nobody stated.
- Keep important qualifications and conditions ("only if the budget is approved").
- Write in {output_language} using {script}. Be concise and specific; keep exact figures (e.g., "sales up 12%", "₹50,000 additional marketing budget").
- Omit empty sections (empty arrays).
Return ONLY JSON matching meeting_summary.schema.json.
```

## Long meetings

When the transcript is longer than the provider's context limit: summarise 10-minute windows
with the same schema, then merge the window summaries, keeping every evidence id.

## Validation applied in code after the model answers

- Every cited segment id exists.
- Every `owner` appears verbatim in at least one cited segment, otherwise it is set to null.
- Every `deadline` expression appears in a cited segment, otherwise it is set to null.
- Every value in `numbers` appears in a cited segment.
- Items that fail are dropped and listed in a `validation_dropped` debug log.
- Invalid JSON: one repair retry, then the transcript is shown with
  "Summary failed — retry / change provider".

## Golden test (topic organisation)

Transcript: "First we discussed the sales performance. Sales increased by 12 percent. Then we
talked about the marketing budget. The marketing team needs another 50,000 rupees. Then we
discussed the hiring process. Three interviews are pending. Finally, we decided to review
everything next Monday."

Expected: three topics (Sales: +12%; Marketing: additional ₹50,000 requested; Recruitment:
3 interviews pending), one confirmed decision ("review next Monday"), **no invented tasks**,
every owner null.

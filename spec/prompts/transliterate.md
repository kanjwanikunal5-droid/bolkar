# Transliteration prompt (Hinglish / Sindhi script conversion)

- **Version:** 1
- **Used by:** script handling (Stage 6).
- **Placeholders:** `{from}`, `{to}`.

## System prompt

```
Convert the script of the given text WITHOUT translating it. Source script: {from}. Target script: {to}. Keep English words in Latin script when target is Roman Hinglish. Keep numbers as digits. Return JSON {"text": string}.
```

## Notes

- A deterministic table-based transliteration is offered wherever one exists, and tests
  compare the two outputs.
- Sindhi Perso-Arabic output stays marked **experimental** until a native reader validates it.

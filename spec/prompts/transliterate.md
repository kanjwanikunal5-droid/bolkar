# Transliteration prompt (Hinglish / Sindhi script conversion)

- **Version:** 1
- **Used by:** script handling (Stage 6).
- **Placeholders:** `{from}`, `{to}`.

## System prompt

```
Convert the script of the given text WITHOUT translating it. Source script: {from}. Target script: {to}. Keep English words in Latin script when target is Roman Hinglish. Keep numbers as digits. Return JSON {"text": string}.
```

## Roman Hinglish system prompt (used by the app, version 2)

```
You convert Hindi written in Devanagari into Roman Hinglish, the way people type in WhatsApp in India. Change ONLY the script, never the words: do not translate, do not add, remove or reorder words, do not fix grammar. Rules:
1. Hindi words: write them in common WhatsApp-style Roman spelling, for example: है -> hai, हैं -> hain, था -> tha, नहीं -> nahi, मैं -> main, मुझे -> mujhe, कर रहा -> kar raha, काम -> kaam, आज -> aaj, क्या -> kya, यार -> yaar, भाई -> bhai, ठीक -> theek, समझ -> samajh, दिमाग -> dimaag, कल -> kal, बजे -> baje.
2. English words written in Devanagari (सोरी, मीटिंग, रिपोर्ट, प्लीज़, बजट) go back to normal English spelling (sorry, meeting, report, please, budget).
3. Words already in Latin letters stay exactly as they are. Keep numbers as digits and keep punctuation.
Return only JSON: {"text": string}
```

## Roman Hinglish with spelling system prompt (version 2)

One call instead of two (script conversion, then spelling polish): faster.
Version 2 (16 Nov 2026): hesitation sounds are written as "um"/"uh" so the filler rules
remove them (version 1 wrote the "um" of an English sentence as "am").

```
You turn speech-recognition output of Hinglish (Hindi mixed with English, possibly partly in Devanagari) into Roman Hinglish, the way people type on WhatsApp in India. Do not translate, do not add or remove sentences, never change the meaning.
Rules:
1. Hindi words: common WhatsApp-style Roman spelling, for example: है -> hai, नहीं -> nahi, मैं -> main, मुझे -> mujhe, कर रहा -> kar raha, बहुत -> bahut, करूँगा -> karunga, विश्वास -> vishwas, ज़्यादा -> zyada, पहले -> pehle, दोस्तों -> doston, वहाँ -> wahan, ठीक -> theek, यार -> yaar, भाई -> bhai.
2. English words written in Devanagari (मीटिंग, रिपोर्ट, प्लीज़) go back to normal English spelling (meeting, report, please). Words already in Latin letters stay as they are, except obvious Hinglish misspellings (baut -> bahut, karun ga -> karunga).
3. Fix a word only when it is clearly mis-heard from context; proper names keep their spelling unless obviously wrong. If unsure, leave the word as it is.
4. Never add a word that was not spoken. Keep numbers as digits and keep punctuation.
5. Hesitation sounds (उम, अम, अं, आं, उह) are written "um" or "uh", never as a word such as "am".
Return only JSON: {"text": string}
```

## Hinglish polish system prompt (version 2)

```
You correct speech-recognition output of Hinglish (Hindi and English mixed, written in Roman letters, the way people type on WhatsApp in India). Fix ONLY spelling and obvious mis-heard words; never change the meaning, never translate, never add or remove sentences.
Rules:
1. Write Hindi words in common WhatsApp spelling: bahut (not baut/bhoot), hai, hain, hoon, nahi, kya, kyun, mujhe, tujhe, mera, mere, kar raha, karunga, jaunga, aaunga, milenge, vishwas, zyada, thoda, sach, sachi, abhi, baad mein, pehle, isliye, doston, wahan, yahan, kaise, kuch, thik/theek, yaar, bhai.
2. Fix words that are clearly mis-heard from context, for example "jester" -> "just", "draa karun ga" -> "try karunga", "aung ba" -> "aaunga", "mire" -> "mere", "dosed to ne" -> "doston ne". Proper names: keep them, fixing only an obvious spelling (Mathiran -> Matheran).
3. Keep English words and English sentences exactly as English. Keep numbers, names and punctuation.
4. Never add a word that was not spoken and never drop one: each word of your answer is one word of the input, fixed or unchanged (joining "karun ga" into "karunga" is allowed). If a phrase does not make sense, leave its words as they are.
5. If you are not sure about a word, leave it unchanged. When a garbled word could be more than one real word (for example a time word: ghante, din, hafte, mahine), leave it unchanged.
Return only JSON: {"text": string}
```

## Notes

- A deterministic table-based transliteration is offered wherever one exists, and tests
  compare the two outputs.
- Sindhi Perso-Arabic output stays marked **experimental** until a native reader validates it.

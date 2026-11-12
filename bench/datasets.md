# Benchmark data

Nothing has been downloaded or measured yet. Licences must be checked on each dataset page
before any audio is downloaded; no dataset audio is ever committed to this repository.

| Set | Use | Languages |
|---|---|---|
| Owner's own recordings (`bench/my_recordings/`, git-ignored) | The numbers that matter most | 10 English, 10 Hinglish, 5 Sindhi, 5 Marathi/Tamil |
| Svarah (AI4Bharat) | Indian-accented English | en-IN |
| Kathbath (AI4Bharat) | Read speech | Indic |
| IndicVoices (AI4Bharat) | Natural speech | Indic incl. Sindhi |
| Vistaar (AI4Bharat) | Benchmark collection | Indic |
| FLEURS (Google) | Cross-check | Many |

## File format

Reference and hypothesis files are UTF-8 text, one recording per line:

```
rec001<TAB>the transcript of recording one
rec002<TAB>the transcript of recording two
```

## Rules

- Same normalisation on both sides (`app_core::metrics::normalise_for_scoring`).
- Report per engine, per language, per condition (quiet / noisy, read / spontaneous).
- Report the pooled corpus rate, not an average of per-recording rates.
- Synthetic (TTS) audio is for plumbing tests only and is never reported as accuracy.

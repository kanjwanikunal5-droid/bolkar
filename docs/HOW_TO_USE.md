# How to use

**There is no app to install yet.** The dictation app arrives in Stage 1. This page will become
the one-page guide (dictation, meeting recording, dictionary words, snippets, languages and
scripts, adding a free API key) once those features exist and have been tested.

## What you can try today

These run in a terminal opened in the project folder.

See how the text cleanup behaves on a sentence:

```
cargo run -p app-bench --bin clean -- "So, um, let's meet at 2 PM... actually make that 3 PM"
```

See what this computer can run:

```
cargo run -p app-bench --bin probe
```

Score a recogniser against reference transcripts (two files of `id<TAB>text` lines):

```
cargo run -p app-bench --bin wer -- --ref refs.tsv --hyp hyps.tsv
```

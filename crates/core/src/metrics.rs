//! Word / character error rate, used by the benchmark harness and the per-language
//! quality badges. Both sides are normalised with the SAME rules before scoring.

use serde::{Deserialize, Serialize};
use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

/// Normalisation applied to reference and hypothesis alike:
/// NFC, lower case, "%" -> "percent", digit grouping removed ("50,000" -> "50000"),
/// apostrophes and zero-width joiners dropped, all other punctuation -> space.
pub fn normalise_for_scoring(s: &str) -> String {
    let chars: Vec<char> = s.nfc().collect::<String>().to_lowercase().chars().collect();
    let mut out = String::with_capacity(s.len());
    for (i, &c) in chars.iter().enumerate() {
        let prev_digit = i > 0 && chars[i - 1].is_ascii_digit();
        let next_digit = chars.get(i + 1).is_some_and(|n| n.is_ascii_digit());
        if c == ',' && prev_digit && next_digit {
            continue; // 50,000 -> 50000
        }
        if c == '.' && prev_digit && next_digit {
            out.push('.'); // keep 3.5 as one token
            continue;
        }
        if c == '%' {
            out.push_str(" percent ");
        } else if matches!(c, '\'' | '\u{2019}' | '\u{200C}' | '\u{200D}') {
            // don't -> dont; joiners carry no sound
        } else if c.is_alphanumeric() || is_combining_mark(c) {
            out.push(c);
        } else {
            out.push(' ');
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EditCounts {
    pub substitutions: usize,
    pub deletions: usize,
    pub insertions: usize,
    pub hits: usize,
    /// Number of tokens in the reference.
    pub reference_len: usize,
}

impl EditCounts {
    pub fn errors(&self) -> usize {
        self.substitutions + self.deletions + self.insertions
    }

    /// Error rate as a fraction (0.12 = 12%). Can exceed 1.0 when the hypothesis has many
    /// extra tokens. `None` when the reference is empty.
    pub fn rate(&self) -> Option<f64> {
        (self.reference_len > 0).then(|| self.errors() as f64 / self.reference_len as f64)
    }

    pub fn add(&mut self, other: &EditCounts) {
        self.substitutions += other.substitutions;
        self.deletions += other.deletions;
        self.insertions += other.insertions;
        self.hits += other.hits;
        self.reference_len += other.reference_len;
    }
}

/// Levenshtein alignment with a backtrace so substitutions, deletions and insertions are
/// counted separately.
fn align<T: PartialEq>(reference: &[T], hypothesis: &[T]) -> EditCounts {
    let (n, m) = (reference.len(), hypothesis.len());
    let width = m + 1;
    let mut cost = vec![0usize; (n + 1) * width];
    for i in 0..=n {
        cost[i * width] = i;
    }
    for (j, cell) in cost.iter_mut().enumerate().take(width) {
        *cell = j;
    }
    for i in 1..=n {
        for j in 1..=m {
            let sub = cost[(i - 1) * width + (j - 1)]
                + usize::from(reference[i - 1] != hypothesis[j - 1]);
            let del = cost[(i - 1) * width + j] + 1;
            let ins = cost[i * width + (j - 1)] + 1;
            cost[i * width + j] = sub.min(del).min(ins);
        }
    }

    let mut counts = EditCounts {
        reference_len: n,
        ..Default::default()
    };
    let (mut i, mut j) = (n, m);
    while i > 0 || j > 0 {
        let here = cost[i * width + j];
        if i > 0
            && j > 0
            && reference[i - 1] == hypothesis[j - 1]
            && here == cost[(i - 1) * width + (j - 1)]
        {
            counts.hits += 1;
            i -= 1;
            j -= 1;
        } else if i > 0 && j > 0 && here == cost[(i - 1) * width + (j - 1)] + 1 {
            counts.substitutions += 1;
            i -= 1;
            j -= 1;
        } else if i > 0 && here == cost[(i - 1) * width + j] + 1 {
            counts.deletions += 1;
            i -= 1;
        } else {
            counts.insertions += 1;
            j -= 1;
        }
    }
    counts
}

/// Word-level edit counts after normalisation.
pub fn word_errors(reference: &str, hypothesis: &str) -> EditCounts {
    let r = normalise_for_scoring(reference);
    let h = normalise_for_scoring(hypothesis);
    let rw: Vec<&str> = r.split_whitespace().collect();
    let hw: Vec<&str> = h.split_whitespace().collect();
    align(&rw, &hw)
}

/// Hinglish has no standard Roman spelling: "wo" and "woh", "to" and "toh", "jane" and "jaane"
/// are all normal on WhatsApp. For the *spelling-tolerant* score, each listed variant is
/// mapped to one form on both sides. Only pure spelling variants of the same word are listed;
/// different words ("tumhe" / "tumhare", "main" / "mein") are never merged. The strict score
/// (`word_errors`) does not use this.
const HINGLISH_VARIANTS: &[(&str, &str)] = &[
    ("woh", "wo"),
    ("vo", "wo"),
    ("toh", "to"),
    ("yeh", "ye"),
    ("jaane", "jane"),
    ("jaana", "jana"),
    ("jaayenge", "jayenge"),
    ("jaayega", "jayega"),
    ("jaayegi", "jayegi"),
    ("doonga", "dunga"),
    ("parega", "padega"),
    ("mehenga", "mehnga"),
    ("mehanga", "mehnga"),
    ("nahin", "nahi"),
    ("hun", "hoon"),
    ("hu", "hoon"),
    ("bohot", "bahut"),
    ("bahot", "bahut"),
    ("jyada", "zyada"),
    ("thik", "theek"),
    ("kyu", "kyun"),
    ("acha", "accha"),
    ("achha", "accha"),
    ("aya", "aaya"),
    ("aj", "aaj"),
    ("hamein", "humein"),
    ("tumhein", "tumhe"),
    ("chalen", "chalein"),
    ("pehle", "pahle"),
    ("sham", "shaam"),
];

fn hinglish_canonical(word: &str) -> &str {
    HINGLISH_VARIANTS
        .iter()
        .find(|(from, _)| *from == word)
        .map_or(word, |(_, to)| to)
}

/// Word-level edit counts with Hinglish spelling variants treated as the same word.
pub fn word_errors_hinglish(reference: &str, hypothesis: &str) -> EditCounts {
    let r = normalise_for_scoring(reference);
    let h = normalise_for_scoring(hypothesis);
    let rw: Vec<&str> = r.split_whitespace().map(hinglish_canonical).collect();
    let hw: Vec<&str> = h.split_whitespace().map(hinglish_canonical).collect();
    align(&rw, &hw)
}

/// Character-level edit counts after normalisation (Unicode code points, single spaces kept).
pub fn char_errors(reference: &str, hypothesis: &str) -> EditCounts {
    let r: Vec<char> = normalise_for_scoring(reference).chars().collect();
    let h: Vec<char> = normalise_for_scoring(hypothesis).chars().collect();
    align(&r, &h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_after_normalisation() {
        let c = word_errors(
            "Sales rose 12%, to ₹50,000.",
            "sales rose 12 percent to 50000",
        );
        assert_eq!(c.errors(), 0);
        assert_eq!(c.reference_len, 6);
    }

    #[test]
    fn counts_each_error_type() {
        // one wrong word and one missing word
        let c = word_errors("the cat sat on the mat", "the cat sit on mat");
        assert_eq!(
            (c.substitutions, c.deletions, c.insertions, c.hits),
            (1, 1, 0, 4)
        );
        assert_eq!(c.reference_len, 6);
        assert!((c.rate().unwrap() - 2.0 / 6.0).abs() < 1e-9);
        // one extra word
        let c = word_errors("hello world", "hello big world");
        assert_eq!(
            (c.substitutions, c.deletions, c.insertions, c.hits),
            (0, 0, 1, 2)
        );
    }

    #[test]
    fn empty_hypothesis_is_all_deletions() {
        let c = word_errors("one two three", "");
        assert_eq!(c.deletions, 3);
        assert!((c.rate().unwrap() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn rate_can_exceed_one() {
        let c = word_errors("hello", "hello there my dear friend");
        assert!((c.rate().unwrap() - 4.0).abs() < 1e-9);
    }

    #[test]
    fn empty_reference_has_no_rate() {
        assert_eq!(word_errors("", "anything").rate(), None);
    }

    #[test]
    fn hinglish_variants_only_merge_spellings_of_one_word() {
        let r = "Bhai woh video dekha kya, toh chalo.";
        assert_eq!(
            word_errors(r, "Bhai wo video dekha kya, to chalo.").errors(),
            2
        );
        assert_eq!(
            word_errors_hinglish(r, "Bhai wo video dekha kya, to chalo.").errors(),
            0
        );
        // Different words stay different.
        assert_eq!(
            word_errors_hinglish("tumhare saath", "tumhe saath").errors(),
            1
        );
        assert_eq!(word_errors_hinglish("main ghar", "mein ghar").errors(), 1);
    }

    #[test]
    fn character_rate() {
        // "kitten" -> "sitting": 3 edits over 6 reference characters
        let c = char_errors("kitten", "sitting");
        assert_eq!(c.errors(), 3);
        assert!((c.rate().unwrap() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn indic_text_is_compared_in_nfc() {
        // क़ written precomposed (U+0958) vs as क + nukta: same text, zero errors.
        let c = word_errors("\u{0958}लम", "\u{0915}\u{093C}लम");
        assert_eq!(c.errors(), 0);
        // Danda is punctuation and is ignored.
        assert_eq!(word_errors("कल मीटिंग है।", "कल मीटिंग है").errors(), 0);
        // A wrong vowel sign is one substitution.
        assert_eq!(word_errors("कल मीटिंग है", "कल मीटींग है").substitutions, 1);
    }
}

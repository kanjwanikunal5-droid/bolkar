//! Offline Hinglish spelling rules: Whisper's spellings of common Hindi words -> the usual
//! WhatsApp spelling, and future-tense verbs split in two ("karun ga" -> "karunga").
//!
//! Used only on text that was recognised as Hinglish. Like the rest of the clean-up, it never
//! guesses: a word is changed only when the table names it and the word has no other common
//! meaning (so "bhoot", which is also "ghost", is left alone; the AI polish handles it).

use std::sync::LazyLock;

use regex::Regex;

/// (spelling seen in recogniser output, usual WhatsApp spelling). Lower case.
const SPELLINGS: &[(&str, &str)] = &[
    ("baut", "bahut"),
    ("bohot", "bahut"),
    ("bohut", "bahut"),
    ("bahot", "bahut"),
    ("bahoot", "bahut"),
    ("bhut", "bahut"),
    ("wishwas", "vishwas"),
    ("wishwaas", "vishwas"),
    ("wishwaz", "vishwas"),
    ("wishwaaz", "vishwas"),
    ("vishvas", "vishwas"),
    ("vishvaas", "vishwas"),
    ("vishwaas", "vishwas"),
    ("zyaada", "zyada"),
    ("zyaadha", "zyada"),
    ("zyadha", "zyada"),
    ("zahada", "zyada"),
    ("zahadah", "zyada"),
    ("jyada", "zyada"),
    ("jyaada", "zyada"),
    ("pahle", "pehle"),
    ("pehele", "pehle"),
    ("kyonki", "kyunki"),
    ("kyuki", "kyunki"),
    ("kyoki", "kyunki"),
    ("wahaan", "wahan"),
    ("vahaan", "wahan"),
    ("vahan", "wahan"),
    ("vohan", "wahan"),
    ("yahaan", "yahan"),
    ("nahin", "nahi"),
    ("nahee", "nahi"),
    ("chaahiye", "chahiye"),
    ("chahie", "chahiye"),
    ("samaj", "samajh"),
    ("dhyaan", "dhyan"),
    ("thodaa", "thoda"),
    ("achha", "accha"),
    ("sacchi", "sachi"),
    ("sachchi", "sachi"),
    ("dosto", "doston"),
    ("bad mein", "baad mein"),
    ("mea", "mein"),
];

/// "karun ga", "jaun gi", "aaun ge", "paun ga" -> one word. Only after a verb stem ending in
/// "un"/"oon"/"aun"/"on", and only for ga/gi/ge (the future-tense ending).
static SPLIT_FUTURE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b([a-z]+?(?:un|oon|aun|on))\s+(g[aie])\b").unwrap());

/// "aung ba" / "aun ba" is how Whisper hears "aaunga" (measured on the owner's voice notes).
static AUNGA: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)\baung?\s+ba\b").unwrap());

static WORD: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[A-Za-z]+").unwrap());

fn keep_case(original: &str, replacement: &str) -> String {
    let mut chars = original.chars();
    match chars.next() {
        Some(c) if c.is_uppercase() => {
            let mut r = replacement.chars();
            r.next()
                .map(|f| f.to_uppercase().collect::<String>() + r.as_str())
                .unwrap_or_default()
        }
        _ => replacement.to_string(),
    }
}

/// Common Hindi words written in Roman letters that are not also everyday English words
/// ("main", "to", "me" are left out on purpose).
const HINDI_MARKERS: &[&str] = &[
    "hai", "hain", "hoon", "hu", "nahi", "nahin", "kya", "kyun", "kyunki", "mujhe", "tujhe",
    "mera", "mere", "meri", "tera", "tere", "raha", "rahi", "rahe", "karunga", "karna", "karo",
    "bhai", "yaar", "abhi", "baad", "mein", "toh", "bahut", "accha", "acha", "theek", "thik",
    "aaj", "kal", "kuch", "sab", "wahan", "yahan", "lekin", "aur", "bhi", "tha", "thi", "hoga",
    "jaana", "jana", "baat", "kaise", "kaun", "kab", "humein", "hum", "tum", "aap", "unka",
];

/// True for Roman text that reads as Hinglish: at least two different common Hindi words.
pub fn looks_hinglish(text: &str) -> bool {
    let mut seen: Vec<String> = Vec::new();
    for w in WORD.find_iter(text) {
        let w = w.as_str().to_lowercase();
        if HINDI_MARKERS.contains(&w.as_str()) && !seen.contains(&w) {
            seen.push(w);
            if seen.len() >= 2 {
                return true;
            }
        }
    }
    false
}

/// Applies the spelling rules. Text that is not Hinglish should not be passed in.
pub fn normalise(text: &str) -> String {
    let joined =
        SPLIT_FUTURE.replace_all(text, |c: &regex::Captures| format!("{}{}", &c[1], &c[2]));
    let joined = AUNGA.replace_all(&joined, |c: &regex::Captures| keep_case(&c[0], "aaunga"));
    // Two-word entries first.
    let mut out = joined.into_owned();
    for (from, to) in SPELLINGS.iter().filter(|(f, _)| f.contains(' ')) {
        let re = Regex::new(&format!(r"(?i)\b{}\b", regex::escape(from))).unwrap();
        out = re
            .replace_all(&out, |c: &regex::Captures| keep_case(&c[0], to))
            .into_owned();
    }
    WORD.replace_all(&out, |c: &regex::Captures| {
        let w = &c[0];
        let lower = w.to_lowercase();
        match SPELLINGS
            .iter()
            .find(|(from, _)| !from.contains(' ') && *from == lower)
        {
            Some((_, to)) => keep_case(w, to),
            None => w.to_string(),
        }
    })
    .into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tells_hinglish_from_english() {
        assert!(looks_hinglish("Bhai main abhi busy hoon"));
        assert!(looks_hinglish("Kal meeting hai, time pe aa jana."));
        assert!(!looks_hinglish(
            "Please send me the main report to the team."
        ));
        assert!(!looks_hinglish(
            "Can we reschedule our call to Thursday afternoon?"
        ));
    }

    #[test]
    fn fixes_common_misspellings() {
        assert_eq!(normalise("Baut maza aaya"), "Bahut maza aaya");
        assert_eq!(
            normalise("Mujhe bhai, wishwaaz tha."),
            "Mujhe bhai, vishwas tha."
        );
        assert_eq!(
            normalise("main try karun ga, okay?"),
            "main try karunga, okay?"
        );
        assert_eq!(normalise("main baad mein aung ba"), "main baad mein aaunga");
        assert_eq!(normalise("Pahle se pata tha"), "Pehle se pata tha");
        assert_eq!(normalise("main jaun gi"), "main jaungi");
    }

    #[test]
    fn leaves_ambiguous_and_english_words_alone() {
        for s in [
            "main bhoot chill kar raha hoon",
            "Please send the report on Monday.",
            "Turn on the garage lights at noon.",
            "Kal milte hain, theek hai?",
        ] {
            assert_eq!(normalise(s), s);
        }
    }
}

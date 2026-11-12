use unicode_normalization::UnicodeNormalization;

/// Unicode NFC, unified line endings and ellipses, single spaces, trimmed.
pub fn normalise(raw: &str) -> String {
    let s: String = raw.nfc().collect();
    let s = s
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\u{00A0}', " ")
        .replace('\u{2026}', "...");

    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for c in s.chars() {
        if c == ' ' || c == '\t' {
            if !prev_space {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(c);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapses_spaces_and_unifies_ellipsis() {
        assert_eq!(normalise("  Hello \t world\u{2026}  "), "Hello world...");
    }

    #[test]
    fn composes_to_nfc() {
        // "e" + combining acute -> single code point
        assert_eq!(normalise("cafe\u{0301}"), "caf\u{00E9}");
    }
}

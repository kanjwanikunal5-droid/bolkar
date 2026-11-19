//! Learning dictionary words from the user's own corrections.
//!
//! When a dictation is fixed by hand, the word-level differences become suggestions such as
//! "ebit da" -> "EBITDA". Nothing is added without the user's confirmation, and only short
//! replacements (1 to 3 words) are suggested; rewritten sentences are ignored.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Suggestion {
    /// How it was written before (what the recogniser heard), lower case.
    pub heard: String,
    /// How the user wants it written.
    pub term: String,
}

fn words(s: &str) -> Vec<String> {
    s.split_whitespace()
        .map(|w| {
            w.trim_matches(|c: char| !c.is_alphanumeric() && c != '&' && c != '+' && c != '#')
                .to_string()
        })
        .filter(|w| !w.is_empty())
        .collect()
}

/// Word-level longest-common-subsequence alignment; returns the changed blocks.
fn changed_blocks(a: &[String], b: &[String]) -> Vec<(Vec<String>, Vec<String>)> {
    let key = |w: &String| w.to_lowercase();
    let (n, m) = (a.len(), b.len());
    let mut lcs = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if a[i] == b[j] {
                lcs[i + 1][j + 1] + 1
            } else {
                lcs[i + 1][j].max(lcs[i][j + 1])
            };
        }
    }
    let mut blocks = Vec::new();
    let (mut i, mut j) = (0, 0);
    let (mut da, mut db) = (Vec::new(), Vec::new());
    while i < n || j < m {
        if i < n && j < m && a[i] == b[j] {
            if !da.is_empty() || !db.is_empty() {
                blocks.push((std::mem::take(&mut da), std::mem::take(&mut db)));
            }
            i += 1;
            j += 1;
        } else if j < m && (i == n || lcs[i][j + 1] >= lcs[i + 1][j]) {
            db.push(b[j].clone());
            j += 1;
        } else {
            da.push(a[i].clone());
            i += 1;
        }
    }
    if !da.is_empty() || !db.is_empty() {
        blocks.push((da, db));
    }
    // Case-only differences count as changes too ("ebitda" -> "EBITDA"), which the exact
    // comparison above already caught; drop blocks that are identical ignoring case AND
    // spelling only differs in case for ordinary lower-case words.
    blocks
        .into_iter()
        .filter(|(x, y)| {
            let same = x.len() == y.len() && x.iter().zip(y).all(|(p, q)| key(p) == key(q));
            // A pure capitalisation change is only worth learning when the new form is not
            // simply "first letter upper case" (sentence starts).
            !same || y.iter().any(|w| w.chars().skip(1).any(char::is_uppercase))
        })
        .collect()
}

/// Suggests dictionary entries from an edit of `before` into `after`.
pub fn suggest(before: &str, after: &str) -> Vec<Suggestion> {
    let (a, b) = (words(before), words(after));
    let mut out: Vec<Suggestion> = Vec::new();
    for (heard, fixed) in changed_blocks(&a, &b) {
        if heard.is_empty() || fixed.is_empty() || heard.len() > 3 || fixed.len() > 3 {
            continue; // insertions, deletions and rewrites are not vocabulary
        }
        let term = fixed.join(" ");
        if term
            .chars()
            .all(|c| c.is_ascii_digit() || c == ',' || c == '.')
        {
            continue; // numbers are facts, not vocabulary
        }
        let s = Suggestion {
            heard: heard.join(" ").to_lowercase(),
            term,
        };
        if !out.contains(&s) {
            out.push(s);
        }
    }
    out
}

/// How a learned correction is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Use {
    /// The misheard form is not an ordinary word: replace it automatically from now on.
    Replace,
    /// The misheard form is an ordinary word ("cloud" -> "Claude"): only steer the recogniser
    /// towards the term; a real "cloud" is never changed.
    HintOnly,
    /// An ordinary word changed into another ordinary word: a style edit, not a mishearing.
    Skip,
}

/// Frequent English and Hinglish words. A correction of one of these is never applied
/// automatically (it may be the right word elsewhere).
const COMMON: &str = "a about above after again against all almost also always am an and any are around as ask at away back bad be because been before being below best better between big both bring but buy by call came can cannot case change check clear close come could course cut day did do does doing done down draft during each early easy either else end enough even ever every example face fact far few file final find fine first five for form found four free from full get give go going good got great had half hand has have he head hear help her here him his home hope how i if important in into is it its job just keep kind know last late later learn least leave left less let life like line little long look lot made make man many may me mean meet might mind more most move much must my name need never new next nice no not note now number of off often ok okay old on once one only open or order other our out over own page part people place plan play please point put question quite rather read ready real really right road room round run said same say see seem send set she should show side simple since small so some something soon sorry start state still stop such sure system take talk team tell test than thank thanks that the their them then there these they thing things think this those though three through time to today together too top try turn two under until up us use very want was way we week well were what when where which while who why will with without word work would write wrong year yes yet you your yours zero cloud rate red read sale sell tax cost price total value plus minus per cent ok bhai yaar hai hain tha thi the ho hoon hun main mein mujhe mera meri mere tum tu tera aap kya kab kahan kaise kyun nahi na haan ji aur par lekin toh to bhi ki ka ke ko se ne wala wali kar karo karna kiya diya liya raha rahi rahe gaya gayi abhi phir ek do teen char baat kaam din ghar log sab kuch bahut thoda accha theek chal chalo dekh dekho bol bolo";

fn is_common(word: &str) -> bool {
    let w = word.to_lowercase();
    COMMON.split(' ').any(|c| c == w)
}

/// Decides how a learned correction is used (see [`Use`]).
pub fn classify(s: &Suggestion) -> Use {
    let heard_common = s.heard.split_whitespace().all(is_common);
    let term_special = s.term.split_whitespace().any(|w| {
        !is_common(w)
            || w.chars().skip(1).any(char::is_uppercase)
            || w.chars().any(|c| c.is_ascii_digit())
    });
    match (heard_common, term_special) {
        (false, _) => Use::Replace,
        (true, true) => Use::HintOnly,
        (true, false) => Use::Skip,
    }
}

/// Finds the dictated text inside a text box that the user may have edited since: returns
/// that part of the box (words joined by single spaces) when it still matches at least 60% of
/// the dictated words in order, else None. Used to learn from corrections made in the app.
pub fn find_edited(inserted: &str, field: &str) -> Option<String> {
    let a = words(inserted);
    let f = words(field);
    let n = a.len();
    if n == 0 || f.is_empty() {
        return None;
    }
    let low = |w: &String| w.to_lowercase();
    let a_low: Vec<String> = a.iter().map(low).collect();
    let f_low: Vec<String> = f.iter().map(low).collect();
    let max_len = n + n / 3 + 3;
    let mut best: Option<(usize, usize, usize)> = None; // (matched, start, end)
    for k in 0..n.min(3) {
        for (j, w) in f_low.iter().enumerate() {
            if *w != a_low[k] || j < k {
                continue;
            }
            let start = j - k;
            let end = (start + max_len).min(f.len());
            let window = &f_low[start..end];
            // Longest common subsequence, keeping where it last matched.
            let m = window.len();
            let mut dp = vec![vec![0u16; m + 1]; n + 1];
            for x in 1..=n {
                for y in 1..=m {
                    dp[x][y] = if a_low[x - 1] == window[y - 1] {
                        dp[x - 1][y - 1] + 1
                    } else {
                        dp[x - 1][y].max(dp[x][y - 1])
                    };
                }
            }
            let matched = dp[n][m] as usize;
            // The region ends after the last window word that still adds to the match.
            let mut last = m;
            while last > 0 && dp[n][last - 1] as usize == matched {
                last -= 1;
            }
            if best.is_none_or(|b| matched > b.0) {
                best = Some((matched, start, start + last));
            }
        }
    }
    let (matched, start, end) = best?;
    (matched * 10 >= n * 6).then(|| f[start..end].join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrections_in_the_app_are_found_and_classified() {
        let inserted = "I have seen that the AKS is not good in Henglish and in English.";
        let field = "Hi Claude,\n\nI have seen that the accuracy is not good in Hinglish and in English. Please fix it.";
        let region = find_edited(inserted, field).unwrap();
        let s = suggest(inserted, &region);
        assert_eq!(
            s,
            vec![
                Suggestion {
                    heard: "aks".into(),
                    term: "accuracy".into()
                },
                Suggestion {
                    heard: "henglish".into(),
                    term: "Hinglish".into()
                },
            ]
        );
        assert_eq!(classify(&s[0]), Use::Replace);
        assert_eq!(classify(&s[1]), Use::Replace);
        assert_eq!(
            classify(&Suggestion {
                heard: "cloud".into(),
                term: "Claude".into()
            }),
            Use::HintOnly,
            "a real word is never replaced automatically"
        );
        assert_eq!(
            classify(&Suggestion {
                heard: "good".into(),
                term: "great".into()
            }),
            Use::Skip,
            "a style edit is not a mishearing"
        );
        assert_eq!(
            classify(&Suggestion {
                heard: "road number".into(),
                term: "roll number".into()
            }),
            Use::HintOnly,
            "\"road\" is a real word: steer the recogniser, never replace"
        );
        // The text was sent or deleted: nothing matches.
        assert_eq!(find_edited(inserted, "Something else entirely here."), None);
        // Unchanged text is found unchanged.
        assert_eq!(
            find_edited(
                "send the report today",
                "ok so send the report today thanks"
            )
            .as_deref(),
            Some("send the report today")
        );
    }

    #[test]
    fn learns_replaced_words() {
        let s = suggest(
            "The ebit da margin and the super base project are ready.",
            "The EBITDA margin and the Supabase project are ready.",
        );
        assert_eq!(
            s,
            vec![
                Suggestion {
                    heard: "ebit da".into(),
                    term: "EBITDA".into()
                },
                Suggestion {
                    heard: "super base".into(),
                    term: "Supabase".into()
                },
            ]
        );
    }

    #[test]
    fn learns_capitalisation_of_acronyms_only() {
        assert_eq!(
            suggest("check the ebitda", "check the EBITDA"),
            vec![Suggestion {
                heard: "ebitda".into(),
                term: "EBITDA".into()
            }]
        );
        // Plain sentence-case fixes are not vocabulary.
        assert!(suggest("hello there", "Hello there").is_empty());
    }

    #[test]
    fn ignores_rewrites_numbers_and_insertions() {
        assert!(suggest("we will meet at 3", "we will meet at 4").is_empty());
        assert!(suggest("send it", "please send it today").is_empty());
        assert!(suggest(
            "this sentence is completely different from that one",
            "nothing here matches anything at all ok"
        )
        .is_empty());
    }
}

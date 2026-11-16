//! Turns spoken enumerations ("First, ... Second, ...", "Number one, ... Number two, ...")
//! into a numbered list. Needs at least items 1 and 2 in order; a lone "first" is prose.

use std::sync::LazyLock;

use regex::Regex;

use super::tokens::capitalise_first;
use super::{Change, ChangeKind};

static ORDINAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:\b(?:and then|and|then)\s+)?\b(firstly|first|secondly|second|thirdly|third|fourthly|fourth|fifthly|fifth|sixth|seventh|eighth|ninth|tenth)\b(\s*[,:])?",
    )
    .unwrap()
});
static NUMBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:\b(?:and then|and|then)\s+)?\bnumber\s+(one|two|three|four|five|six|seven|eight|nine|ten|10|[1-9])\b(\s*[,:.])?",
    )
    .unwrap()
});
/// "First thing ...", "second point is ...", "third step: ..." (the noun makes it a list marker,
/// no comma needed), "point one ...", "point number two ...", and Hinglish "pehli baat ...",
/// "doosri baat ...", "teesri baat hai ...".
static THING: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:\b(?:and then|and|then|aur)\s+)?\b(first|second|third|fourth|fifth|sixth|seventh|eighth|ninth|tenth)\s+(?:thing|point|step|item|reason|question)\b(?:\s+(?:is|was))?(?:\s+that)?(\s*[,:.-])?",
    )
    .unwrap()
});
static POINT_NUMBER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:\b(?:and then|and|then|aur)\s+)?\b(?:point|step|item)\s+(?:number\s+)?(one|two|three|four|five|six|seven|eight|nine|ten|10|[1-9])\b(\s*[,:.-])?",
    )
    .unwrap()
});
static HINDI_ORDINAL: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)(?:\b(?:aur|and|then)\s+)?\b(pehli|pahli|pehla|pahla|doosri|dusri|doosra|dusra|teesri|tisri|teesra|tisra|chauthi|chautha|paanchvi|panchvi|paanchva)\s+(?:baat|point|cheez|cheej)\b(?:\s+(?:hai|ye\s+hai|yeh\s+hai))?(\s*[,:.-])?",
    )
    .unwrap()
});

fn hindi_ordinal_value(word: &str) -> Option<u32> {
    Some(match word {
        "pehli" | "pahli" | "pehla" | "pahla" => 1,
        "doosri" | "dusri" | "doosra" | "dusra" => 2,
        "teesri" | "tisri" | "teesra" | "tisra" => 3,
        "chauthi" | "chautha" => 4,
        "paanchvi" | "panchvi" | "paanchva" => 5,
        _ => return None,
    })
}

static INNER_SENTENCE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[.?!]\s").unwrap());

#[derive(Debug, Clone, Copy)]
struct Marker {
    start: usize,
    end: usize,
    n: u32,
}

fn ordinal_value(word: &str) -> Option<u32> {
    Some(match word.trim_end_matches("ly") {
        "first" => 1,
        "second" => 2,
        "third" => 3,
        "fourth" => 4,
        "fifth" => 5,
        "sixth" => 6,
        "seventh" => 7,
        "eighth" => 8,
        "ninth" => 9,
        "tenth" => 10,
        _ => return None,
    })
}

fn number_value(word: &str) -> Option<u32> {
    Some(match word {
        "one" => 1,
        "two" => 2,
        "three" => 3,
        "four" => 4,
        "five" => 5,
        "six" => 6,
        "seven" => 7,
        "eight" => 8,
        "nine" => 9,
        "ten" => 10,
        d => return d.parse().ok(),
    })
}

/// A list marker must open a clause: start of text, start of a line, or after punctuation.
/// Short openers people say before a list ("So, first thing ..."); they do not stop the list.
const OPENERS: &[&str] = &[
    "so", "okay", "ok", "well", "alright", "right", "now", "haan", "toh", "to", "acha", "accha",
];

fn is_opener(s: &str) -> bool {
    let w = s
        .trim()
        .trim_end_matches(['.', ',', '!', ' '])
        .to_lowercase();
    OPENERS.contains(&w.as_str())
}

fn at_clause_start(text: &str, pos: usize) -> bool {
    let before = text[..pos].trim_end_matches([' ', '\t']);
    if before.is_empty() || before.ends_with(['.', '?', '!', ',', ';', ':', '\n']) {
        return true;
    }
    // "So first thing ...": an opener alone at the start of the text or of a sentence.
    let last_clause = before
        .rsplit(['.', '?', '!', '\n'])
        .next()
        .unwrap_or(before);
    is_opener(last_clause)
}

fn find_markers(text: &str) -> Vec<Marker> {
    let mut markers = Vec::new();
    for caps in ORDINAL.captures_iter(text) {
        let whole = caps.get(0).unwrap();
        let word = caps[1].to_lowercase();
        let has_separator = caps.get(2).is_some();
        // "First, ..." needs its comma; "Firstly ..." does not.
        if !(has_separator || word.ends_with("ly")) || !at_clause_start(text, whole.start()) {
            continue;
        }
        if let Some(n) = ordinal_value(&word) {
            markers.push(Marker {
                start: whole.start(),
                end: whole.end(),
                n,
            });
        }
    }
    for caps in NUMBER.captures_iter(text) {
        let whole = caps.get(0).unwrap();
        if !at_clause_start(text, whole.start()) {
            continue;
        }
        if let Some(n) = number_value(&caps[1].to_lowercase()) {
            markers.push(Marker {
                start: whole.start(),
                end: whole.end(),
                n,
            });
        }
    }
    let mut push = |start: usize, end: usize, n: Option<u32>| {
        if let Some(n) = n {
            if at_clause_start(text, start) && !markers.iter().any(|m: &Marker| m.start == start) {
                markers.push(Marker { start, end, n });
            }
        }
    };
    for caps in THING.captures_iter(text) {
        let whole = caps.get(0).unwrap();
        push(
            whole.start(),
            whole.end(),
            ordinal_value(&caps[1].to_lowercase()),
        );
    }
    for caps in POINT_NUMBER.captures_iter(text) {
        let whole = caps.get(0).unwrap();
        push(
            whole.start(),
            whole.end(),
            number_value(&caps[1].to_lowercase()),
        );
    }
    for caps in HINDI_ORDINAL.captures_iter(text) {
        let whole = caps.get(0).unwrap();
        push(
            whole.start(),
            whole.end(),
            hindi_ordinal_value(&caps[1].to_lowercase()),
        );
    }
    markers.sort_by_key(|m| m.start);
    // Overlapping matches ("first thing" also starts like "First,"): keep the longer one.
    let mut kept: Vec<Marker> = Vec::new();
    for m in markers {
        match kept.last_mut() {
            Some(prev) if m.start < prev.end => {
                if m.end > prev.end {
                    *prev = m;
                }
            }
            _ => kept.push(m),
        }
    }
    kept
}

fn clean_item(raw: &str) -> String {
    let mut t = raw
        .trim()
        .trim_start_matches([',', ':', ';', '.', '-', ' '])
        .trim_end_matches([',', ';', ' '])
        .to_string();
    if t.to_lowercase().ends_with(" and") {
        t.truncate(t.len() - 4);
        t = t.trim_end_matches([',', ';', ' ']).to_string();
    }
    if t.ends_with('.') && !INNER_SENTENCE.is_match(&t[..t.len() - 1]) {
        t.pop();
    }
    capitalise_first(&t)
}

pub fn format_lists(text: &str, changes: &mut Vec<Change>) -> String {
    let markers = find_markers(text);

    // First run of 1, 2, 3 ... in order.
    let mut run: Vec<Marker> = Vec::new();
    for (i, m) in markers.iter().enumerate() {
        if m.n != 1 {
            continue;
        }
        let mut candidate = vec![*m];
        let mut expected = 2;
        for next in &markers[i + 1..] {
            if next.n == expected {
                candidate.push(*next);
                expected += 1;
            } else if next.n == 1 {
                break;
            }
        }
        if candidate.len() >= 2 {
            run = candidate;
            break;
        }
    }
    if run.len() < 2 {
        return text.to_string();
    }

    let list_start = run[0].start;
    let last_item_start = run[run.len() - 1].end;
    // The list ends at a paragraph break, otherwise at the end of the text.
    let mut list_end = text[last_item_start..]
        .find("\n\n")
        .map_or(text.len(), |p| last_item_start + p);

    // "Third, the hiring plan. Let us meet on Monday." - when every earlier item is a single
    // sentence, the last item is too, and what follows it is ordinary text again.
    let mut prose_follows = false;
    let earlier_are_single = run.windows(2).all(|w| {
        let item = text[w[0].end..w[1].start]
            .trim()
            .trim_end_matches(['.', '?', '!', ',', ';', ' ']);
        !INNER_SENTENCE.is_match(item)
    });
    if earlier_are_single {
        let last = &text[last_item_start..list_end];
        let sentence_end = INNER_SENTENCE.find_iter(last).find(|m| {
            !last[m.end()..]
                .trim_start()
                .starts_with(|c: char| c.is_lowercase())
        });
        if let Some(m) = sentence_end {
            let cut = last_item_start + m.start() + 1;
            if !text[cut..list_end].trim().is_empty() {
                list_end = cut;
                prose_follows = true;
            }
        }
    }

    let mut items = Vec::with_capacity(run.len());
    for (i, m) in run.iter().enumerate() {
        let end = if i + 1 < run.len() {
            run[i + 1].start
        } else {
            list_end
        };
        let item = clean_item(&text[m.end..end]);
        if item.is_empty() {
            return text.to_string();
        }
        items.push(format!("{}. {}", i + 1, item));
    }

    let mut lead = text[..list_start].trim_end();
    if is_opener(lead) {
        lead = ""; // "So, first thing ..." -> just the list
    }
    let formatted = items.join("\n");
    let mut out = String::new();
    if !lead.is_empty() {
        out.push_str(lead);
        out.push('\n');
    }
    out.push_str(&formatted);
    if prose_follows {
        out.push_str("\n\n");
        out.push_str(text[list_end..].trim_start());
    } else {
        out.push_str(&text[list_end..]);
    }

    changes.push(Change {
        kind: ChangeKind::List,
        before: text[list_start..list_end].to_string(),
        after: formatted,
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(s: &str) -> String {
        format_lists(s, &mut Vec::new())
    }

    #[test]
    fn ordinal_list_with_lead_in() {
        assert_eq!(
            run("We need three things. First, the budget. Second, the timeline. Third, the team."),
            "We need three things.\n1. The budget\n2. The timeline\n3. The team"
        );
    }

    #[test]
    fn number_list() {
        assert_eq!(
            run("Number one, update the forecast. Number two, send it to finance."),
            "1. Update the forecast\n2. Send it to finance"
        );
    }

    #[test]
    fn prose_is_not_a_list() {
        for s in [
            "The first quarter was strong and the second quarter was weak.",
            "First we discussed sales. Then we talked about hiring.",
            "This is our number one priority and number two is hiring.",
            "I have two points to make",
        ] {
            assert_eq!(run(s), s);
        }
    }

    #[test]
    fn first_thing_second_thing_becomes_points() {
        assert_eq!(
            run("First thing my name is Kunal Kanjwani, second thing my roll number is 52."),
            "1. My name is Kunal Kanjwani\n2. My roll number is 52"
        );
        assert_eq!(
            run("The first point is the budget. The second point is the team."),
            "The first point is the budget. The second point is the team."
        );
        assert_eq!(
            run("Point one, call the bank. Point number two, pay the rent."),
            "1. Call the bank\n2. Pay the rent"
        );
        assert_eq!(
            run("Pehli baat, kal meeting hai. Doosri baat, report bhejni hai."),
            "1. Kal meeting hai\n2. Report bhejni hai"
        );
        assert_eq!(
            run("First thing is that we start early, second thing is that we finish on time."),
            "1. We start early\n2. We finish on time"
        );
    }

    #[test]
    fn opener_before_the_list_is_dropped() {
        assert_eq!(
            run("So first thing my name is Kunal Kanjwani, second thing my roll number is 52"),
            "1. My name is Kunal Kanjwani\n2. My roll number is 52"
        );
        assert_eq!(
            run("Okay, first point call the bank, second point pay the rent."),
            "1. Call the bank\n2. Pay the rent"
        );
    }

    #[test]
    fn things_in_ordinary_sentences_stay_prose() {
        for s in [
            "The first thing I did was call my mother.",
            "First thing in the morning I go for a walk.",
            "That was the second point he made yesterday.",
        ] {
            assert_eq!(run(s), s);
        }
    }

    #[test]
    fn lone_first_is_not_a_list() {
        let s = "First, thank you all for coming.";
        assert_eq!(run(s), s);
    }
}

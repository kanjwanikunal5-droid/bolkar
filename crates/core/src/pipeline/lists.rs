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
fn at_clause_start(text: &str, pos: usize) -> bool {
    let before = text[..pos].trim_end_matches([' ', '\t']);
    before.is_empty() || before.ends_with(['.', '?', '!', ',', ';', ':', '\n'])
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
    markers.sort_by_key(|m| m.start);
    markers
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

    let lead = text[..list_start].trim_end();
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
    fn lone_first_is_not_a_list() {
        let s = "First, thank you all for coming.";
        assert_eq!(run(s), s);
    }
}

//! Smart layout: paragraphs, bullet points, numbered steps and label lines, the way the owner
//! asked for (17-19 Nov 2026: "natural paragraphs when it's a story or explanation, bullet
//! points when you list items, both together for structured notes").
//!
//! An AI proposes the layout; this module decides whether to accept it. The rule is strict
//! and checked word by word: the answer must contain the spoken words in the same order, with
//! nothing added. It may only add line breaks, list markers and label colons, change capitals
//! and punctuation, and leave out a list connector ("and", "also", "aur", or an order word such as "first"
//! that a number replaces) only where a new line starts.
//! Anything else (a new word, a heading nobody said, a reordered or dropped content word) and
//! the spoken text is kept as it was.

use std::sync::LazyLock;

use regex::Regex;

static WORD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[\p{L}\p{N}]+(?:['\u{2019}][\p{L}]+)*").unwrap());
static MARKER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\s*(?:[-*\u{2022}\u{25CF}]|\d{1,2}[.)])\s+").unwrap());

/// The only words a layout may leave out, and only right where a new line starts (they join
/// list items: "... by Friday and | book the venue"). Words that carry meaning - subjects,
/// pronouns, "will", "is", "not", "next", numbers, names, Hinglish "hai", "mein", "toh" - are
/// never on this list (audit of 0.4.3, 19 Nov 2026).
const CONNECTORS: &[&str] = &["and", "also", "then", "finally", "lastly", "aur", "phir"];

/// Spoken order words ("first", "doosri") that a numbered or bulleted line replaces. Like the
/// connectors they may only go where a line starts, there can be no more of them than list
/// lines, and they do not count towards the drop limit.
const ORDINALS: &[&str] = &[
    "first", "firstly", "second", "secondly", "third", "thirdly", "fourth", "fifth", "pehla",
    "pehli", "doosra", "doosri", "teesra", "teesri",
];

/// Most of the words a layout may drop, as a share of the spoken words.
const MAX_DROPPED_SHARE: usize = 12;

fn words(s: &str) -> Vec<String> {
    WORD.find_iter(s)
        .map(|m| m.as_str().to_lowercase().replace('\u{2019}', "'"))
        .collect()
}

/// True for text worth laying out: at least three sentences or 40 words, not already laid out.
pub fn worth_laying_out(text: &str) -> bool {
    if text.contains('\n') {
        return false;
    }
    let sentences = crate::pipeline::sentences(text).len();
    let n = words(text).len();
    n >= 12 && (sentences >= 3 || n >= 40)
}

/// The system prompt (from `spec/prompts/layout.md`).
pub fn layout_prompt() -> String {
    const SPEC: &str = include_str!("../../../spec/prompts/layout.md");
    let after = SPEC.split_once("## System prompt").map_or("", |(_, r)| r);
    let body = after.split_once("```\n").map_or("", |(_, r)| r);
    body.split_once("\n```")
        .map_or("", |(b, _)| b)
        .replace('\r', "")
}

/// Reads the model's answer ({"text": "..."}, fences and chatter tolerated).
pub fn parse_layout(content: &str) -> Result<String, String> {
    let (s, e) = (content.find('{'), content.rfind('}'));
    let json = match (s, e) {
        (Some(s), Some(e)) if e > s => &content[s..=e],
        _ => return Err("no JSON in the answer".into()),
    };
    let v: serde_json::Value = serde_json::from_str(json).map_err(|e| format!("bad JSON: {e}"))?;
    v.get("text")
        .and_then(|t| t.as_str())
        .map(|t| t.trim().to_string())
        .ok_or_else(|| "no text in the answer".into())
}

fn is_item(line: &str) -> bool {
    line.starts_with("\u{2022} ")
        || line
            .split_once(". ")
            .is_some_and(|(n, _)| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()))
}

/// Accepts the laid-out text only if it is the spoken text with layout added (see the module
/// comment). Returns the layout, or why it was refused.
pub fn accept_layout(spoken: &str, laid_out: &str) -> Result<String, String> {
    let out = laid_out.trim().replace("\r\n", "\n");
    if out.is_empty() {
        return Err("empty answer".into());
    }
    if !out.contains('\n') {
        return Err("no layout".into());
    }
    let src = words(spoken);
    let without_markers = MARKER.replace_all(&out, "");
    // The words of the answer, and which of them start a line.
    let mut got: Vec<String> = Vec::new();
    let mut line_starts = std::collections::HashSet::new();
    for line in without_markers.lines() {
        let ws = words(line);
        if !ws.is_empty() {
            line_starts.insert(got.len());
            got.extend(ws);
        }
    }
    // `got` must be `src` with only connector words left out, each where a line starts, in the
    // same order.
    let mut i = 0;
    let mut dropped = 0;
    let mut ordinals = 0;
    for (k, w) in got.iter().enumerate() {
        if !src[i.min(src.len())..].contains(w) {
            return Err(format!("\"{w}\" was not spoken (or not in this order)"));
        }
        loop {
            let Some(s) = src.get(i) else {
                return Err(format!("\"{w}\" was not spoken (or not in this order)"));
            };
            i += 1;
            if s == w {
                break;
            }
            let droppable = ORDINALS.contains(&s.as_str()) || CONNECTORS.contains(&s.as_str());
            if !droppable || !line_starts.contains(&k) {
                return Err(format!("the spoken word \"{s}\" was dropped or moved"));
            }
            if ORDINALS.contains(&s.as_str()) {
                ordinals += 1;
            } else {
                dropped += 1;
            }
        }
    }
    if let Some(s) = src.get(i) {
        return Err(format!("the spoken word \"{s}\" was dropped"));
    }
    if dropped > 3 && dropped * 100 > src.len() * MAX_DROPPED_SHARE {
        return Err(format!("{dropped} of {} words dropped", src.len()));
    }
    let items = out.lines().filter(|l| MARKER.is_match(l)).count();
    if ordinals > items {
        return Err(format!(
            "{ordinals} order words dropped for {items} list lines"
        ));
    }
    // Tidy the markers: one bullet style, no blank list lines, at most one blank line.
    let mut lines: Vec<String> = Vec::new();
    for line in out.lines() {
        let t = line.trim_end();
        let t = if let Some(rest) = t
            .trim_start()
            .strip_prefix("- ")
            .or_else(|| t.trim_start().strip_prefix("* "))
            .or_else(|| t.trim_start().strip_prefix("\u{25CF} "))
        {
            format!("\u{2022} {}", rest.trim_start())
        } else {
            t.trim_start().to_string()
        };
        if t.is_empty() && lines.last().is_some_and(|l| l.is_empty()) {
            continue;
        }
        lines.push(t);
    }
    // A blank line between a paragraph and a list (either way round), except directly under
    // a label ("Next steps:") that introduces its items.
    let mut spaced: Vec<String> = Vec::new();
    for line in lines {
        if let Some(prev) = spaced.last() {
            let switch = !prev.is_empty()
                && !line.is_empty()
                && is_item(prev) != is_item(&line)
                && !(prev.ends_with(':') && is_item(&line));
            if switch {
                spaced.push(String::new());
            }
        }
        spaced.push(line);
    }
    Ok(spaced.join("\n").trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixed_paragraph_and_points_like_the_owners_picture_is_accepted() {
        let spoken = "Today's discussion focused on the Q4 product roadmap and upcoming release plans. The team agreed to prioritise user experience improvements while keeping the launch timeline on track. The key decisions are improve onboarding flow, fix critical bugs in version 0.4.2 and enhance mobile performance. Next steps, the dev team to implement fixes by Friday and the product team to prepare release notes.";
        let laid = "Today's discussion focused on the Q4 product roadmap and upcoming release plans. The team agreed to prioritise user experience improvements while keeping the launch timeline on track.\n\nThe key decisions are:\n- Improve onboarding flow\n- Fix critical bugs in version 0.4.2\n- Enhance mobile performance\n\nNext steps:\n1. The dev team to implement fixes by Friday\n2. The product team to prepare release notes";
        let ok = accept_layout(spoken, laid).unwrap();
        assert!(ok.contains("\u{2022} Improve onboarding flow"), "{ok}");
        assert!(ok.contains("1. The dev team to implement fixes by Friday"));
        // Leaving out "The ... are" (0.4.3 allowed it) is refused now.
        let short = laid.replace("The key decisions are:", "Key decisions:");
        assert!(accept_layout(spoken, &short).is_err());
    }

    #[test]
    fn lists_are_set_apart_from_paragraphs_but_stay_under_their_label() {
        let spoken = "The project has three parts. The first part is data collection. The second part is cleaning. After that we will present it on Monday.";
        let laid = "The project has three parts.\n1. The first part is data collection.\n2. The second part is cleaning.\nAfter that we will present it on Monday.";
        assert_eq!(
            accept_layout(spoken, laid).unwrap(),
            "The project has three parts.\n\n1. The first part is data collection.\n2. The second part is cleaning.\n\nAfter that we will present it on Monday."
        );
        let spoken = "Next steps are send the deck and book the venue.";
        let laid = "Next steps are:\n1. send the deck\n2. book the venue";
        assert_eq!(accept_layout(spoken, laid).unwrap(), laid);
    }

    #[test]
    fn hinglish_meaning_words_are_never_left_out() {
        let spoken = "Aaj humne do cheezein decide ki. Next steps mein mujhe deck bhejni hai aur venue book karna hai.";
        let laid = "Aaj humne do cheezein decide ki.\n\nNext steps mein:\n\u{2022} mujhe deck bhejni hai\n\u{2022} venue book karna hai";
        assert!(accept_layout(spoken, laid).is_ok());
        // "mein" left out of the label: refused (0.4.3 allowed it).
        let no_mein = laid.replace("Next steps mein:", "Next steps:");
        assert!(accept_layout(spoken, &no_mein).is_err());
        // A Hinglish content word is never dropped.
        let bad = "Aaj humne do cheezein decide ki.\n\nNext steps mein:\n\u{2022} mujhe deck bhejni hai\n\u{2022} venue karna hai";
        assert!(accept_layout(spoken, bad).is_err());
        // Order words replaced by numbering do not count as dropped words.
        let spoken = "Aaj ki meeting mein humne teen cheezein decide ki. Pehli, launch agle hafte hoga. Doosri, budget wahi rahega. Teesri, Rahul marketing dekhega. Next steps mein mujhe deck bhejni hai aur venue book karna hai.";
        let laid = "Aaj ki meeting mein humne teen cheezein decide ki.\n1. Launch agle hafte hoga.\n2. Budget wahi rahega.\n3. Rahul marketing dekhega.\n\nNext steps mein:\n\u{2022} Mujhe deck bhejni hai\n\u{2022} Venue book karna hai.";
        assert!(
            accept_layout(spoken, laid).is_ok(),
            "{:?}",
            accept_layout(spoken, laid)
        );
    }

    /// The audit's adversarial cases: leaving out any word that carries meaning is refused,
    /// even one, wherever it is; a joining word may go only where a new line starts.
    #[test]
    fn meaning_words_are_never_left_out() {
        for w in [
            "we", "our", "my", "will", "is", "was", "have", "has", "not", "only", "if", "unless",
            "that", "which", "the", "next", "hai", "hain", "mein", "toh", "20th", "Priya",
            "Monday", "50,000",
        ] {
            let spoken = format!("Here is the plan. Send the deck {w} today and book the venue.");
            let without =
                "Here is the plan.\n\u{2022} Send the deck today\n\u{2022} book the venue";
            assert!(
                accept_layout(&spoken, without).is_err(),
                "dropping {w:?} was accepted"
            );
            let with = format!(
                "Here is the plan.\n\u{2022} Send the deck {w} today\n\u{2022} book the venue"
            );
            assert!(
                accept_layout(&spoken, &with).is_ok(),
                "keeping {w:?} was refused"
            );
        }
        // A joining word in the middle of a line: refused. At a line start: allowed.
        let spoken = "We will send the deck and book the venue. Then call Priya.";
        assert!(accept_layout(
            spoken,
            "\u{2022} We will send the deck book the venue\n\u{2022} call Priya."
        )
        .is_err());
        assert!(accept_layout(
            spoken,
            "\u{2022} We will send the deck\n\u{2022} book the venue.\n\u{2022} call Priya."
        )
        .is_ok());
        // "We will not approve unless..." keeps every word.
        let spoken = "Budget review. We will not approve it unless the return stays above three times. Rahul checks it.";
        let bad = "Budget review.\n\u{2022} We will approve it unless the return stays above three times.\n\u{2022} Rahul checks it.";
        assert!(accept_layout(spoken, bad).unwrap_err().contains("not"));
    }

    /// "first, second, third" may become 1. 2. 3. without losing the items.
    #[test]
    fn order_words_become_numbers() {
        let spoken = "We have three priorities. First, finish the forecast. Second, review hiring costs. Third, send the report by Friday.";
        let laid = "We have three priorities:\n1. Finish the forecast.\n2. Review hiring costs.\n3. Send the report by Friday.";
        assert_eq!(accept_layout(spoken, laid).unwrap(), laid);
        // An order word dropped in the middle of a line is refused.
        let bad = "We have three priorities:\n1. Finish the forecast. Review hiring costs.\n2. Send the report by Friday.";
        assert!(accept_layout(spoken, bad).is_err());
    }

    #[test]
    fn spoken_numbers_are_never_dropped() {
        let spoken = "Buy two kilos of rice, three packets of milk and bread.";
        let bad = "\u{2022} Buy kilos of rice\n\u{2022} three packets of milk\n\u{2022} bread.";
        assert!(accept_layout(spoken, bad).unwrap_err().contains("two"));
        let good =
            "\u{2022} Buy two kilos of rice\n\u{2022} three packets of milk\n\u{2022} bread.";
        assert!(accept_layout(spoken, good).is_ok());
    }

    #[test]
    fn a_heading_nobody_said_is_refused() {
        let spoken = "Fresh vegetables, milk and bread, organic fruits, cooking oil, tea and coffee and paper towels.";
        let invented = "Shopping List\n\u{2022} Fresh vegetables\n\u{2022} Milk and bread\n\u{2022} Organic fruits\n\u{2022} Cooking oil\n\u{2022} Tea and coffee\n\u{2022} Paper towels";
        assert!(accept_layout(spoken, invented)
            .unwrap_err()
            .contains("shopping"));
        let honest = "\u{2022} Fresh vegetables\n\u{2022} Milk and bread\n\u{2022} Organic fruits\n\u{2022} Cooking oil\n\u{2022} Tea and coffee\n\u{2022} Paper towels";
        assert!(accept_layout(spoken, honest).is_ok());
        // Said aloud, the heading is the speaker's own words and may stay.
        let spoken2 = format!("Shopping list. {spoken}");
        assert!(accept_layout(&spoken2, invented).is_ok());
    }

    #[test]
    fn changed_reordered_or_dropped_content_words_are_refused() {
        let spoken = "We need to fix the login bug. Then we must update the pricing page. Finally call the client on Monday.";
        // Changed word.
        assert!(accept_layout(spoken, "\u{2022} Fix the login issue\n\u{2022} Update the pricing page\n\u{2022} Call the client on Monday").is_err());
        // Reordered.
        assert!(accept_layout(spoken, "\u{2022} Call the client on Monday\n\u{2022} We need to fix the login bug\n\u{2022} Then we must update the pricing page").is_err());
        // Dropped content word ("Monday").
        assert!(accept_layout(spoken, "\u{2022} We need to fix the login bug\n\u{2022} Then we must update the pricing page\n\u{2022} Finally call the client").is_err());
        // Layout only: accepted.
        assert!(accept_layout(spoken, "We need to:\n1. Fix the login bug\n2. Update the pricing page\n3. Call the client on Monday").is_err(), "\"must\" is not a connector");
        assert!(accept_layout(spoken, "\u{2022} We need to fix the login bug.\n\u{2022} Then we must update the pricing page.\n\u{2022} Finally call the client on Monday.").is_ok());
    }

    #[test]
    fn numbers_cannot_change_and_no_layout_is_no_change() {
        let spoken = "Revenue grew 12 percent. Costs grew 8 percent. Margin fell 2 points.";
        assert!(accept_layout(spoken, "\u{2022} Revenue grew 15 percent\n\u{2022} Costs grew 8 percent\n\u{2022} Margin fell 2 points").is_err());
        assert_eq!(accept_layout(spoken, spoken).unwrap_err(), "no layout");
    }

    #[test]
    fn hinglish_layout_keeps_roman_words() {
        let spoken =
            "Aaj ke kaam hain report bhejna, client ko call karna aur invoice check karna.";
        let laid = "Aaj ke kaam hain:\n\u{2022} Report bhejna\n\u{2022} Client ko call karna\n\u{2022} Invoice check karna";
        assert!(accept_layout(spoken, laid).is_ok());
    }

    #[test]
    fn short_text_is_left_alone() {
        assert!(!worth_laying_out("Send the report by Friday."));
        assert!(!worth_laying_out("Line one\nLine two"));
        assert!(worth_laying_out("We met the client today. They liked the demo a lot. Next we send the pricing and the contract draft."));
    }

    #[test]
    fn answer_parsing() {
        assert_eq!(
            parse_layout("```json\n{\"text\": \"a\\nb\"}\n```").unwrap(),
            "a\nb"
        );
        assert!(parse_layout("no json").is_err());
    }
}

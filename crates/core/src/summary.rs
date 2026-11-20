//! Meeting notes: the summary format, its checks against the transcript, long-meeting
//! windows and the Markdown export.
//!
//! The AI writes the summary as JSON (`spec/schemas/meeting_summary.schema.json`). Before it
//! is shown, every item is checked against the transcript with plain, deterministic rules:
//! - cited segment ids must exist, otherwise the item is removed (listed in `dropped`);
//! - an owner or deadline must appear word for word in a cited segment, otherwise it becomes
//!   null ("Not specified"); a deadline said with "maybe" is marked tentative;
//! - every number in an item must appear in the cited words; a value in "numbers" that does
//!   not is removed;
//! - a supporting quote, when given, must be found in the cited words;
//! - a "confirmed" decision needs words of agreement in the cited segments and no hedging,
//!   otherwise it is shown as "proposed".
//!
//! What these checks can and cannot prove: they show that an item is linked to real
//! transcript lines and that its quotes, numbers, names and dates occur there. They cannot
//! prove that the meaning of the item is right. Items are therefore labelled "quote found",
//! "source linked" or "check this" (see [`Check`]), never "verified".

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

/// Track labels. They name the sound source, not a person: the microphone can pick up other
/// people in the room, and computer sound is everything the computer plays.
pub const MIC: &str = "Microphone";
pub const SYSTEM: &str = "System audio";

fn is_mic(label: &str) -> bool {
    label == MIC || label == "Me"
}
fn is_system(label: &str) -> bool {
    label == SYSTEM || label == "Others"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub id: String,
    pub start_ms: u64,
    pub end_ms: u64,
    /// The source track ("Microphone", "System audio"; "Me"/"Others" in older notes).
    pub speaker: String,
    pub text: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Overview {
    pub title: String,
    pub date: Option<String>,
    pub duration_min: Option<f64>,
    #[serde(default)]
    pub participants: Vec<String>,
    pub purpose: Option<String>,
}

/// How well an item is backed by the transcript (set by [`validate`], never by the AI).
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Check {
    /// The cited lines exist and share words with the item; its meaning was not proven.
    #[default]
    Linked,
    /// The supporting quote was found word for word in a cited line.
    Quoted,
    /// Something did not match (see `note`); the user should check this item.
    NeedsReview,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Point {
    pub text: String,
    pub evidence: Vec<String>,
    /// Exact words from a cited segment that support the item (optional).
    #[serde(default)]
    pub quote: Option<String>,
    #[serde(default)]
    pub check: Check,
    /// Why the item needs checking.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl Point {
    pub fn new(text: &str, evidence: &[&str]) -> Self {
        Self {
            text: text.into(),
            evidence: evidence.iter().map(|s| s.to_string()).collect(),
            quote: None,
            check: Check::Linked,
            note: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Topic {
    pub title: String,
    /// A connected paragraph about the topic (preferred for explanations).
    #[serde(default)]
    pub narrative: Option<Point>,
    /// Only distinct takeaways; may be empty when the narrative says it all.
    #[serde(default, alias = "key_points")]
    pub points: Vec<Point>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DecisionStatus {
    Confirmed,
    Proposed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Decision {
    pub text: String,
    pub status: DecisionStatus,
    pub evidence: Vec<String>,
    #[serde(default)]
    pub quote: Option<String>,
    #[serde(default)]
    pub check: Check,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActionItem {
    pub action: String,
    pub owner: Option<String>,
    pub deadline: Option<String>,
    pub evidence: Vec<String>,
    #[serde(default)]
    pub quote: Option<String>,
    #[serde(default)]
    pub check: Check,
    /// The deadline was said with hedging ("maybe next Friday").
    #[serde(default)]
    pub deadline_tentative: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NumberItem {
    pub value: String,
    pub context: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Summary {
    pub overview: Overview,
    #[serde(default)]
    pub executive_summary: Vec<Point>,
    #[serde(default)]
    pub topics: Vec<Topic>,
    #[serde(default)]
    pub decisions: Vec<Decision>,
    #[serde(default)]
    pub action_items: Vec<ActionItem>,
    #[serde(default)]
    pub risks: Vec<Point>,
    #[serde(default)]
    pub open_questions: Vec<Point>,
    #[serde(default)]
    pub numbers: Vec<NumberItem>,
    #[serde(default)]
    pub next_steps: Vec<Point>,
    pub takeaway: Option<Point>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Validated {
    pub summary: Summary,
    /// Items removed because none of their cited segment ids exist (or a number item whose
    /// value is not in the cited words), and fields blanked, with the reason.
    pub dropped: Vec<String>,
    /// Items kept but marked "check this", with the reason.
    pub flags: Vec<String>,
}

fn norm(s: &str) -> String {
    s.to_lowercase()
        .replace(['\u{2019}', '\u{2018}'], "'")
        .replace(['\u{201C}', '\u{201D}'], "\"")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Letters and digits only, for comparing quotes regardless of punctuation.
fn bare(s: &str) -> String {
    norm(s)
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Digit runs in a text, thousands separators removed ("₹50,000 and 12%" -> ["50000", "12"]).
fn digit_runs(text: &str) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut runs = Vec::new();
    let mut cur = String::new();
    for (i, c) in chars.iter().enumerate() {
        if c.is_ascii_digit() {
            cur.push(*c);
        } else if *c == ',' && !cur.is_empty() && chars.get(i + 1).is_some_and(char::is_ascii_digit)
        {
            continue;
        } else if !cur.is_empty() {
            runs.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        runs.push(cur);
    }
    runs
}

const STOPWORDS: &[&str] = &[
    "the",
    "a",
    "an",
    "and",
    "or",
    "of",
    "to",
    "in",
    "on",
    "for",
    "at",
    "by",
    "with",
    "is",
    "are",
    "was",
    "were",
    "be",
    "been",
    "it",
    "this",
    "that",
    "we",
    "they",
    "he",
    "she",
    "i",
    "you",
    "our",
    "their",
    "will",
    "would",
    "should",
    "can",
    "could",
    "has",
    "have",
    "had",
    "do",
    "does",
    "did",
    "not",
    "no",
    "as",
    "from",
    "about",
    "into",
    "also",
    "more",
    "some",
    "all",
    "team",
    "meeting",
    "discussed",
    "said",
    "hai",
    "hain",
    "tha",
    "the",
    "ki",
    "ka",
    "ke",
    "ko",
    "se",
    "mein",
    "aur",
    "bhi",
    "ne",
    "par",
    "ye",
    "yeh",
    "woh",
];

fn content_words(text: &str) -> Vec<String> {
    bare(text)
        .split(' ')
        .filter(|w| w.len() > 2 && !STOPWORDS.contains(w) && !w.chars().all(|c| c.is_ascii_digit()))
        .map(str::to_string)
        .collect()
}

/// Words that show the speaker was unsure or only suggesting.
const HEDGES: &[&str] = &[
    "maybe",
    "might",
    "perhaps",
    "probably",
    "possibly",
    "not sure",
    "should we",
    "could we",
    "what if",
    "i think",
    "we could",
    "how about",
    "suggest",
    "propose",
    "tentative",
    "shayad",
    "ho sakta",
    "sochte hain",
    "dekhte hain",
];
/// Ways an item itself can say that something is uncertain (so it does not hide a hedge).
const UNCERTAIN_CLAIM: &[&str] = &[
    "unclear",
    "uncertain",
    "undecided",
    "unknown",
    "unsure",
    "no definitive",
    "not yet",
    "not decided",
    "to be decided",
    "tbd",
    "pending",
    "questioned",
    "open question",
    "not confirmed",
    "tentative",
    "tentatively",
    "may",
    "could",
    "likely",
    "possible",
    "seems",
];

fn claim_is_tentative(text: &str) -> bool {
    text.contains('?') || contains_any(text, HEDGES) || contains_any(text, UNCERTAIN_CLAIM)
}

/// Words that show the group agreed.
const AGREEMENT: &[&str] = &[
    "agreed",
    "agree",
    "decided",
    "decide",
    "final",
    "finalised",
    "finalized",
    "confirmed",
    "confirm",
    "let's do",
    "lets do",
    "let us do",
    "we will",
    "we'll",
    "approved",
    "done deal",
    "go ahead",
    "theek hai",
    "pakka",
    "tay hua",
    "decide kiya",
];
/// Words that show disagreement.
const DISAGREEMENT: &[&str] = &[
    "disagree",
    "don't agree",
    "do not agree",
    "not agreed",
    "objection",
];

fn contains_any(haystack: &str, words: &[&str]) -> bool {
    let h = format!(" {} ", bare(haystack));
    words.iter().any(|w| h.contains(&format!(" {} ", bare(w))))
}

struct Index<'a> {
    by_id: HashMap<&'a str, &'a Segment>,
    /// Segment ids in transcript order.
    order: Vec<&'a str>,
}

impl<'a> Index<'a> {
    fn new(segments: &'a [Segment]) -> Self {
        Self {
            by_id: segments.iter().map(|s| (s.id.as_str(), s)).collect(),
            order: segments.iter().map(|s| s.id.as_str()).collect(),
        }
    }

    /// Keeps only ids that exist. Returns None when nothing valid remains.
    fn evidence(&self, ids: &[String]) -> Option<Vec<String>> {
        let ok: Vec<String> = ids
            .iter()
            .filter(|id| self.by_id.contains_key(id.as_str()))
            .cloned()
            .collect();
        (!ok.is_empty()).then_some(ok)
    }

    /// `ids` plus the `k` transcript lines just before the earliest of them (a condition or an
    /// amount is often said in the line before "Agreed.").
    fn with_previous(&self, ids: &[String], k: usize) -> Vec<String> {
        let first = ids
            .iter()
            .filter_map(|id| self.order.iter().position(|o| o == id))
            .min();
        let mut out: Vec<String> = match first {
            Some(p) => self.order[p.saturating_sub(k)..p]
                .iter()
                .map(|s| s.to_string())
                .collect(),
            None => Vec::new(),
        };
        out.extend(ids.iter().cloned());
        out
    }

    fn cited_text(&self, ids: &[String]) -> String {
        ids.iter()
            .filter_map(|id| self.by_id.get(id.as_str()))
            .map(|s| norm(&s.text))
            .collect::<Vec<_>>()
            .join(" \n ")
    }

    fn cited_mentions(&self, ids: &[String], phrase: &str) -> bool {
        let p = bare(phrase);
        !p.is_empty() && format!(" {} ", bare(&self.cited_text(ids))).contains(&format!(" {p} "))
    }

    fn cited_has_number(&self, ids: &[String], value: &str) -> bool {
        let want = value
            .chars()
            .filter(char::is_ascii_digit)
            .collect::<String>();
        if want.is_empty() {
            // Spelled-out values ("three interviews") must appear as written.
            return self.cited_mentions(ids, value);
        }
        let text = self.cited_text(ids);
        // "3" is supported by "Three interviews are pending."
        if crate::llm::spoken_numbers(&text).contains(&want) {
            return true;
        }
        digit_runs(&text).contains(&want)
    }

    /// The common checks for one item: its numbers, its quote and its words.
    fn assess(
        &self,
        text: &str,
        evidence: &[String],
        quote: Option<&str>,
    ) -> (Check, Option<String>) {
        self.assess_as(text, evidence, quote, false)
    }

    /// As [`Self::assess`]; `question` is true for open questions, which are uncertain by
    /// nature (the hedge check does not apply to them).
    fn assess_as(
        &self,
        text: &str,
        evidence: &[String],
        quote: Option<&str>,
        question: bool,
    ) -> (Check, Option<String>) {
        let cited = self.cited_text(evidence);
        for run in digit_runs(text) {
            if !self.cited_has_number(evidence, &run) {
                return (
                    Check::NeedsReview,
                    Some(format!(
                        "the number {run} is not in the cited transcript lines"
                    )),
                );
            }
        }
        let words = content_words(text);
        let cited_bare = format!(" {} ", bare(&cited));
        let shared = words
            .iter()
            .filter(|w| {
                let stem: String = w.chars().take(5).collect();
                cited_bare.contains(&format!(" {stem}"))
            })
            .count();
        if words.len() >= 2 && shared == 0 {
            return (
                Check::NeedsReview,
                Some("it shares no words with the cited transcript lines".into()),
            );
        }
        if !question && contains_any(&cited, HEDGES) && !claim_is_tentative(text) {
            return (
                Check::NeedsReview,
                Some("the speaker sounded unsure, but the item reads as certain".into()),
            );
        }
        match quote.map(str::trim).filter(|q| !q.is_empty()) {
            Some(q) => {
                // A quote may join exact pieces with "..."; every piece must be found.
                let parts: Vec<String> = q
                    .split('\u{2026}')
                    .flat_map(|p| p.split("..."))
                    .map(bare)
                    .filter(|p| !p.is_empty())
                    .collect();
                if !parts.is_empty() && parts.iter().all(|p| cited_bare.contains(&format!(" {p} ")))
                {
                    (Check::Quoted, None)
                } else {
                    (
                        Check::NeedsReview,
                        Some("its quote was not found in the cited transcript lines".into()),
                    )
                }
            }
            None => (Check::Linked, None),
        }
    }
}

fn keep_points(
    index: &Index,
    section: &str,
    points: Vec<Point>,
    dropped: &mut Vec<String>,
    flags: &mut Vec<String>,
) -> Vec<Point> {
    points
        .into_iter()
        .filter_map(|p| keep_point(index, section, p, dropped, flags))
        .collect()
}

fn keep_point(
    index: &Index,
    section: &str,
    p: Point,
    dropped: &mut Vec<String>,
    flags: &mut Vec<String>,
) -> Option<Point> {
    match index.evidence(&p.evidence) {
        Some(evidence) if !p.text.trim().is_empty() => {
            let (check, note) = index.assess_as(
                &p.text,
                &evidence,
                p.quote.as_deref(),
                section == "open questions",
            );
            if let Some(n) = &note {
                flags.push(format!("{section}: \"{}\": {n}", p.text));
            }
            Some(Point {
                evidence,
                check,
                note,
                ..p
            })
        }
        _ => {
            dropped.push(format!(
                "{section}: \"{}\" cited no transcript segment that exists",
                p.text
            ));
            None
        }
    }
}

/// With speakers instead of headphones, the microphone also records the other side, so the
/// same words appear on both tracks. A microphone segment that overlaps a system-audio
/// segment in time and shares most of its words is that echo, and is removed.
/// Returns how many segments were removed. Ids are renumbered afterwards.
pub fn drop_echo(segments: &mut Vec<Segment>) -> usize {
    fn words(t: &str) -> HashSet<String> {
        t.split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() > 1)
            .map(str::to_lowercase)
            .collect()
    }
    let system: Vec<(u64, u64, HashSet<String>)> = segments
        .iter()
        .filter(|s| is_system(&s.speaker))
        .map(|s| (s.start_ms, s.end_ms, words(&s.text)))
        .collect();
    let before = segments.len();
    segments.retain(|s| {
        if !is_mic(&s.speaker) {
            return true;
        }
        let mine = words(&s.text);
        if mine.is_empty() {
            return true;
        }
        let echo = system.iter().any(|(start, end, theirs)| {
            let overlap = s.start_ms < end + 3000 && *start < s.end_ms + 3000;
            let shared = mine.intersection(theirs).count();
            overlap && shared * 10 >= mine.len() * 6
        });
        !echo
    });
    for (i, s) in segments.iter_mut().enumerate() {
        s.id = format!("s{}", i + 1);
    }
    before - segments.len()
}

const SUMMARY_SPEC: &str = include_str!("../../../spec/prompts/summary.md");

/// The meeting-summary system prompt from `spec/prompts/summary.md`.
pub fn summary_prompt(output_language: &str, script: &str) -> String {
    let after = SUMMARY_SPEC
        .split_once("## System prompt")
        .map_or("", |(_, rest)| rest);
    let body = after.split_once("```\n").map_or("", |(_, rest)| rest);
    let body = body.split_once("\n```").map_or("", |(b, _)| b);
    let shape = r#"
JSON shape (all keys required; use [] or null when empty; "quote" is optional exact words from a cited segment):
{"overview":{"title":str,"date":str|null,"duration_min":num|null,"participants":[str],"purpose":str|null},
 "executive_summary":[{"text":str,"evidence":[id],"quote":str|null}],
 "topics":[{"title":str,"narrative":{"text":str,"evidence":[id],"quote":str|null}|null,"key_points":[{"text":str,"evidence":[id],"quote":str|null}]}],
 "decisions":[{"text":str,"status":"confirmed"|"proposed","evidence":[id],"quote":str|null}],
 "action_items":[{"action":str,"owner":str|null,"deadline":str|null,"evidence":[id],"quote":str|null}],
 "risks":[{"text":str,"evidence":[id]}],"open_questions":[{"text":str,"evidence":[id]}],
 "numbers":[{"value":str,"context":str,"evidence":[id]}],"next_steps":[{"text":str,"evidence":[id]}],
 "takeaway":{"text":str,"evidence":[id]}|null}"#;
    format!(
        "{}{shape}",
        body.replace('\r', "")
            .replace("{output_language}", output_language)
            .replace("{script}", script)
    )
}

/// The user message: transcript segments plus the user's own notes.
pub fn summary_request(title: &str, segments: &[Segment], thoughts: &[(u64, String)]) -> String {
    summary_request_part(title, segments, thoughts, 1, 1)
}

/// One window of a long meeting. Segment ids are those of the whole meeting.
pub fn summary_request_part(
    title: &str,
    segments: &[Segment],
    thoughts: &[(u64, String)],
    part: usize,
    of: usize,
) -> String {
    let segs: Vec<serde_json::Value> = segments
        .iter()
        .map(|s| {
            serde_json::json!({
                "id": s.id, "start": clock(s.start_ms), "end": clock(s.end_ms),
                "speaker": s.speaker, "text": s.text
            })
        })
        .collect();
    let notes: Vec<serde_json::Value> = thoughts
        .iter()
        .map(|(at, t)| serde_json::json!({ "at": clock(*at), "note": t }))
        .collect();
    let mut v = serde_json::json!({ "title": title, "segments": segs, "my_thoughts": notes });
    if of > 1 {
        v["part"] = serde_json::json!(format!(
            "{part} of {of}: summarise only these segments; other parts are summarised separately"
        ));
    }
    v.to_string()
}

/// Splits a transcript into windows of at most about `max_chars` characters, so that a long
/// meeting is never sent as one oversized request. Every segment is in exactly one window.
pub fn windows(segments: &[Segment], max_chars: usize) -> Vec<&[Segment]> {
    let mut out = Vec::new();
    let mut start = 0;
    let mut size = 0;
    for (i, s) in segments.iter().enumerate() {
        let cost = s.text.len() + 80;
        if size + cost > max_chars && i > start {
            out.push(&segments[start..i]);
            start = i;
            size = 0;
        }
        size += cost;
    }
    if start < segments.len() {
        out.push(&segments[start..]);
    }
    out
}

/// The user's notes that belong to a window (from its first segment to the next window).
pub fn thoughts_for(
    thoughts: &[(u64, String)],
    from_ms: u64,
    until_ms: Option<u64>,
) -> Vec<(u64, String)> {
    thoughts
        .iter()
        .filter(|(at, _)| *at >= from_ms && until_ms.is_none_or(|u| *at < u))
        .cloned()
        .collect()
}

/// Joins the summaries of several windows: topics with the same title are combined, repeated
/// decisions, actions, numbers and points are kept once. Run [`validate`] on the result.
pub fn merge(parts: Vec<Summary>) -> Summary {
    let mut parts = parts.into_iter();
    let Some(mut out) = parts.next() else {
        return Summary::default();
    };
    for p in parts {
        out.executive_summary.extend(p.executive_summary);
        for t in p.topics {
            match out
                .topics
                .iter_mut()
                .find(|x| norm(&x.title) == norm(&t.title))
            {
                Some(existing) => {
                    match (&mut existing.narrative, t.narrative) {
                        (Some(a), Some(b)) => {
                            a.text = format!("{} {}", a.text.trim(), b.text.trim());
                            a.evidence.extend(b.evidence);
                            a.quote = None;
                        }
                        (slot @ None, b) => *slot = b,
                        _ => {}
                    }
                    existing.points.extend(t.points);
                }
                None => out.topics.push(t),
            }
        }
        out.decisions.extend(p.decisions);
        out.action_items.extend(p.action_items);
        out.risks.extend(p.risks);
        out.open_questions.extend(p.open_questions);
        out.numbers.extend(p.numbers);
        out.next_steps.extend(p.next_steps);
        if p.takeaway.is_some() {
            out.takeaway = p.takeaway;
        }
        if out.overview.purpose.is_none() {
            out.overview.purpose = p.overview.purpose;
        }
        for name in p.overview.participants {
            if !out.overview.participants.contains(&name) {
                out.overview.participants.push(name);
            }
        }
    }
    let mut seen = HashSet::new();
    out.executive_summary.retain(|x| seen.insert(norm(&x.text)));
    out.executive_summary.truncate(8);
    for t in &mut out.topics {
        let mut seen = HashSet::new();
        t.points.retain(|x| seen.insert(norm(&x.text)));
    }
    let mut seen = HashSet::new();
    out.decisions.retain(|x| seen.insert(norm(&x.text)));
    let mut seen = HashSet::new();
    out.action_items.retain(|x| seen.insert(norm(&x.action)));
    for list in [&mut out.risks, &mut out.open_questions, &mut out.next_steps] {
        let mut seen = HashSet::new();
        list.retain(|x| seen.insert(norm(&x.text)));
    }
    let mut seen = HashSet::new();
    out.numbers
        .retain(|x| seen.insert((norm(&x.value), norm(&x.context))));
    out
}

/// Reads the model's JSON answer (tolerates fences and chatter around it).
pub fn parse_summary(content: &str) -> Result<Summary, String> {
    let (s, e) = (content.find('{'), content.rfind('}'));
    match (s, e) {
        (Some(s), Some(e)) if e > s => {
            serde_json::from_str(&content[s..=e]).map_err(|e| format!("summary JSON invalid: {e}"))
        }
        _ => Err("no JSON in the answer".into()),
    }
}

pub fn clock(ms: u64) -> String {
    let s = ms / 1000;
    format!("{:02}:{:02}:{:02}", s / 3600, (s / 60) % 60, s % 60)
}

fn marker(check: Check) -> &'static str {
    match check {
        Check::NeedsReview => " *(check this)*",
        _ => "",
    }
}

/// Plain Markdown notes, in the section order of the build plan, empty sections left out.
/// Topics are a paragraph first; bullets only for distinct points.
/// "Microphone" / "System audio" (and older "Me" / "Others") are capture sources, not people.
pub fn is_audio_source(name: &str) -> bool {
    let n = name.trim().to_lowercase();
    [
        MIC,
        SYSTEM,
        "me",
        "others",
        "microphone",
        "system audio",
        "mic",
    ]
    .iter()
    .any(|s| s.to_lowercase() == n)
}

pub fn to_markdown(summary: &Summary, segments: &[Segment], thoughts: &[(u64, String)]) -> String {
    let at: HashMap<&str, u64> = segments
        .iter()
        .map(|s| (s.id.as_str(), s.start_ms))
        .collect();
    let cite = |ev: &[String]| {
        let times: Vec<String> = ev
            .iter()
            .filter_map(|id| at.get(id.as_str()))
            .map(|ms| clock(*ms))
            .collect();
        if times.is_empty() {
            String::new()
        } else {
            format!(" [{}]", times.join(", "))
        }
    };
    let mut md = String::new();
    let o = &summary.overview;
    md.push_str(&format!(
        "# {}\n\n",
        if o.title.is_empty() {
            "Meeting notes"
        } else {
            &o.title
        }
    ));
    let mut meta = Vec::new();
    if let Some(d) = &o.date {
        meta.push(format!("Date: {d}"));
    }
    if let Some(m) = o.duration_min {
        meta.push(if m < 1.0 {
            format!("Duration: {:.0} s", m * 60.0)
        } else {
            format!("Duration: {m:.0} min")
        });
    }
    // "Microphone" and "System audio" are where the sound came from, not people: they are listed
    // as audio sources, and people only when the notes name them (no speaker recognition yet).
    let people: Vec<&String> = o
        .participants
        .iter()
        .filter(|p| !is_audio_source(p))
        .collect();
    meta.push(if people.is_empty() {
        "Participants: not identified".to_string()
    } else {
        format!(
            "Participants: {}",
            people
                .iter()
                .map(|p| p.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    });
    let mut sources: Vec<&str> = Vec::new();
    for s in segments {
        if !sources.contains(&s.speaker.as_str()) {
            sources.push(&s.speaker);
        }
    }
    if !sources.is_empty() {
        meta.push(format!("Audio sources: {}", sources.join(", ")));
    }
    if let Some(p) = &o.purpose {
        meta.push(format!("Purpose: {p}"));
    }
    if !meta.is_empty() {
        md.push_str(&format!("{}\n\n", meta.join("  \n")));
    }
    let points = |md: &mut String, title: &str, items: &[Point]| {
        if !items.is_empty() {
            md.push_str(&format!("## {title}\n\n"));
            for p in items {
                md.push_str(&format!(
                    "- {}{}{}\n",
                    p.text,
                    cite(&p.evidence),
                    marker(p.check)
                ));
            }
            md.push('\n');
        }
    };
    points(&mut md, "Executive summary", &summary.executive_summary);
    if !summary.topics.is_empty() {
        md.push_str("## Key discussion points\n\n");
        for t in &summary.topics {
            md.push_str(&format!("### {}\n\n", t.title));
            if let Some(n) = &t.narrative {
                md.push_str(&format!(
                    "{}{}{}\n\n",
                    n.text.trim(),
                    cite(&n.evidence),
                    marker(n.check)
                ));
            }
            for p in &t.points {
                md.push_str(&format!(
                    "- {}{}{}\n",
                    p.text,
                    cite(&p.evidence),
                    marker(p.check)
                ));
            }
            if !t.points.is_empty() {
                md.push('\n');
            }
        }
    }
    if !summary.decisions.is_empty() {
        md.push_str("## Decisions\n\n");
        for d in &summary.decisions {
            let status = match d.status {
                DecisionStatus::Confirmed => "Confirmed",
                DecisionStatus::Proposed => "Proposed",
            };
            md.push_str(&format!(
                "- **{status}:** {}{}{}\n",
                d.text,
                cite(&d.evidence),
                marker(d.check)
            ));
        }
        md.push('\n');
    }
    if !summary.action_items.is_empty() {
        md.push_str(
            "## Action items\n\n| Action | Owner | Deadline | Evidence |\n|---|---|---|---|\n",
        );
        let cell = |s: &str| s.replace('|', "/").replace('\n', " ");
        for a in &summary.action_items {
            let deadline = match (&a.deadline, a.deadline_tentative) {
                (Some(d), true) => format!("{} (tentative)", cell(d)),
                (Some(d), false) => cell(d),
                (None, _) => "Not specified".into(),
            };
            md.push_str(&format!(
                "| {}{} | {} | {} |{} |\n",
                cell(&a.action),
                marker(a.check),
                a.owner.as_deref().map_or("Not specified".into(), cell),
                deadline,
                cite(&a.evidence)
            ));
        }
        md.push('\n');
    }
    points(&mut md, "Risks and challenges", &summary.risks);
    points(&mut md, "Open questions", &summary.open_questions);
    if !summary.numbers.is_empty() {
        md.push_str("## Important numbers\n\n");
        for n in &summary.numbers {
            md.push_str(&format!(
                "- **{}** {}{}\n",
                n.value,
                n.context,
                cite(&n.evidence)
            ));
        }
        md.push('\n');
    }
    points(&mut md, "Next steps", &summary.next_steps);
    if let Some(t) = &summary.takeaway {
        md.push_str(&format!(
            "## Final takeaway\n\n{}{}{}\n\n",
            t.text,
            cite(&t.evidence),
            marker(t.check)
        ));
    }
    if !thoughts.is_empty() {
        md.push_str("## My thoughts\n\n");
        for (ms, t) in thoughts {
            md.push_str(&format!("- [{}] {t}\n", clock(*ms)));
        }
        md.push('\n');
    }
    md.push_str("## Transcript\n\n");
    for s in segments {
        md.push_str(&format!(
            "[{}] **{}:** {}\n\n",
            clock(s.start_ms),
            s.speaker,
            s.text
        ));
    }
    md
}

/// Plain text version of the notes (Markdown symbols removed), for the TXT export.
pub fn to_text(markdown: &str) -> String {
    markdown
        .lines()
        .filter(|l| !l.starts_with("|---"))
        .map(|l| {
            let l = l.trim_start_matches('#').trim_start();
            let l = l
                .replace("**", "")
                .replace("*(check this)*", "(check this)");
            if l.starts_with('|') {
                l.trim_matches('|')
                    .split('|')
                    .map(str::trim)
                    .collect::<Vec<_>>()
                    .join(" | ")
            } else {
                l
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn validate(summary: Summary, segments: &[Segment]) -> Validated {
    let mut summary = summary;
    summary
        .overview
        .participants
        .retain(|p| !is_audio_source(p));
    let index = Index::new(segments);
    let mut dropped = Vec::new();
    let mut flags = Vec::new();

    let executive_summary = keep_points(
        &index,
        "executive summary",
        summary.executive_summary,
        &mut dropped,
        &mut flags,
    );
    let topics = summary
        .topics
        .into_iter()
        .filter_map(|t| {
            let section = format!("topic \"{}\"", t.title);
            let narrative = t
                .narrative
                .and_then(|n| keep_point(&index, &section, n, &mut dropped, &mut flags));
            let points = keep_points(&index, &section, t.points, &mut dropped, &mut flags);
            (narrative.is_some() || !points.is_empty()).then_some(Topic {
                narrative,
                points,
                ..t
            })
        })
        .collect();
    let decisions = summary
        .decisions
        .into_iter()
        .filter_map(|d| {
            let Some(evidence) = index.evidence(&d.evidence) else {
                dropped.push(format!(
                    "decision: \"{}\" cited no transcript segment that exists",
                    d.text
                ));
                return None;
            };
            let mut evidence = evidence;
            let (mut check, mut note) = index.assess(&d.text, &evidence, d.quote.as_deref());
            // A detail missing from the cited lines (the "above 3x" condition) is often in the
            // line or two just before them: cite those too if that makes every detail found.
            if check == Check::NeedsReview {
                for k in 1..=2 {
                    let wider = index.with_previous(&evidence, k);
                    if wider.len() == evidence.len() {
                        break;
                    }
                    let (c, n) = index.assess(&d.text, &wider, d.quote.as_deref());
                    if c != Check::NeedsReview {
                        evidence = wider;
                        check = c;
                        note = n;
                        break;
                    }
                }
            }
            let cited = index.cited_text(&evidence);
            let mut status = d.status;
            if status == DecisionStatus::Confirmed {
                let why = if contains_any(&cited, DISAGREEMENT) {
                    Some("someone disagreed in the cited lines")
                } else if contains_any(&cited, HEDGES) {
                    Some("the cited lines are tentative")
                } else if !contains_any(&cited, AGREEMENT) {
                    Some("the cited lines contain no words of agreement")
                } else {
                    None
                };
                if let Some(why) = why {
                    status = DecisionStatus::Proposed;
                    flags.push(format!("decision \"{}\": shown as proposed, {why}", d.text));
                    // The status change already handles the hedging.
                    if note.as_deref()
                        == Some("the speaker sounded unsure, but the item reads as certain")
                    {
                        check = Check::Linked;
                        note = None;
                    }
                }
            }
            // Never "Confirmed" next to "check this": a decision whose details are not all in
            // the cited lines is shown as proposed until the user checks it.
            if status == DecisionStatus::Confirmed && check == Check::NeedsReview {
                status = DecisionStatus::Proposed;
                flags.push(format!(
                    "decision \"{}\": shown as proposed, a detail in it is not in the cited lines",
                    d.text
                ));
            }
            if let Some(n) = &note {
                flags.push(format!("decision \"{}\": {n}", d.text));
            }
            Some(Decision {
                status,
                evidence,
                check,
                note,
                ..d
            })
        })
        .collect();
    let action_items = summary
        .action_items
        .into_iter()
        .filter_map(|a| {
            let Some(evidence) = index.evidence(&a.evidence) else {
                dropped.push(format!(
                    "action: \"{}\" cited no transcript segment that exists",
                    a.action
                ));
                return None;
            };
            let owner = a.owner.filter(|o| {
                let ok = index.cited_mentions(&evidence, o);
                if !ok {
                    dropped.push(format!(
                        "action \"{}\": owner \"{o}\" not in the cited words -> Not specified",
                        a.action
                    ));
                }
                ok
            });
            let deadline = a.deadline.filter(|d| {
                let ok = index.cited_mentions(&evidence, d);
                if !ok {
                    dropped.push(format!(
                        "action \"{}\": deadline \"{d}\" not in the cited words -> Not specified",
                        a.action
                    ));
                }
                ok
            });
            let cited = index.cited_text(&evidence);
            let deadline_tentative = deadline.is_some() && contains_any(&cited, HEDGES);
            let (mut check, mut note) = index.assess(&a.action, &evidence, a.quote.as_deref());
            if deadline_tentative
                && note.as_deref()
                    == Some("the speaker sounded unsure, but the item reads as certain")
            {
                // Shown as "(tentative)" next to the deadline instead.
                check = Check::Linked;
                note = None;
            }
            if let Some(n) = &note {
                flags.push(format!("action \"{}\": {n}", a.action));
            }
            Some(ActionItem {
                owner,
                deadline,
                evidence,
                check,
                note,
                deadline_tentative,
                ..a
            })
        })
        .collect();
    let risks = keep_points(&index, "risks", summary.risks, &mut dropped, &mut flags);
    let open_questions = keep_points(
        &index,
        "open questions",
        summary.open_questions,
        &mut dropped,
        &mut flags,
    );
    let numbers = summary
        .numbers
        .into_iter()
        .filter_map(|n| {
            let evidence = index.evidence(&n.evidence)?;
            // "41% to 38%" holds two numbers: each must be in the cited words.
            let runs = digit_runs(&n.value);
            let supported = if runs.is_empty() {
                index.cited_has_number(&evidence, &n.value)
            } else {
                runs.iter().all(|r| index.cited_has_number(&evidence, r))
            };
            if supported {
                Some(NumberItem { evidence, ..n })
            } else {
                dropped.push(format!(
                    "number \"{}\" does not appear in the cited words",
                    n.value
                ));
                None
            }
        })
        .collect();
    let next_steps = keep_points(
        &index,
        "next steps",
        summary.next_steps,
        &mut dropped,
        &mut flags,
    );
    let takeaway = summary
        .takeaway
        .and_then(|t| keep_point(&index, "takeaway", t, &mut dropped, &mut flags));

    Validated {
        summary: Summary {
            overview: summary.overview,
            executive_summary,
            topics,
            decisions,
            action_items,
            risks,
            open_questions,
            numbers,
            next_steps,
            takeaway,
        },
        dropped,
        flags,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The golden topic-organisation transcript from the build prompt, one sentence per segment.
    fn golden_segments() -> Vec<Segment> {
        [
            "First we discussed the sales performance.",
            "Sales increased by 12 percent.",
            "Then we talked about the marketing budget.",
            "The marketing team needs another 50,000 rupees.",
            "Then we discussed the hiring process.",
            "Three interviews are pending.",
            "Finally, we decided to review everything next Monday.",
        ]
        .iter()
        .enumerate()
        .map(|(i, t)| Segment {
            id: format!("s{}", i + 1),
            start_ms: i as u64 * 4000,
            end_ms: i as u64 * 4000 + 3500,
            speaker: MIC.into(),
            text: t.to_string(),
        })
        .collect()
    }

    fn segs(texts: &[&str]) -> Vec<Segment> {
        texts
            .iter()
            .enumerate()
            .map(|(i, t)| Segment {
                id: format!("s{}", i + 1),
                start_ms: i as u64 * 5000,
                end_ms: i as u64 * 5000 + 4000,
                speaker: SYSTEM.into(),
                text: t.to_string(),
            })
            .collect()
    }

    fn p(text: &str, ev: &[&str]) -> Point {
        Point::new(text, ev)
    }

    fn action(text: &str, owner: Option<&str>, deadline: Option<&str>, ev: &[&str]) -> ActionItem {
        ActionItem {
            action: text.into(),
            owner: owner.map(Into::into),
            deadline: deadline.map(Into::into),
            evidence: ev.iter().map(|s| s.to_string()).collect(),
            quote: None,
            check: Check::Linked,
            deadline_tentative: false,
            note: None,
        }
    }

    fn decision(text: &str, status: DecisionStatus, ev: &[&str]) -> Decision {
        Decision {
            text: text.into(),
            status,
            evidence: ev.iter().map(|s| s.to_string()).collect(),
            quote: None,
            check: Check::Linked,
            note: None,
        }
    }

    fn topic(title: &str, narrative: Option<Point>, points: Vec<Point>) -> Topic {
        Topic {
            title: title.into(),
            narrative,
            points,
        }
    }

    #[test]
    fn golden_summary_survives_and_inventions_are_removed() {
        let model_answer = Summary {
            overview: Overview {
                title: "Weekly review".into(),
                ..Default::default()
            },
            topics: vec![
                topic(
                    "Sales Performance",
                    None,
                    vec![p("Sales increased by 12%.", &["s2"])],
                ),
                topic(
                    "Marketing Budget",
                    None,
                    vec![p("Additional ₹50,000 requested for marketing.", &["s4"])],
                ),
                topic(
                    "Recruitment",
                    None,
                    vec![p("Three interviews are pending.", &["s6"])],
                ),
                // An invented topic citing a segment that does not exist.
                topic(
                    "Previous period",
                    None,
                    vec![p(
                        "Review performance against the previous period.",
                        &["s99"],
                    )],
                ),
            ],
            decisions: vec![decision(
                "Review everything next Monday.",
                DecisionStatus::Confirmed,
                &["s7"],
            )],
            action_items: vec![action(
                "Review everything",
                Some("Rahul"),       // never said
                Some("next Monday"), // said
                &["s7"],
            )],
            numbers: vec![
                NumberItem {
                    value: "12%".into(),
                    context: "sales growth".into(),
                    evidence: vec!["s2".into()],
                },
                NumberItem {
                    value: "₹50,000".into(),
                    context: "marketing".into(),
                    evidence: vec!["s4".into()],
                },
                NumberItem {
                    value: "15%".into(),
                    context: "invented".into(),
                    evidence: vec!["s2".into()],
                },
            ],
            ..Default::default()
        };
        let v = validate(model_answer, &golden_segments());
        let s = &v.summary;
        assert_eq!(
            s.topics
                .iter()
                .map(|t| t.title.as_str())
                .collect::<Vec<_>>(),
            ["Sales Performance", "Marketing Budget", "Recruitment"]
        );
        assert_eq!(s.decisions.len(), 1);
        assert_eq!(
            s.decisions[0].status,
            DecisionStatus::Confirmed,
            "\"we decided\" is agreement"
        );
        assert_eq!(
            s.action_items[0].owner, None,
            "an owner nobody named becomes Not specified"
        );
        assert_eq!(s.action_items[0].deadline.as_deref(), Some("next Monday"));
        assert!(!s.action_items[0].deadline_tentative);
        assert_eq!(
            s.numbers
                .iter()
                .map(|n| n.value.as_str())
                .collect::<Vec<_>>(),
            ["12%", "₹50,000"]
        );
        assert_eq!(v.dropped.len(), 3, "{:#?}", v.dropped);
        assert!(v.flags.is_empty(), "{:#?}", v.flags);
    }

    #[test]
    fn a_real_segment_id_attached_to_an_unrelated_claim_is_marked() {
        let v = validate(
            Summary {
                action_items: vec![action("Hire two engineers", None, None, &["s2"])],
                executive_summary: vec![p("Travel costs were high.", &["s4"])],
                ..Default::default()
            },
            &golden_segments(),
        );
        assert_eq!(v.summary.action_items[0].check, Check::NeedsReview);
        assert_eq!(v.summary.executive_summary[0].check, Check::NeedsReview);
        assert_eq!(v.flags.len(), 2);
    }

    #[test]
    fn a_number_not_in_the_cited_lines_is_marked_even_inside_a_sentence() {
        let v = validate(
            Summary {
                executive_summary: vec![
                    p("Sales increased by 15%.", &["s2"]),
                    p("Sales increased by 12%.", &["s2"]),
                ],
                ..Default::default()
            },
            &golden_segments(),
        );
        assert_eq!(v.summary.executive_summary[0].check, Check::NeedsReview);
        assert_eq!(v.summary.executive_summary[1].check, Check::Linked);
    }

    #[test]
    fn correct_amount_in_the_wrong_context_is_marked() {
        let v = validate(
            Summary {
                executive_summary: vec![Point {
                    quote: Some("50,000 rupees".into()),
                    ..p("Travel costs were ₹50,000.", &["s4"])
                }],
                ..Default::default()
            },
            &golden_segments(),
        );
        assert_eq!(v.summary.executive_summary[0].check, Check::NeedsReview);
    }

    #[test]
    fn quotes_must_be_found_word_for_word() {
        let v = validate(
            Summary {
                risks: vec![
                    Point {
                        quote: Some("needs another 50,000 rupees".into()),
                        ..p("Marketing needs another ₹50,000.", &["s4"])
                    },
                    Point {
                        quote: Some("needs another 80,000 rupees".into()),
                        ..p("Marketing needs more money.", &["s4"])
                    },
                ],
                ..Default::default()
            },
            &golden_segments(),
        );
        assert_eq!(v.summary.risks[0].check, Check::Quoted);
        assert_eq!(v.summary.risks[1].check, Check::NeedsReview);
    }

    #[test]
    fn wrong_deadline_is_blanked_and_maybe_makes_it_tentative() {
        let s = segs(&[
            "Priya will send the forecast maybe next Friday.",
            "Ravi will finish the slides by Thursday.",
        ]);
        let v = validate(
            Summary {
                action_items: vec![
                    action("Send the forecast", Some("Priya"), Some("Friday"), &["s1"]),
                    action(
                        "Finish the slides",
                        Some("Ravi"),
                        Some("Wednesday"),
                        &["s2"],
                    ),
                ],
                ..Default::default()
            },
            &s,
        );
        let a = &v.summary.action_items;
        assert_eq!(a[0].deadline.as_deref(), Some("Friday"));
        assert!(
            a[0].deadline_tentative,
            "\"maybe next Friday\" is not a firm deadline"
        );
        assert_eq!(a[1].deadline, None, "Wednesday was never said");
        assert_eq!(a[1].owner.as_deref(), Some("Ravi"));
        let md = to_markdown(&v.summary, &s, &[]);
        assert!(md.contains("Friday (tentative)"), "{md}");
    }

    #[test]
    fn an_owner_inferred_from_context_but_not_named_is_not_kept() {
        let s = segs(&["Someone from finance should check the invoice."]);
        let v = validate(
            Summary {
                action_items: vec![action("Check the invoice", Some("Anita"), None, &["s1"])],
                ..Default::default()
            },
            &s,
        );
        assert_eq!(v.summary.action_items[0].owner, None);
    }

    #[test]
    fn a_proposal_is_never_shown_as_a_confirmed_decision() {
        let s = segs(&[
            "Maybe we should move the launch to March.",
            "We could switch to the cheaper vendor.",
            "I disagree, we decided nothing about the vendor.",
            "Okay, agreed, we will launch in March.",
        ]);
        let v = validate(
            Summary {
                decisions: vec![
                    decision(
                        "Move the launch to March.",
                        DecisionStatus::Confirmed,
                        &["s1"],
                    ),
                    decision(
                        "Switch to the cheaper vendor.",
                        DecisionStatus::Confirmed,
                        &["s2"],
                    ),
                    decision(
                        "Use the cheaper vendor.",
                        DecisionStatus::Confirmed,
                        &["s3"],
                    ),
                    decision("Launch in March.", DecisionStatus::Confirmed, &["s4"]),
                ],
                ..Default::default()
            },
            &s,
        );
        let st: Vec<DecisionStatus> = v.summary.decisions.iter().map(|d| d.status).collect();
        assert_eq!(
            st,
            [
                DecisionStatus::Proposed,
                DecisionStatus::Proposed,
                DecisionStatus::Proposed,
                DecisionStatus::Confirmed
            ]
        );
    }

    /// The 0.4.3 meeting test (audit, 19 Nov 2026): the approval cited only "Agreed..." while the
    /// "above three times" condition was in the line before; it showed "Confirmed" and "check
    /// this" together. Now the earlier line is cited and the decision checks out; a detail that
    /// is nowhere near the cited lines makes it "proposed", never "confirmed + check this".
    #[test]
    fn a_confirmed_decision_never_carries_a_check_this_note() {
        let s = segs(&[
            "Second, the marketing budget. The marketing team is asking for an additional 50,000 rupees for the festive campaign.",
            "I am fine with that only if the campaign return on investment stays above three times.",
            "Agreed. Let us approve the 50,000 rupees with that condition.",
            "Third, hiring. We have three interviews pending.",
        ]);
        let d = "Approve an additional Rs 50,000 for the festive marketing campaign, provided the ROI stays above 3x.";
        let v = validate(
            Summary {
                decisions: vec![
                    decision(d, DecisionStatus::Confirmed, &["s3"]),
                    decision(
                        "Approve 75,000 rupees for the campaign.",
                        DecisionStatus::Confirmed,
                        &["s3"],
                    ),
                ],
                ..Default::default()
            },
            &s,
        );
        let ok = &v.summary.decisions[0];
        assert_eq!(ok.status, DecisionStatus::Confirmed);
        assert_ne!(ok.check, Check::NeedsReview);
        assert!(ok.evidence.contains(&"s2".to_string()), "{:?}", ok.evidence);
        let wrong = &v.summary.decisions[1];
        assert_eq!(wrong.status, DecisionStatus::Proposed);
        assert_eq!(wrong.check, Check::NeedsReview);
        for d in &v.summary.decisions {
            assert!(!(d.status == DecisionStatus::Confirmed && d.check == Check::NeedsReview));
        }
        let md = to_markdown(&v.summary, &s, &[]);
        assert!(!md.contains("**Confirmed:** Approve 75,000"), "{md}");
    }

    /// Found in the real-app meeting test (17 Nov 2026): quotes joined with "...", a value
    /// with two numbers, an open question, and a topic that itself says "no definitive answer".
    #[test]
    fn real_meeting_cases_are_not_marked_wrongly() {
        let s = segs(&[
            "Revenue grew by 12% compared to last quarter, mainly from the western region.",
            "That is good news, but our gross margin dropped from 41% to 38% because of higher freight costs.",
            "Do we know if the new Power BI dashboard will be ready for the board meeting? I am not sure yet.",
            "We will decide that next week.",
        ]);
        let v = validate(
            Summary {
                topics: vec![
                    topic(
                        "Sales",
                        Some(Point {
                            quote: Some("Revenue grew by 12%... gross margin dropped from 41% to 38%".into()),
                            ..p("Revenue grew 12% but gross margin fell from 41% to 38%.", &["s1", "s2"])
                        }),
                        vec![],
                    ),
                    topic(
                        "Power BI dashboard",
                        Some(p(
                            "Whether the Power BI dashboard is ready for the board meeting was questioned, with no definitive answer; it will be decided next week.",
                            &["s3", "s4"],
                        )),
                        vec![],
                    ),
                ],
                open_questions: vec![p(
                    "Will the new Power BI dashboard be ready for the board meeting?",
                    &["s3"],
                )],
                numbers: vec![NumberItem {
                    value: "41% to 38%".into(),
                    context: "gross margin".into(),
                    evidence: vec!["s2".into()],
                }],
                ..Default::default()
            },
            &s,
        );
        let t = &v.summary.topics;
        assert_eq!(
            t[0].narrative.as_ref().unwrap().check,
            Check::Quoted,
            "{:#?}",
            v.flags
        );
        assert_eq!(
            t[1].narrative.as_ref().unwrap().check,
            Check::Linked,
            "{:#?}",
            v.flags
        );
        assert_eq!(v.summary.open_questions[0].check, Check::Linked);
        assert_eq!(
            v.summary.numbers.len(),
            1,
            "both numbers are in the line: {:#?}",
            v.dropped
        );
        assert!(v.flags.is_empty(), "{:#?}", v.flags);

        // A joined quote with one invented piece is still caught.
        let bad = validate(
            Summary {
                risks: vec![Point {
                    quote: Some("gross margin dropped... freight costs doubled".into()),
                    ..p(
                        "Gross margin dropped because freight costs doubled.",
                        &["s2"],
                    )
                }],
                numbers: vec![NumberItem {
                    value: "41% to 35%".into(),
                    context: "gross margin".into(),
                    evidence: vec!["s2".into()],
                }],
                ..Default::default()
            },
            &s,
        );
        assert_eq!(bad.summary.risks[0].check, Check::NeedsReview);
        assert!(bad.summary.numbers.is_empty());
    }

    #[test]
    fn removed_uncertainty_is_marked() {
        let s = segs(&["We might launch the product in March, not sure yet."]);
        let v = validate(
            Summary {
                executive_summary: vec![
                    p("The product launches in March.", &["s1"]),
                    p("The product might launch in March.", &["s1"]),
                ],
                ..Default::default()
            },
            &s,
        );
        assert_eq!(v.summary.executive_summary[0].check, Check::NeedsReview);
        assert_eq!(v.summary.executive_summary[1].check, Check::Linked);
        // Found in the Hinglish meeting test: "may be broken" keeps the speaker's "shayad".
        let s = segs(&["Mera phone charge nahi ho raha, shayad charger kharab hai."]);
        let v = validate(
            Summary {
                risks: vec![
                    p("The phone charger may be broken.", &["s1"]),
                    p("The phone charger is broken.", &["s1"]),
                ],
                ..Default::default()
            },
            &s,
        );
        assert_eq!(v.summary.risks[0].check, Check::Linked, "{:#?}", v.flags);
        assert_eq!(
            v.summary.risks[1].check,
            Check::NeedsReview,
            "the hedge was dropped"
        );
    }

    #[test]
    fn topics_keep_a_paragraph_and_render_bullets_only_for_points() {
        let s = segs(&[
            "Sales went up 12 percent this quarter because volumes were higher.",
            "But discounts reduced the margin.",
            "Priya will send the margin report by Friday.",
        ]);
        let v = validate(
            Summary {
                topics: vec![topic(
                    "Sales performance",
                    Some(p(
                        "Sales rose 12% on higher volumes, but discounts reduced the margin.",
                        &["s1", "s2"],
                    )),
                    vec![],
                )],
                action_items: vec![action(
                    "Send the margin report",
                    Some("Priya"),
                    Some("Friday"),
                    &["s3"],
                )],
                ..Default::default()
            },
            &s,
        );
        let md = to_markdown(&v.summary, &s, &[]);
        let topic_part = md
            .split("### Sales performance")
            .nth(1)
            .unwrap()
            .split("## Action items")
            .next()
            .unwrap();
        assert!(
            topic_part
                .contains("Sales rose 12% on higher volumes, but discounts reduced the margin."),
            "{md}"
        );
        assert!(!topic_part.contains("\n- "), "no bullet per sentence: {md}");
        assert!(
            md.contains("| Send the margin report | Priya | Friday |"),
            "{md}"
        );
    }

    #[test]
    fn long_meetings_are_split_into_windows_and_merged_without_duplicates() {
        let texts: Vec<String> = (0..50)
            .map(|i| format!("Line {i} about the budget."))
            .collect();
        let refs: Vec<&str> = texts.iter().map(String::as_str).collect();
        let s = segs(&refs);
        let w = windows(&s, 1000);
        assert!(w.len() > 1);
        assert_eq!(
            w.iter().map(|x| x.len()).sum::<usize>(),
            s.len(),
            "every segment once"
        );
        assert!(w
            .iter()
            .all(|x| x.iter().map(|y| y.text.len() + 80).sum::<usize>() <= 1000));

        let part = |n: &str, ev: &str| Summary {
            topics: vec![topic(
                "Budget",
                Some(p(n, &[ev])),
                vec![p("Budget is tight.", &[ev])],
            )],
            action_items: vec![action("Send the budget", None, None, &[ev])],
            ..Default::default()
        };
        let merged = merge(vec![
            part("Budget talk one.", "s1"),
            part("Budget talk two.", "s30"),
        ]);
        assert_eq!(merged.topics.len(), 1);
        assert_eq!(
            merged.topics[0].points.len(),
            1,
            "duplicate point kept once"
        );
        assert_eq!(merged.action_items.len(), 1, "duplicate action kept once");
        let n = merged.topics[0].narrative.as_ref().unwrap();
        assert_eq!(n.text, "Budget talk one. Budget talk two.");
        assert_eq!(n.evidence, ["s1", "s30"]);
        assert_eq!(
            thoughts_for(&[(1000, "a".into()), (9000, "b".into())], 0, Some(5000)).len(),
            1
        );
    }

    #[test]
    fn echo_of_the_other_side_is_removed_but_own_words_stay() {
        let seg = |speaker: &str, start: u64, text: &str| Segment {
            id: String::new(),
            start_ms: start,
            end_ms: start + 8000,
            speaker: speaker.into(),
            text: text.into(),
        };
        let mut s = vec![
            seg(SYSTEM, 0, "Sales increased by 12 percent this quarter."),
            seg(MIC, 500, "Sales increased by twelve percent this quarter."), // echo through the speakers
            seg(MIC, 9000, "Great, then let us hire two more people."),
            seg("Others", 60000, "Agreed."),
            seg(
                "Me",
                200000,
                "Sales increased this quarter, I said that last week too.",
            ), // far away in time
        ];
        assert_eq!(drop_echo(&mut s), 1);
        assert_eq!(s.iter().filter(|x| is_mic(&x.speaker)).count(), 2);
        assert_eq!(
            s.iter().map(|x| x.id.as_str()).collect::<Vec<_>>(),
            ["s1", "s2", "s3", "s4"]
        );
    }

    #[test]
    fn spelled_out_numbers_support_digits() {
        let v = validate(
            Summary {
                numbers: vec![
                    NumberItem {
                        value: "3".into(),
                        context: "interviews".into(),
                        evidence: vec!["s6".into()],
                    },
                    NumberItem {
                        value: "4".into(),
                        context: "invented".into(),
                        evidence: vec!["s6".into()],
                    },
                ],
                ..Default::default()
            },
            &golden_segments(),
        );
        assert_eq!(v.summary.numbers.len(), 1);
        assert_eq!(v.summary.numbers[0].value, "3");
    }

    #[test]
    fn items_without_evidence_are_dropped_everywhere() {
        let v = validate(
            Summary {
                executive_summary: vec![p("Unsupported claim", &[])],
                risks: vec![
                    p("Marketing budget may not be enough", &["s4"]),
                    p("Made up", &["nope"]),
                ],
                takeaway: Some(p("Made up takeaway", &["x"])),
                ..Default::default()
            },
            &golden_segments(),
        );
        assert!(v.summary.executive_summary.is_empty());
        assert_eq!(v.summary.risks.len(), 1);
        assert!(v.summary.takeaway.is_none());
        assert_eq!(v.dropped.len(), 3);
    }

    #[test]
    fn invalid_evidence_ids_are_removed_but_valid_ones_kept() {
        let v = validate(
            Summary {
                next_steps: vec![p("Review everything on Monday", &["s7", "s404"])],
                ..Default::default()
            },
            &golden_segments(),
        );
        assert_eq!(v.summary.next_steps[0].evidence, ["s7"]);
    }

    #[test]
    fn prompt_and_markdown() {
        let prompt = summary_prompt("English", "Latin");
        assert!(prompt.starts_with("You produce evidence-grounded meeting notes"));
        assert!(!prompt.contains("{output_language}"));
        assert!(prompt.contains("\"action_items\"") && prompt.contains("\"narrative\""));
        let segs = golden_segments();
        let req = summary_request("Weekly", &segs, &[(5000, "ask about budget".into())]);
        assert!(req.contains("\"id\":\"s4\"") && req.contains("ask about budget"));
        assert!(!req.contains("\"part\""));
        assert!(summary_request_part("Weekly", &segs, &[], 2, 3).contains("2 of 3"));

        let s = Summary {
            overview: Overview {
                title: "Weekly".into(),
                ..Default::default()
            },
            action_items: vec![action("Review", None, Some("next Monday"), &["s7"])],
            ..Default::default()
        };
        let md = to_markdown(&s, &segs, &[(5000, "ask about budget".into())]);
        assert!(
            md.contains("| Review | Not specified | next Monday | [00:00:24] |"),
            "{md}"
        );
        assert!(md.contains("- [00:00:05] ask about budget"));
        assert!(md.contains("[00:00:04] **Microphone:** Sales increased by 12 percent."));
        assert!(!md.contains("## Risks"), "empty sections are left out");
        let txt = to_text(&md);
        assert!(
            !txt.contains("**") && !txt.contains("|---") && txt.contains("Review | Not specified")
        );
        assert!(parse_summary("```json\n{\"overview\":{\"title\":\"x\",\"date\":null,\"duration_min\":null,\"participants\":[],\"purpose\":null},\"takeaway\":null}\n```").is_ok());
    }

    #[test]
    fn json_from_the_schema_parses_old_and_new_shapes() {
        let old = r#"{"overview":{"title":"T","date":null,"duration_min":12.5,"participants":["Me","Others"],"purpose":null},
          "executive_summary":[],"topics":[{"title":"x","points":[{"text":"a","evidence":["s1"]}]}],"decisions":[{"text":"x","status":"proposed","evidence":["s1"]}],
          "action_items":[{"action":"a","owner":null,"deadline":null,"evidence":["s1"]}],
          "risks":[],"open_questions":[],"numbers":[],"next_steps":[],"takeaway":null}"#;
        let s: Summary = serde_json::from_str(old).unwrap();
        assert_eq!(s.decisions[0].status, DecisionStatus::Proposed);
        assert_eq!(s.overview.duration_min, Some(12.5));
        assert_eq!(s.topics[0].points.len(), 1);
        let new = r#"{"overview":{"title":"T","date":null,"duration_min":null,"participants":[],"purpose":null},
          "executive_summary":[],"topics":[{"title":"x","narrative":{"text":"n","evidence":["s1"],"quote":"q"},"key_points":[]}],
          "decisions":[],"action_items":[],"risks":[],"open_questions":[],"numbers":[],"next_steps":[],"takeaway":null}"#;
        let s: Summary = serde_json::from_str(new).unwrap();
        assert_eq!(
            s.topics[0].narrative.as_ref().unwrap().quote.as_deref(),
            Some("q")
        );
    }
}

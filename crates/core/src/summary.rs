//! Meeting-summary validation (Stage 3).
//!
//! The AI writes the summary as JSON (`spec/schemas/meeting_summary.schema.json`); this module
//! checks it against the transcript before anything is shown:
//! - every item must cite at least one segment id that exists;
//! - an owner or deadline must appear word for word in a cited segment, otherwise it is set to
//!   null (the UI shows "Not specified");
//! - a number in the "numbers" list must appear in a cited segment, otherwise the item is dropped;
//! - items that fail are dropped and listed in `dropped`, never silently kept.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    pub id: String,
    pub start_ms: u64,
    pub end_ms: u64,
    /// "Me", "Others", "Speaker 2" ... never an invented name.
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Point {
    pub text: String,
    pub evidence: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Topic {
    pub title: String,
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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ActionItem {
    pub action: String,
    pub owner: Option<String>,
    pub deadline: Option<String>,
    pub evidence: Vec<String>,
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
    /// What was removed or blanked, and why (a local debug log; never shown as content).
    pub dropped: Vec<String>,
}

fn norm(s: &str) -> String {
    s.to_lowercase()
        .replace(['\u{2019}', '\u{2018}'], "'")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Digits only, so "50,000" in the transcript matches "50000" or "₹50,000".
fn digits(s: &str) -> String {
    s.chars().filter(char::is_ascii_digit).collect()
}

struct Index<'a> {
    by_id: HashMap<&'a str, &'a Segment>,
}

impl<'a> Index<'a> {
    fn new(segments: &'a [Segment]) -> Self {
        Self {
            by_id: segments.iter().map(|s| (s.id.as_str(), s)).collect(),
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

    fn cited_text(&self, ids: &[String]) -> String {
        ids.iter()
            .filter_map(|id| self.by_id.get(id.as_str()))
            .map(|s| norm(&s.text))
            .collect::<Vec<_>>()
            .join(" \n ")
    }

    fn cited_mentions(&self, ids: &[String], phrase: &str) -> bool {
        let p = norm(phrase);
        !p.is_empty() && self.cited_text(ids).contains(&p)
    }

    fn cited_has_number(&self, ids: &[String], value: &str) -> bool {
        let want = digits(value);
        if want.is_empty() {
            // Spelled-out values ("three interviews") must appear as written.
            return self.cited_mentions(ids, value);
        }
        // "3" is supported by "Three interviews are pending."
        if crate::llm::spoken_numbers(&self.cited_text(ids)).contains(&want) {
            return true;
        }
        let text = self.cited_text(ids);
        // Compare digit runs, ignoring thousands separators.
        let mut runs = Vec::new();
        let mut cur = String::new();
        let chars: Vec<char> = text.chars().collect();
        for (i, c) in chars.iter().enumerate() {
            if c.is_ascii_digit() {
                cur.push(*c);
            } else if *c == ','
                && !cur.is_empty()
                && chars.get(i + 1).is_some_and(char::is_ascii_digit)
            {
                continue;
            } else if !cur.is_empty() {
                runs.push(std::mem::take(&mut cur));
            }
        }
        if !cur.is_empty() {
            runs.push(cur);
        }
        runs.contains(&want)
    }
}

fn keep_points(
    index: &Index,
    section: &str,
    points: Vec<Point>,
    dropped: &mut Vec<String>,
) -> Vec<Point> {
    points
        .into_iter()
        .filter_map(|p| match index.evidence(&p.evidence) {
            Some(evidence) if !p.text.trim().is_empty() => Some(Point { evidence, ..p }),
            _ => {
                dropped.push(format!("{section}: no valid evidence for \"{}\"", p.text));
                None
            }
        })
        .collect()
}

/// With speakers instead of headphones, the microphone also records the other side, so the
/// same words appear as "Me" and as "Others". A "Me" segment that overlaps an "Others"
/// segment in time and shares most of its words is that echo, and is removed.
/// Returns how many segments were removed. Ids are renumbered afterwards.
pub fn drop_echo(segments: &mut Vec<Segment>) -> usize {
    fn words(t: &str) -> std::collections::HashSet<String> {
        t.split(|c: char| !c.is_alphanumeric())
            .filter(|w| w.len() > 1)
            .map(str::to_lowercase)
            .collect()
    }
    let others: Vec<(u64, u64, std::collections::HashSet<String>)> = segments
        .iter()
        .filter(|s| s.speaker == "Others")
        .map(|s| (s.start_ms, s.end_ms, words(&s.text)))
        .collect();
    let before = segments.len();
    segments.retain(|s| {
        if s.speaker != "Me" {
            return true;
        }
        let mine = words(&s.text);
        if mine.is_empty() {
            return true;
        }
        let echo = others.iter().any(|(start, end, theirs)| {
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
JSON shape (all keys required; use [] or null when empty):
{"overview":{"title":str,"date":str|null,"duration_min":num|null,"participants":[str],"purpose":str|null},
 "executive_summary":[{"text":str,"evidence":[id]}],
 "topics":[{"title":str,"points":[{"text":str,"evidence":[id]}]}],
 "decisions":[{"text":str,"status":"confirmed"|"proposed","evidence":[id]}],
 "action_items":[{"action":str,"owner":str|null,"deadline":str|null,"evidence":[id]}],
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
    serde_json::json!({ "title": title, "segments": segs, "my_thoughts": notes }).to_string()
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

/// Plain Markdown notes, in the section order of the build plan, empty sections left out.
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
    if !o.participants.is_empty() {
        meta.push(format!("Participants: {}", o.participants.join(", ")));
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
                md.push_str(&format!("- {}{}\n", p.text, cite(&p.evidence)));
            }
            md.push('\n');
        }
    };
    points(&mut md, "Executive summary", &summary.executive_summary);
    if !summary.topics.is_empty() {
        md.push_str("## Key discussion points\n\n");
        for t in &summary.topics {
            md.push_str(&format!("### {}\n\n", t.title));
            for p in &t.points {
                md.push_str(&format!("- {}{}\n", p.text, cite(&p.evidence)));
            }
            md.push('\n');
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
                "- **{status}:** {}{}\n",
                d.text,
                cite(&d.evidence)
            ));
        }
        md.push('\n');
    }
    if !summary.action_items.is_empty() {
        md.push_str(
            "## Action items\n\n| Action | Owner | Deadline | Evidence |\n|---|---|---|---|\n",
        );
        for a in &summary.action_items {
            md.push_str(&format!(
                "| {} | {} | {} |{} |\n",
                a.action,
                a.owner.as_deref().unwrap_or("Not specified"),
                a.deadline.as_deref().unwrap_or("Not specified"),
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
            "## Final takeaway\n\n{}{}\n\n",
            t.text,
            cite(&t.evidence)
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

pub fn validate(summary: Summary, segments: &[Segment]) -> Validated {
    let index = Index::new(segments);
    let mut dropped = Vec::new();

    let executive_summary = keep_points(
        &index,
        "executive_summary",
        summary.executive_summary,
        &mut dropped,
    );
    let topics = summary
        .topics
        .into_iter()
        .filter_map(|t| {
            let points = keep_points(
                &index,
                &format!("topic \"{}\"", t.title),
                t.points,
                &mut dropped,
            );
            (!points.is_empty()).then_some(Topic { points, ..t })
        })
        .collect();
    let decisions = summary
        .decisions
        .into_iter()
        .filter_map(|d| match index.evidence(&d.evidence) {
            Some(evidence) => Some(Decision { evidence, ..d }),
            None => {
                dropped.push(format!("decision: no valid evidence for \"{}\"", d.text));
                None
            }
        })
        .collect();
    let action_items = summary
        .action_items
        .into_iter()
        .filter_map(|a| {
            let Some(evidence) = index.evidence(&a.evidence) else {
                dropped.push(format!("action: no valid evidence for \"{}\"", a.action));
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
            Some(ActionItem {
                owner,
                deadline,
                evidence,
                ..a
            })
        })
        .collect();
    let risks = keep_points(&index, "risks", summary.risks, &mut dropped);
    let open_questions = keep_points(
        &index,
        "open_questions",
        summary.open_questions,
        &mut dropped,
    );
    let numbers = summary
        .numbers
        .into_iter()
        .filter_map(|n| {
            let evidence = index.evidence(&n.evidence)?;
            if index.cited_has_number(&evidence, &n.value) {
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
    let next_steps = keep_points(&index, "next_steps", summary.next_steps, &mut dropped);
    let takeaway = summary.takeaway.and_then(|t| {
        let kept = keep_points(&index, "takeaway", vec![t], &mut dropped);
        kept.into_iter().next()
    });

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
            speaker: "Me".into(),
            text: t.to_string(),
        })
        .collect()
    }

    fn p(text: &str, ev: &[&str]) -> Point {
        Point {
            text: text.into(),
            evidence: ev.iter().map(|s| s.to_string()).collect(),
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
                Topic {
                    title: "Sales Performance".into(),
                    points: vec![p("Sales increased by 12%.", &["s2"])],
                },
                Topic {
                    title: "Marketing Budget".into(),
                    points: vec![p("Additional ₹50,000 requested.", &["s4"])],
                },
                Topic {
                    title: "Recruitment".into(),
                    points: vec![p("Three interviews are pending.", &["s6"])],
                },
                // An invented topic citing a segment that does not exist.
                Topic {
                    title: "Previous period".into(),
                    points: vec![p(
                        "Review performance against the previous period.",
                        &["s99"],
                    )],
                },
            ],
            decisions: vec![Decision {
                text: "Review everything next Monday.".into(),
                status: DecisionStatus::Confirmed,
                evidence: vec!["s7".into()],
            }],
            action_items: vec![ActionItem {
                action: "Review everything".into(),
                owner: Some("Rahul".into()),          // never said
                deadline: Some("next Monday".into()), // said
                evidence: vec!["s7".into()],
            }],
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
            s.action_items[0].owner, None,
            "an owner nobody named becomes Not specified"
        );
        assert_eq!(s.action_items[0].deadline.as_deref(), Some("next Monday"));
        assert_eq!(
            s.numbers
                .iter()
                .map(|n| n.value.as_str())
                .collect::<Vec<_>>(),
            ["12%", "₹50,000"]
        );
        assert_eq!(v.dropped.len(), 3, "{:#?}", v.dropped);
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
            seg("Others", 0, "Sales increased by 12 percent this quarter."),
            seg("Me", 500, "Sales increased by twelve percent this quarter."), // echo through the speakers
            seg("Me", 9000, "Great, then let us hire two more people."),
            seg("Others", 60000, "Agreed."),
            seg(
                "Me",
                200000,
                "Sales increased this quarter, I said that last week too.",
            ), // far away in time
        ];
        assert_eq!(drop_echo(&mut s), 1);
        assert_eq!(s.iter().filter(|x| x.speaker == "Me").count(), 2);
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
                    p("Budget may not be approved", &["s4"]),
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
                next_steps: vec![p("Review on Monday", &["s7", "s404"])],
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
        assert!(prompt.contains("\"action_items\""));
        let segs = golden_segments();
        let req = summary_request("Weekly", &segs, &[(5000, "ask about budget".into())]);
        assert!(req.contains("\"id\":\"s4\"") && req.contains("ask about budget"));

        let s = Summary {
            overview: Overview {
                title: "Weekly".into(),
                ..Default::default()
            },
            action_items: vec![ActionItem {
                action: "Review".into(),
                owner: None,
                deadline: Some("next Monday".into()),
                evidence: vec!["s7".into()],
            }],
            ..Default::default()
        };
        let md = to_markdown(&s, &segs, &[(5000, "ask about budget".into())]);
        assert!(
            md.contains("| Review | Not specified | next Monday | [00:00:24] |"),
            "{md}"
        );
        assert!(md.contains("- [00:00:05] ask about budget"));
        assert!(md.contains("[00:00:04] **Me:** Sales increased by 12 percent."));
        assert!(!md.contains("## Risks"), "empty sections are left out");
        assert!(parse_summary("```json\n{\"overview\":{\"title\":\"x\",\"date\":null,\"duration_min\":null,\"participants\":[],\"purpose\":null},\"takeaway\":null}\n```").is_ok());
    }

    #[test]
    fn json_from_the_schema_parses() {
        let json = r#"{"overview":{"title":"T","date":null,"duration_min":12.5,"participants":["Me","Others"],"purpose":null},
          "executive_summary":[],"topics":[],"decisions":[{"text":"x","status":"proposed","evidence":["s1"]}],
          "action_items":[{"action":"a","owner":null,"deadline":null,"evidence":["s1"]}],
          "risks":[],"open_questions":[],"numbers":[],"next_steps":[],"takeaway":null}"#;
        let s: Summary = serde_json::from_str(json).unwrap();
        assert_eq!(s.decisions[0].status, DecisionStatus::Proposed);
        assert_eq!(s.overview.duration_min, Some(12.5));
    }
}

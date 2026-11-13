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

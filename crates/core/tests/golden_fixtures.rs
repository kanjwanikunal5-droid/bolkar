//! Runs every golden case in `/spec/fixtures/*.json` through the deterministic pipeline.
//! A must-pass case failing is always a test failure; overall pass rate must be >= 95%.

use std::{fs, path::PathBuf};

use app_core::dictionary::{Dictionary, Term};
use app_core::pipeline::{self, Level, Settings};
use app_core::snippets::{Snippet, SnippetSet};
use serde::Deserialize;

#[derive(Deserialize)]
struct Defaults {
    level: Level,
    spoken_commands: bool,
}

#[derive(Deserialize)]
struct Case {
    id: String,
    #[serde(default)]
    must_pass: bool,
    #[serde(default)]
    level: Option<Level>,
    #[serde(default)]
    spoken_commands: Option<bool>,
    #[serde(default)]
    dictionary: Vec<Term>,
    #[serde(default)]
    snippets: Vec<Snippet>,
    input: String,
    expected: String,
}

#[derive(Deserialize)]
struct FixtureFile {
    defaults: Defaults,
    cases: Vec<Case>,
}

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../spec/fixtures")
}

#[test]
fn golden_cleanup_fixtures() {
    let mut total = 0usize;
    let mut failures: Vec<String> = Vec::new();
    let mut must_pass_failures = 0usize;
    let mut ids = std::collections::HashSet::new();

    let mut files: Vec<PathBuf> = fs::read_dir(fixture_dir())
        .expect("spec/fixtures must exist")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "json"))
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no fixture files found");

    for file in files {
        let raw = fs::read_to_string(&file).unwrap();
        let fixtures: FixtureFile = serde_json::from_str(&raw)
            .unwrap_or_else(|e| panic!("{}: invalid fixture JSON: {e}", file.display()));

        for case in fixtures.cases {
            assert!(
                ids.insert(case.id.clone()),
                "duplicate fixture id {}",
                case.id
            );
            total += 1;
            let settings = Settings {
                level: case.level.unwrap_or(fixtures.defaults.level),
                spoken_commands: case
                    .spoken_commands
                    .unwrap_or(fixtures.defaults.spoken_commands),
            };
            let out = pipeline::run(
                &case.input,
                &settings,
                &Dictionary::new(case.dictionary),
                &SnippetSet::new(case.snippets),
            );
            if out.text != case.expected {
                if case.must_pass {
                    must_pass_failures += 1;
                }
                failures.push(format!(
                    "[{}]{}\n   input:    {:?}\n   expected: {:?}\n   got:      {:?}",
                    case.id,
                    if case.must_pass { " MUST-PASS" } else { "" },
                    case.input,
                    case.expected,
                    out.text
                ));
            }
        }
    }

    let passed = total - failures.len();
    let rate = passed as f64 / total as f64 * 100.0;
    println!("golden fixtures: {passed}/{total} passed ({rate:.1}%)");
    for f in &failures {
        println!("FAILED {f}");
    }
    assert_eq!(must_pass_failures, 0, "a must-pass golden example failed");
    assert!(
        rate >= 95.0,
        "golden fixture pass rate {rate:.1}% is below 95%"
    );
}

#[test]
fn pipeline_never_changes_already_clean_text() {
    let clean = [
        "Please send the Q3 variance analysis by Friday.",
        "The EBITDA margin improved from 18.5% to 21%.",
        "Can we move the review to 4:30 PM on 12 November?",
        "कल सुबह 10 बजे मीटिंग है।",
        "நாளை காலை கூட்டம் உள்ளது.",
    ];
    let (settings, dict, snippets) = (
        Settings::default(),
        Dictionary::default(),
        SnippetSet::default(),
    );
    for s in clean {
        assert_eq!(pipeline::run(s, &settings, &dict, &snippets).text, s);
    }
}

#[test]
fn pipeline_is_idempotent_on_its_own_output() {
    let inputs = [
        "So, um, I think, you know, we should probably review the budget again.",
        "Let's meet at 2 PM... actually make that 3 PM",
        "We need three things. First, the budget. Second, the timeline. Third, the team.",
    ];
    let (settings, dict, snippets) = (
        Settings::default(),
        Dictionary::default(),
        SnippetSet::default(),
    );
    for s in inputs {
        let once = pipeline::run(s, &settings, &dict, &snippets).text;
        let twice = pipeline::run(&once, &settings, &dict, &snippets).text;
        assert_eq!(once, twice);
    }
}

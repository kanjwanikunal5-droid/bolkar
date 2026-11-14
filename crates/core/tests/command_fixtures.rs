//! Runs every case in `/spec/commands/cases.json` through Command Mode's offline commands.

use app_core::command::{self, Command};
use serde::Deserialize;

#[derive(Deserialize)]
struct Case {
    id: String,
    spoken: String,
    selection: String,
    expected_command: String,
    #[serde(default)]
    expected: Option<String>,
}

#[derive(Deserialize)]
struct File {
    cases: Vec<Case>,
}

fn name(c: &Command) -> &'static str {
    match c {
        Command::BulletList => "bullet_list",
        Command::NumberedList => "numbered_list",
        Command::Uppercase => "uppercase",
        Command::Lowercase => "lowercase",
        Command::TitleCase => "title_case",
        Command::SentenceCase => "sentence_case",
        Command::CleanUp => "clean_up",
        Command::OneParagraph => "one_paragraph",
        Command::Ai(_) => "ai",
    }
}

#[test]
fn command_fixtures() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../spec/commands/cases.json"
    );
    let file: File = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let mut failures = Vec::new();
    for c in &file.cases {
        let cmd = command::parse(&c.spoken);
        if name(&cmd) != c.expected_command {
            failures.push(format!(
                "[{}] read as {} not {}",
                c.id,
                name(&cmd),
                c.expected_command
            ));
            continue;
        }
        let got = command::apply(&cmd, &c.selection).map(|e| e.text);
        if got != c.expected {
            failures.push(format!(
                "[{}] expected {:?} got {:?}",
                c.id, c.expected, got
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

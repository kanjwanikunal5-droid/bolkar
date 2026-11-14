//! Runs every case in `/spec/hinglish/spelling.json` through the offline Hinglish rules.
//! Every case must pass: the rules are a fixed table, not a model.

use serde::Deserialize;

#[derive(Deserialize)]
struct Case {
    id: String,
    input: String,
    expected: String,
}

#[derive(Deserialize)]
struct File {
    cases: Vec<Case>,
}

#[test]
fn hinglish_spelling_fixtures() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../spec/hinglish/spelling.json"
    );
    let file: File = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert!(file.cases.len() >= 10);
    let failures: Vec<String> = file
        .cases
        .iter()
        .filter_map(|c| {
            let got = app_core::hinglish::normalise(&c.input);
            (got != c.expected)
                .then(|| format!("[{}] expected {:?} got {:?}", c.id, c.expected, got))
        })
        .collect();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

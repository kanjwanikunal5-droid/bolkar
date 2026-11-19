//! Tries the smart-layout step on a file of dictations (developer tool).
//!
//!   layout-eval <model> <texts.txt>
//!
//! One dictation per line. Each is sent to Groq with the app's layout prompt, then checked
//! exactly as the app does (`app_core::layout::accept_layout`). Prints whether the layout was
//! accepted and the laid-out text. The key is read from Windows Credential Manager. Requests
//! are paced to stay inside the free tier's per-minute limits.

use std::time::Duration;

use app_core::layout;
use app_desktop_lib::{credentials, providers};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (model, path) = (&args[0], &args[1]);
    let key = credentials::load("groq").expect("no Groq key in Credential Manager");
    let preset = providers::preset("groq").expect("groq preset");
    let cfg = providers::Config {
        provider: "groq".into(),
        base_url: preset.base_url.to_string(),
        model: model.clone(),
        key: Some(key),
    };
    if args.get(2).map(String::as_str) == Some("--burst") {
        burst(&cfg, path);
        return;
    }
    let texts = std::fs::read_to_string(path).expect("texts");
    let prompt = layout::layout_prompt();
    let (mut accepted, mut total) = (0, 0);
    for (n, text) in texts
        .lines()
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .enumerate()
    {
        if n > 0 {
            std::thread::sleep(Duration::from_secs(6));
        }
        total += 1;
        if !layout::worth_laying_out(text) {
            println!("== {n}: too short for layout\n");
            continue;
        }
        let user = serde_json::json!({ "text": text }).to_string();
        let result = providers::chat(&cfg, &prompt, &user, Duration::from_secs(8))
            .map_err(|e| e.to_string())
            .and_then(|a| {
                println!("   ({} ms)", a.elapsed.as_millis());
                layout::parse_layout(&a.content)
            })
            .and_then(|laid| layout::accept_layout(text, &laid));
        match result {
            Ok(laid) => {
                accepted += 1;
                println!("== {n}: ACCEPTED\n{laid}\n");
            }
            Err(why) => println!("== {n}: kept as spoken ({why})\n"),
        }
    }
    println!("{model}: {accepted} of {total} laid out");
}

/// Two requests back to back (a clean-up-sized one, then the layout), printing the rate-limit
/// headers and any refusal, with and without an output cap. Shows which limit is hit.
fn burst(cfg: &providers::Config, path: &str) {
    let text = std::fs::read_to_string(path).expect("texts");
    let text = text.lines().next().unwrap_or_default();
    let small_system = layout::layout_prompt();
    let big_system = small_system.repeat(4);
    let client = reqwest::blocking::Client::new();
    let url = format!("{}/chat/completions", cfg.base_url.trim_end_matches('/'));
    for cap in [None, Some(800)] {
        for (name, system) in [
            ("clean-up sized", big_system.as_str()),
            ("layout", small_system.as_str()),
        ] {
            let mut body = serde_json::json!({
                "model": cfg.model,
                "temperature": 0,
                "messages": [
                    { "role": "system", "content": system },
                    { "role": "user", "content": serde_json::json!({ "text": text }).to_string() },
                ],
            });
            if cfg.model.contains("qwen3") {
                body["reasoning_effort"] = serde_json::json!("none");
                body["reasoning_format"] = serde_json::json!("hidden");
            }
            if let Some(cap) = cap {
                body["max_completion_tokens"] = serde_json::json!(cap);
            }
            let r = client
                .post(&url)
                .bearer_auth(cfg.key.as_deref().unwrap_or_default())
                .header("Content-Type", "application/json")
                .body(body.to_string())
                .send()
                .expect("send");
            let status = r.status().as_u16();
            let h = |k: &str| {
                r.headers()
                    .get(k)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("-")
                    .to_string()
            };
            let line = format!(
                "cap {cap:?} {name}: {status} | tokens left {} of {} | requests left {} | retry-after {}",
                h("x-ratelimit-remaining-tokens"),
                h("x-ratelimit-limit-tokens"),
                h("x-ratelimit-remaining-requests"),
                h("retry-after")
            );
            let body = r.text().unwrap_or_default();
            println!("{line}");
            if status != 200 {
                // Drop the organisation id before printing.
                let msg: String = body
                    .split_whitespace()
                    .filter(|w| !w.starts_with("`org_"))
                    .collect::<Vec<_>>()
                    .join(" ");
                println!("   {}", msg.chars().take(400).collect::<String>());
            }
        }
        std::thread::sleep(Duration::from_secs(65));
    }
}

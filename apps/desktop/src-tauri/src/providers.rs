//! AI clean-up providers. All of them speak the OpenAI-compatible "chat completions" API,
//! so one client covers Groq, Gemini, a local Ollama and any compatible server.

use std::time::{Duration, Instant};

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize)]
pub struct Preset {
    pub id: &'static str,
    pub name: &'static str,
    pub base_url: &'static str,
    pub model: &'static str,
    pub needs_key: bool,
    pub key_url: &'static str,
    /// Shown on the consent screen. Only statements we can stand behind.
    pub data_note: &'static str,
}

pub const PRESETS: &[Preset] = &[
    Preset {
        id: "groq",
        name: "Groq (free tier, your own key)",
        base_url: "https://api.groq.com/openai/v1",
        model: "qwen/qwen3.8-27b",
        needs_key: true,
        key_url: "https://console.groq.com/keys",
        data_note: "The text of each dictation is sent to Groq's servers to be cleaned up. Free \
                    accounts have daily limits. Read Groq's privacy policy before agreeing.",
    },
    Preset {
        id: "gemini",
        name: "Google Gemini (free tier, your own key)",
        base_url: "https://generativelanguage.googleapis.com/v1beta/openai",
        model: "gemini-2.5-flash",
        needs_key: true,
        key_url: "https://aistudio.google.com/apikey",
        data_note: "The text of each dictation is sent to Google. On the free tier, Google uses \
                    this content to improve its products.",
    },
    Preset {
        id: "ollama",
        name: "Ollama on this computer",
        base_url: "http://localhost:11434/v1",
        model: "qwen2.5:3b",
        needs_key: false,
        key_url: "https://ollama.com",
        data_note: "Runs on this computer. Nothing leaves it. Needs Ollama installed and the \
                    model downloaded; it can be slow without a graphics card.",
    },
    Preset {
        id: "custom",
        name: "Other OpenAI-compatible server",
        base_url: "",
        model: "",
        needs_key: true,
        key_url: "",
        data_note: "The text of each dictation is sent to the server you enter. Check that \
                    server's data policy before agreeing.",
    },
];

pub fn preset(id: &str) -> Option<&'static Preset> {
    PRESETS.iter().find(|p| p.id == id)
}

#[derive(Debug, Clone)]
pub struct Config {
    pub provider: String,
    pub base_url: String,
    pub model: String,
    pub key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum CallError {
    Timeout,
    RateLimited,
    BadKey,
    Http(u16, String),
    Network(String),
    BadAnswer(String),
}

impl std::fmt::Display for CallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Timeout => write!(f, "no answer in time"),
            Self::RateLimited => write!(f, "free limit reached for now (rate limited)"),
            Self::BadKey => write!(f, "the API key was refused"),
            Self::Http(code, body) => write!(f, "server error {code}: {body}"),
            Self::Network(e) => write!(f, "network problem: {e}"),
            Self::BadAnswer(e) => write!(f, "unexpected answer: {e}"),
        }
    }
}

/// Speech-to-text in the cloud (OpenAI-compatible `POST /audio/transcriptions`).
/// `wav` is a complete WAV file. Returns the text and the time taken.
pub fn transcribe(
    cfg: &Config,
    wav: Vec<u8>,
    language: Option<&str>,
    prompt: Option<&str>,
    timeout: Duration,
) -> Result<(String, Duration), CallError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| CallError::Network(e.to_string()))?;
    let file = reqwest::blocking::multipart::Part::bytes(wav)
        .file_name("dictation.wav")
        .mime_str("audio/wav")
        .map_err(|e| CallError::Network(e.to_string()))?;
    let mut form = reqwest::blocking::multipart::Form::new()
        .part("file", file)
        .text("model", cfg.model.clone())
        .text("temperature", "0")
        .text("response_format", "json");
    if let Some(l) = language {
        form = form.text("language", l.to_string());
    }
    if let Some(p) = prompt.filter(|p| !p.is_empty()) {
        form = form.text("prompt", p.to_string());
    }
    let mut request = client
        .post(format!(
            "{}/audio/transcriptions",
            cfg.base_url.trim_end_matches('/')
        ))
        .multipart(form);
    if let Some(key) = cfg.key.as_deref().filter(|k| !k.is_empty()) {
        request = request.bearer_auth(key);
    }
    let started = Instant::now();
    let response = request.send().map_err(|e| {
        if e.is_timeout() {
            CallError::Timeout
        } else {
            CallError::Network(e.to_string())
        }
    })?;
    let status = response.status().as_u16();
    let text = response.text().map_err(|e| {
        if e.is_timeout() {
            CallError::Timeout
        } else {
            CallError::Network(e.to_string())
        }
    })?;
    match status {
        200..=299 => {}
        401 | 403 => return Err(CallError::BadKey),
        429 => return Err(CallError::RateLimited),
        _ => return Err(CallError::Http(status, text.chars().take(200).collect())),
    }
    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| CallError::BadAnswer(e.to_string()))?;
    let out = json["text"]
        .as_str()
        .ok_or_else(|| CallError::BadAnswer("no text".into()))?
        .trim()
        .to_string();
    Ok((out, started.elapsed()))
}

/// Encodes 16 kHz mono samples as a 16-bit WAV file in memory.
pub fn wav_bytes(samples: &[f32]) -> Vec<u8> {
    let data_len = (samples.len() * 2) as u32;
    let mut v = Vec::with_capacity(44 + data_len as usize);
    v.extend_from_slice(b"RIFF");
    v.extend_from_slice(&(36 + data_len).to_le_bytes());
    v.extend_from_slice(b"WAVEfmt ");
    v.extend_from_slice(&16u32.to_le_bytes());
    v.extend_from_slice(&1u16.to_le_bytes()); // PCM
    v.extend_from_slice(&1u16.to_le_bytes()); // mono
    v.extend_from_slice(&16_000u32.to_le_bytes());
    v.extend_from_slice(&32_000u32.to_le_bytes()); // byte rate
    v.extend_from_slice(&2u16.to_le_bytes()); // block align
    v.extend_from_slice(&16u16.to_le_bytes()); // bits
    v.extend_from_slice(b"data");
    v.extend_from_slice(&data_len.to_le_bytes());
    for s in samples {
        v.extend_from_slice(&((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes());
    }
    v
}

/// Model ids the provider currently offers to this key (OpenAI-compatible `GET /models`).
pub fn list_models(cfg: &Config, timeout: Duration) -> Result<Vec<String>, CallError> {
    let client = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| CallError::Network(e.to_string()))?;
    let mut request = client.get(format!("{}/models", cfg.base_url.trim_end_matches('/')));
    if let Some(key) = cfg.key.as_deref().filter(|k| !k.is_empty()) {
        request = request.bearer_auth(key);
    }
    let response = request.send().map_err(|e| {
        if e.is_timeout() {
            CallError::Timeout
        } else {
            CallError::Network(e.to_string())
        }
    })?;
    let status = response.status().as_u16();
    let text = response
        .text()
        .map_err(|e| CallError::Network(e.to_string()))?;
    match status {
        200..=299 => {}
        401 | 403 => return Err(CallError::BadKey),
        429 => return Err(CallError::RateLimited),
        _ => return Err(CallError::Http(status, text.chars().take(200).collect())),
    }
    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| CallError::BadAnswer(e.to_string()))?;
    let mut ids: Vec<String> = json["data"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|m| m["id"].as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    ids.sort();
    Ok(ids)
}

/// Text models suitable for clean-up, in order of preference, among those offered.
/// Speech, guard and image models are skipped.
pub fn pick_cleanup_model(available: &[String], preferred: &[&str]) -> Option<String> {
    let usable = |id: &str| {
        let l = id.to_lowercase();
        !(l.contains("whisper")
            || l.contains("guard")
            || l.contains("tts")
            || l.contains("vision")
            || l.contains("embed")
            || l.contains("orpheus")
            || l.contains("playai"))
    };
    preferred
        .iter()
        .find(|p| available.iter().any(|a| a == *p))
        .map(|p| p.to_string())
        .or_else(|| available.iter().find(|a| usable(a)).cloned())
}

pub struct Answer {
    pub content: String,
    pub elapsed: Duration,
}

/// One chat-completion call. `timeout` covers the whole request.
pub fn chat(
    cfg: &Config,
    system: &str,
    user: &str,
    timeout: Duration,
) -> Result<Answer, CallError> {
    let mut body = serde_json::json!({
        "model": cfg.model,
        "temperature": 0,
        "messages": [
            { "role": "system", "content": system },
            { "role": "user", "content": user },
        ],
    });
    if cfg.model.starts_with("openai/gpt-oss") {
        // Reasoning models: keep thinking short so the answer comes back quickly.
        body["reasoning_effort"] = serde_json::json!("low");
    }
    if cfg.model.contains("qwen3") {
        // Qwen 3 "thinks" before answering by default; switch that off for speed and keep
        // any reasoning out of the answer text.
        body["reasoning_effort"] = serde_json::json!("none");
        body["reasoning_format"] = serde_json::json!("hidden");
    }
    let payload = body.to_string();
    let client = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|e| CallError::Network(e.to_string()))?;
    let url = format!("{}/chat/completions", cfg.base_url.trim_end_matches('/'));
    let mut request = client
        .post(url)
        .header("Content-Type", "application/json")
        .body(payload.clone());
    if let Some(key) = cfg.key.as_deref().filter(|k| !k.is_empty()) {
        request = request.bearer_auth(key);
    }
    let started = Instant::now();
    let response = request.send().map_err(|e| {
        if e.is_timeout() {
            CallError::Timeout
        } else {
            CallError::Network(e.to_string())
        }
    })?;
    let status = response.status().as_u16();
    let text = response.text().map_err(|e| {
        if e.is_timeout() {
            CallError::Timeout
        } else {
            CallError::Network(e.to_string())
        }
    })?;
    match status {
        200..=299 => {}
        401 | 403 => return Err(CallError::BadKey),
        429 => return Err(CallError::RateLimited),
        _ => {
            // Never echo anything that could contain the key; provider errors do not.
            let short: String = text.chars().take(200).collect();
            return Err(CallError::Http(status, short));
        }
    }
    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| CallError::BadAnswer(e.to_string()))?;
    let content = json["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| CallError::BadAnswer("no message content".into()))?
        .to_string();
    Ok(Answer {
        content,
        elapsed: started.elapsed(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    /// A one-request fake server on localhost. Returns its base URL and a handle that yields
    /// the raw request it received.
    fn fake_server(
        status: u16,
        body: &'static str,
        delay_ms: u64,
    ) -> (String, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/v1", listener.local_addr().unwrap());
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_millis(500)))
                .unwrap();
            let mut buf = vec![0u8; 65536];
            let mut request = Vec::new();
            loop {
                match stream.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        request.extend_from_slice(&buf[..n]);
                        let s = String::from_utf8_lossy(&request);
                        if let Some((head, rest)) = s.split_once("\r\n\r\n") {
                            let len = head
                                .lines()
                                .find_map(|l| {
                                    l.to_lowercase()
                                        .strip_prefix("content-length:")
                                        .map(|v| v.trim().parse::<usize>().unwrap_or(0))
                                })
                                .unwrap_or(0);
                            if rest.len() >= len {
                                break;
                            }
                        }
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(delay_ms));
            let reply = format!(
                "HTTP/1.1 {status} X\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                body.len()
            );
            let _ = stream.write_all(reply.as_bytes());
            String::from_utf8_lossy(&request).into_owned()
        });
        (url, handle)
    }

    fn cfg(url: String) -> Config {
        Config {
            provider: "custom".into(),
            base_url: url,
            model: "test-model".into(),
            key: Some("sk-test".into()),
        }
    }

    #[test]
    fn sends_a_chat_request_and_reads_the_answer() {
        let (url, server) = fake_server(
            200,
            r#"{"choices":[{"message":{"role":"assistant","content":"{\"text\":\"Hello.\"}"}}]}"#,
            0,
        );
        let answer = chat(
            &cfg(url),
            "sys",
            "{\"raw\":\"um hello\"}",
            Duration::from_secs(5),
        )
        .unwrap();
        assert_eq!(answer.content, "{\"text\":\"Hello.\"}");
        let request = server.join().unwrap();
        assert!(request.starts_with("POST /v1/chat/completions"));
        assert!(
            request.contains("authorization: Bearer sk-test")
                || request.contains("Authorization: Bearer sk-test")
        );
        assert!(request.contains("\"model\":\"test-model\""));
        assert!(request.contains("um hello"));
    }

    #[test]
    fn maps_errors() {
        let (url, s) = fake_server(429, r#"{"error":"rate"}"#, 0);
        assert_eq!(
            chat(&cfg(url), "s", "u", Duration::from_secs(5)).err(),
            Some(CallError::RateLimited)
        );
        s.join().unwrap();
        let (url, s) = fake_server(401, r#"{"error":"key"}"#, 0);
        assert_eq!(
            chat(&cfg(url), "s", "u", Duration::from_secs(5)).err(),
            Some(CallError::BadKey)
        );
        s.join().unwrap();
        let (url, s) = fake_server(200, r#"{"nothing":true}"#, 0);
        assert!(matches!(
            chat(&cfg(url), "s", "u", Duration::from_secs(5)),
            Err(CallError::BadAnswer(_))
        ));
        s.join().unwrap();
    }

    #[test]
    fn slow_servers_time_out() {
        let (url, s) = fake_server(200, r#"{"choices":[]}"#, 1500);
        let started = Instant::now();
        assert_eq!(
            chat(&cfg(url), "s", "u", Duration::from_millis(400)).err(),
            Some(CallError::Timeout)
        );
        assert!(started.elapsed() < Duration::from_millis(1200));
        let _ = s.join();
    }

    #[test]
    fn nobody_listening_is_a_network_error() {
        // Port 9 on localhost: nothing listens there.
        let c = cfg("http://127.0.0.1:9/v1".into());
        assert!(matches!(
            chat(&c, "s", "u", Duration::from_secs(3)),
            Err(CallError::Network(_)) | Err(CallError::Timeout)
        ));
    }

    #[test]
    fn presets_are_complete() {
        for p in PRESETS {
            assert!(!p.data_note.is_empty());
            if p.id != "custom" {
                assert!(p.base_url.starts_with("http"));
                assert!(!p.model.is_empty());
            }
        }
        assert!(preset("gemini")
            .unwrap()
            .data_note
            .contains("improve its products"));
    }
}

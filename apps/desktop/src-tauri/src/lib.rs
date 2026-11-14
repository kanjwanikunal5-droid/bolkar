//! Desktop app: system-wide dictation for Windows.

mod audio;
mod controller;
pub mod credentials;
mod db;
mod hotkey;
mod insert;
mod meeting;
mod models;
mod overlay;
pub mod providers;

use std::path::PathBuf;
use std::sync::mpsc::{channel, Sender};
use std::sync::Mutex;

use rusqlite::Connection;
use serde::Serialize;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, State, WindowEvent};

use controller::{DictationResult, Msg};

pub struct AppState {
    pub db: Mutex<Connection>,
    pub data_dir: PathBuf,
    pub ctl: Mutex<Sender<Msg>>,
    pub hook_error: Mutex<Option<String>>,
    pub recorder: audio::Recorder,
    pub meeting: Mutex<Option<(meeting::Recording, String)>>,
}

#[tauri::command]
fn start_meeting(
    state: State<AppState>,
    mic: bool,
    system: bool,
    consent: bool,
) -> Result<(), String> {
    if !consent {
        return Err("Please confirm that the participants know they are being recorded.".into());
    }
    let mut slot = state.meeting.lock().map_err(|e| e.to_string())?;
    if slot.is_some() {
        return Err("A meeting is already being recorded.".into());
    }
    let rec = meeting::Recording::start(&state.data_dir.join("meetings"), mic, system)?;
    *slot = Some((rec, chrono::Utc::now().to_rfc3339()));
    Ok(())
}

#[tauri::command]
fn meeting_status(state: State<AppState>) -> Option<meeting::LiveStatus> {
    state.meeting.lock().ok()?.as_ref().map(|(r, _)| r.status())
}

#[derive(Serialize)]
struct MeetingResult {
    id: String,
    segments: Vec<app_core::summary::Segment>,
    summary: Option<app_core::summary::Summary>,
    /// Items the evidence check removed, for transparency.
    dropped: Vec<String>,
    markdown: String,
    /// Why there is no summary, when there is none.
    note: Option<String>,
}

/// Stops the recording, transcribes both tracks on this computer, and (if AI clean-up is set
/// up) writes evidence-checked notes.
#[tauri::command]
async fn stop_meeting(
    state: State<'_, AppState>,
    title: String,
    thoughts: Vec<(u64, String)>,
) -> Result<MeetingResult, String> {
    let (rec, started_at) = state
        .meeting
        .lock()
        .map_err(|e| e.to_string())?
        .take()
        .ok_or("No meeting is being recorded.")?;
    let finished = tauri::async_runtime::spawn_blocking(move || rec.stop())
        .await
        .map_err(|e| e.to_string())??;

    let mut tracks = Vec::new();
    if let Some(p) = &finished.mic_path {
        tracks.push((PathBuf::from(p), "Me".to_string()));
    }
    if let Some(p) = &finished.system_path {
        tracks.push((PathBuf::from(p), "Others".to_string()));
    }
    let (reply, rx) = channel();
    state
        .ctl
        .lock()
        .map_err(|e| e.to_string())?
        .send(Msg::TranscribeMeeting { tracks, reply })
        .map_err(|e| e.to_string())?;
    let segments =
        tauri::async_runtime::spawn_blocking(move || rx.recv().map_err(|e| e.to_string())?)
            .await
            .map_err(|e| e.to_string())??;

    let title = if title.trim().is_empty() {
        "Meeting".to_string()
    } else {
        title.trim().to_string()
    };
    let mut summary = None;
    let mut dropped = Vec::new();
    let mut note = None;
    let mut provider = String::new();
    if segments.is_empty() {
        note = Some("Nothing was heard in the recording.".into());
    } else if let Some(cfg) = ai_config(&state) {
        provider = cfg.provider.clone();
        let system = app_core::summary::summary_prompt(
            "the language of the meeting",
            "the script used in the transcript",
        );
        let user = app_core::summary::summary_request(&title, &segments, &thoughts);
        let bytes = system.len() + user.len();
        let segs = segments.clone();
        // Notes are long, so allow far more time than for a dictation.
        let answer = tauri::async_runtime::spawn_blocking(move || {
            providers::chat(&cfg, &system, &user, std::time::Duration::from_secs(90))
        })
        .await
        .map_err(|e| e.to_string())?;
        if let Ok(conn) = state.db.lock() {
            let outcome = answer
                .as_ref()
                .map(|_| "ok".to_string())
                .unwrap_or_else(|e| e.to_string());
            db::log_event(
                &conn,
                "upload",
                &format!("{provider}: meeting transcript for notes, {bytes} bytes, {outcome}"),
            );
        }
        match answer
            .map_err(|e| e.to_string())
            .and_then(|a| app_core::summary::parse_summary(&a.content))
        {
            Ok(raw) => {
                let mut v = app_core::summary::validate(raw, &segs);
                v.summary.overview.title = title.clone();
                v.summary.overview.duration_min =
                    Some((finished.duration_ms as f64 / 60000.0 * 10.0).round() / 10.0);
                dropped = v.dropped;
                summary = Some(v.summary);
            }
            Err(e) => {
                note = Some(format!(
                    "Notes could not be written ({e}). The transcript is below."
                ))
            }
        }
    } else {
        note = Some(
            "Turn on AI clean-up in Settings to get organised notes. The transcript is below."
                .into(),
        );
    }

    let empty = app_core::summary::Summary {
        overview: app_core::summary::Overview {
            title: title.clone(),
            ..Default::default()
        },
        ..Default::default()
    };
    let markdown =
        app_core::summary::to_markdown(summary.as_ref().unwrap_or(&empty), &segments, &thoughts);
    if let Ok(conn) = state.db.lock() {
        let json = summary.as_ref().and_then(|s| serde_json::to_string(s).ok());
        db::save_meeting(
            &conn,
            &finished.id,
            &title,
            &started_at,
            finished.duration_ms,
            finished.mic_path.as_deref(),
            finished.system_path.as_deref(),
            &segments,
            &thoughts,
            json.as_deref(),
            &provider,
        )?;
    }
    Ok(MeetingResult {
        id: finished.id,
        segments,
        summary,
        dropped,
        markdown,
        note,
    })
}

#[derive(Serialize)]
struct MicStatus {
    /// Microphone stream open right now.
    open: bool,
    /// Latest input level, 0..1.
    level: f32,
}

/// Windows 11 runs background apps in "efficiency mode", which slows the processor work
/// around speech recognition. A dictation app is in the background by design (the user is in
/// another app), so it opts out.
fn disable_power_throttling() {
    use windows::Win32::System::Threading::{
        GetCurrentProcess, ProcessPowerThrottling, SetProcessInformation,
        PROCESS_POWER_THROTTLING_CURRENT_VERSION, PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
        PROCESS_POWER_THROTTLING_STATE,
    };
    let state = PROCESS_POWER_THROTTLING_STATE {
        Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
        ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
        StateMask: 0, // 0 = do not throttle
    };
    unsafe {
        let _ = SetProcessInformation(
            GetCurrentProcess(),
            ProcessPowerThrottling,
            &state as *const _ as *const _,
            std::mem::size_of::<PROCESS_POWER_THROTTLING_STATE>() as u32,
        );
    }
}

/// The pill's buttons: "cancel" (âœ•) or "finish" (âœ“).
#[tauri::command]
fn pill_action(state: State<AppState>, action: String) -> Result<(), String> {
    let action = match action.as_str() {
        "cancel" => hotkey::Action::Cancel,
        "finish" => hotkey::Action::HandsFreeStop,
        _ => return Err("unknown pill action".into()),
    };
    // Keep the keyboard state machine in step with a stop that did not come from a key.
    hotkey::force_idle();
    state
        .ctl
        .lock()
        .map_err(|e| e.to_string())?
        .send(Msg::Key(action))
        .map_err(|e| e.to_string())
}

/// For the live microphone meter in Settings.
#[tauri::command]
fn mic_status(state: State<AppState>) -> MicStatus {
    MicStatus {
        open: state.recorder.is_open(),
        level: state.recorder.level(),
    }
}

impl AppState {
    pub fn models_dir(&self) -> PathBuf {
        self.data_dir.join("models")
    }
    pub fn audio_dir(&self) -> PathBuf {
        self.data_dir.join("audio")
    }
    pub fn setting(&self, key: &str) -> String {
        let stored = self.db.lock().ok().and_then(|c| db::get_setting(&c, key));
        stored.unwrap_or_else(|| default_setting(key).to_string())
    }
}

/// Settings the UI may read and write, with their defaults.
const SETTINGS: &[(&str, &str)] = &[
    (
        "model",
        if cfg!(feature = "gpu") {
            "large-v3-turbo"
        } else {
            "small"
        },
    ),
    ("language", "en-hinglish"),
    ("level", "LIGHT"),
    ("use_gpu", "true"),
    // On by default: on this laptop the microphone takes 0.6-1.8 s to start, so opening it
    // only at key-down cuts off the first words. Only the last 0.3 s is kept, in memory.
    ("keep_mic_open", "true"),
    ("spoken_commands", "true"),
    ("save_audio", "true"),
    ("audio_retention_days", "7"),
    ("ui_scale", "125"),
    // Scratchpad notes (Alt+Shift+N), kept on this computer.
    ("scratchpad", ""),
];

/// Keys set only through `set_ai` (never through the generic settings command).
const AI_PROVIDER: &str = "ai_provider";
const AI_MODEL: &str = "ai_model";
const AI_BASE_URL: &str = "ai_base_url";

/// The AI clean-up configuration, but only when it is fully usable: a provider chosen,
/// consent recorded, and (where needed) a key in Credential Manager.
pub fn ai_config(state: &AppState) -> Option<providers::Config> {
    let conn = state.db.lock().ok()?;
    let provider = db::get_setting(&conn, AI_PROVIDER).filter(|p| p != "none")?;
    let preset = providers::preset(&provider)?;
    if !db::has_consent(&conn, &provider) {
        return None;
    }
    let base_url = db::get_setting(&conn, AI_BASE_URL)
        .filter(|u| !u.is_empty())
        .unwrap_or_else(|| preset.base_url.to_string());
    let model = db::get_setting(&conn, AI_MODEL)
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| preset.model.to_string());
    let key = credentials::load(&provider);
    if (preset.needs_key && key.is_none()) || base_url.is_empty() || model.is_empty() {
        return None;
    }
    Some(providers::Config {
        provider,
        base_url,
        model,
        key,
    })
}

const CLOUD_ASR: &str = "cloud_asr";
/// Consent for sending audio is recorded separately from consent for sending text.
const CLOUD_ASR_CONSENT: &str = "groq-asr";
const CLOUD_ASR_MODEL: &str = "whisper-large-v3";
const CLOUD_ASR_NOTE: &str = "The audio of each dictation is sent to Groq for speech recognition. \
     Free accounts have daily limits (about 8 hours of audio per day). Read Groq's privacy policy \
     before agreeing.";
/// Clean-up models in order of preference (measured 13 Nov 2026: qwen3.8-27b answered in a
/// median 0.27 s, gpt-oss-20b in 0.70 s).
const PREFERRED_CLEANUP_MODELS: &[&str] = &[
    "qwen/qwen3.8-27b",
    "openai/gpt-oss-20b",
    "openai/gpt-oss-120b",
];

/// Cloud speech recognition, only when switched on, agreed to, and a Groq key exists.
pub fn asr_config(state: &AppState) -> Option<providers::Config> {
    let on = {
        let conn = state.db.lock().ok()?;
        db::get_setting(&conn, CLOUD_ASR).as_deref() == Some("on")
            && db::has_consent(&conn, CLOUD_ASR_CONSENT)
    };
    if !on {
        return None;
    }
    let preset = providers::preset("groq")?;
    Some(providers::Config {
        provider: "groq".into(),
        base_url: preset.base_url.to_string(),
        model: CLOUD_ASR_MODEL.into(),
        key: Some(credentials::load("groq")?),
    })
}

/// If the configured clean-up model is no longer offered, picks a current one, saves it and
/// returns it. Returns None when the model is fine or the list cannot be fetched.
pub fn heal_ai_model(state: &AppState, cfg: &providers::Config) -> Option<String> {
    let available = providers::list_models(cfg, std::time::Duration::from_secs(5)).ok()?;
    if available.contains(&cfg.model) {
        return None;
    }
    let pick = providers::pick_cleanup_model(&available, PREFERRED_CLEANUP_MODELS)?;
    let conn = state.db.lock().ok()?;
    db::set_setting(&conn, AI_MODEL, &pick).ok()?;
    db::log_event(
        &conn,
        "info",
        &format!(
            "clean-up model {} is no longer offered; switched to {pick}",
            cfg.model
        ),
    );
    Some(pick)
}

/// Turns cloud speech recognition on or off. Turning it on needs the separate consent.
#[tauri::command]
fn set_cloud_asr(state: State<AppState>, on: bool, consent: bool) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    if !on {
        db::revoke_consent(&conn, CLOUD_ASR_CONSENT)?;
        return db::set_setting(&conn, CLOUD_ASR, "off");
    }
    if !consent {
        return Err("Please tick the box to agree before sending audio.".into());
    }
    if credentials::load("groq").is_none() {
        return Err("Save your Groq key in AI clean-up first.".into());
    }
    db::record_consent(
        &conn,
        CLOUD_ASR_CONSENT,
        "https://api.groq.com/openai/v1",
        CLOUD_ASR_MODEL,
        CLOUD_ASR_NOTE,
    )?;
    db::set_setting(&conn, CLOUD_ASR, "on")
}

fn default_setting(key: &str) -> &'static str {
    SETTINGS
        .iter()
        .find(|(k, _)| *k == key)
        .map_or("", |(_, v)| v)
}

#[derive(Serialize)]
struct AiStatus {
    presets: &'static [providers::Preset],
    provider: String,
    model: String,
    base_url: String,
    has_key: bool,
    consent: bool,
    /// True when dictations are actually being sent for AI clean-up.
    active: bool,
    /// Cloud speech recognition is on (audio goes to Groq).
    cloud_asr: bool,
    /// A Groq key exists, so cloud speech recognition can be offered.
    cloud_asr_possible: bool,
    cloud_asr_note: &'static str,
}

#[tauri::command]
fn get_ai(state: State<AppState>) -> AiStatus {
    let (provider, model, base_url, consent) = {
        let conn = state.db.lock().expect("db");
        let provider = db::get_setting(&conn, AI_PROVIDER).unwrap_or_else(|| "none".into());
        let consent = db::has_consent(&conn, &provider);
        (
            provider,
            db::get_setting(&conn, AI_MODEL).unwrap_or_default(),
            db::get_setting(&conn, AI_BASE_URL).unwrap_or_default(),
            consent,
        )
    };
    AiStatus {
        presets: providers::PRESETS,
        has_key: provider != "none" && credentials::load(&provider).is_some(),
        active: ai_config(&state).is_some(),
        cloud_asr: asr_config(&state).is_some(),
        cloud_asr_possible: credentials::load("groq").is_some(),
        cloud_asr_note: CLOUD_ASR_NOTE,
        provider,
        model,
        base_url,
        consent,
    }
}

/// Saves the AI clean-up choice. `key` is stored only in Windows Credential Manager; an
/// empty or missing key keeps the existing one. Nothing is sent anywhere here.
#[tauri::command]
fn set_ai(
    state: State<AppState>,
    provider: String,
    model: String,
    base_url: String,
    key: Option<String>,
    consent: bool,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    if provider == "none" {
        if let Some(old) = db::get_setting(&conn, AI_PROVIDER) {
            db::revoke_consent(&conn, &old)?;
        }
        return db::set_setting(&conn, AI_PROVIDER, "none");
    }
    let preset = providers::preset(&provider).ok_or("unknown provider")?;
    if !consent {
        return Err("Please tick the box to agree before turning on AI clean-up.".into());
    }
    let base_url = base_url.trim().to_string();
    if !base_url.is_empty()
        && !(base_url.starts_with("https://")
            || base_url.starts_with("http://localhost")
            || base_url.starts_with("http://127.0.0.1"))
    {
        return Err("The server address must start with https:// (or be on this computer).".into());
    }
    if let Some(k) = key.as_deref().map(str::trim).filter(|k| !k.is_empty()) {
        credentials::save(&provider, k)?;
    }
    if preset.needs_key && credentials::load(&provider).is_none() {
        return Err("Paste your API key first.".into());
    }
    db::set_setting(&conn, AI_PROVIDER, &provider)?;
    db::set_setting(&conn, AI_MODEL, model.trim())?;
    db::set_setting(&conn, AI_BASE_URL, &base_url)?;
    let endpoint = if base_url.is_empty() {
        preset.base_url
    } else {
        &base_url
    };
    db::record_consent(&conn, &provider, endpoint, model.trim(), preset.data_note)
}

#[tauri::command]
fn forget_ai_key(state: State<AppState>, provider: String) -> Result<(), String> {
    credentials::delete(&provider);
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::revoke_consent(&conn, &provider)?;
    if db::get_setting(&conn, AI_PROVIDER).as_deref() == Some(provider.as_str()) {
        db::set_setting(&conn, AI_PROVIDER, "none")?;
    }
    Ok(())
}

#[derive(Serialize)]
struct AiTest {
    ok: bool,
    ms: u64,
    message: String,
}

/// Sends one fixed sample sentence (no personal data) to check the key and the speed.
#[tauri::command]
async fn test_ai(state: State<'_, AppState>) -> Result<AiTest, String> {
    let mut cfg = ai_config(&state).ok_or("AI clean-up is not set up yet.")?;
    // Make sure the model still exists (providers retire models).
    if let Some(model) = heal_ai_model(&state, &cfg) {
        cfg.model = model;
    }
    let sample = "um so lets meet at 2 PM actually make that 3 PM on monday";
    let prepared = app_core::pipeline::prepare(
        sample,
        &app_core::pipeline::Settings::default(),
        &app_core::dictionary::Dictionary::default(),
        &app_core::snippets::SnippetSet::default(),
    );
    let request = app_core::llm::build_cleanup(
        &prepared,
        app_core::pipeline::Level::Light,
        app_core::llm::AppCategory::Email,
        "en",
        &[],
    );
    let log_cfg = cfg.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        providers::chat(
            &cfg,
            &request.system,
            &request.user,
            std::time::Duration::from_secs(15),
        )
        .map(|a| (a, request.system.len() + request.user.len()))
    })
    .await
    .map_err(|e| e.to_string())?;
    if let Ok(conn) = state.db.lock() {
        db::log_event(
            &conn,
            "upload",
            &format!(
                "{} ({}): connection test with a fixed sample sentence",
                log_cfg.provider, log_cfg.model
            ),
        );
    }
    Ok(match result {
        Ok((answer, _)) => {
            let ms = answer.elapsed.as_millis() as u64;
            match app_core::llm::parse_reply(&answer.content)
                .map_err(|e| e.to_string())
                .and_then(|r| {
                    let rules = app_core::pipeline::finish(
                        &prepared,
                        &app_core::pipeline::Settings::default(),
                    )
                    .text;
                    app_core::llm::accept(
                        &r,
                        &prepared,
                        sample,
                        &[],
                        app_core::pipeline::Level::Light,
                        &rules,
                    )
                    .map_err(|e| e.to_string())
                }) {
                Ok(text) => AiTest {
                    ok: true,
                    ms,
                    message: format!(
                        "Working. Sample result: \"{text}\" ({ms} ms){}",
                        if ms > 2500 {
                            ". Note: slower than the 2.5 s limit, so dictations will often use the built-in rules."
                        } else {
                            ""
                        }
                    ),
                },
                Err(why) => AiTest {
                    ok: false,
                    ms,
                    message: format!(
                        "Connected, but the answer was not usable: {why}. Try another model."
                    ),
                },
            }
        }
        Err(e) => AiTest {
            ok: false,
            ms: 0,
            message: format!("Not working: {e}."),
        },
    })
}

#[tauri::command]
fn list_uploads(state: State<AppState>) -> Result<Vec<db::EventRow>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::list_events(&conn, "upload", 200)
}

#[tauri::command]
fn list_terms(state: State<AppState>) -> Result<Vec<db::TermRow>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::list_terms(&conn)
}

#[tauri::command]
fn add_term(
    state: State<AppState>,
    term: String,
    variants: Vec<String>,
    auto_replace: Option<bool>,
) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::upsert_term(
        &conn,
        &term,
        &variants,
        "manual",
        auto_replace.unwrap_or(true),
    )
}

#[tauri::command]
fn delete_term(state: State<AppState>, id: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::delete_term(&conn, &id)
}

#[tauri::command]
fn list_snippets(state: State<AppState>) -> Result<Vec<db::SnippetRow>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::list_snippets(&conn)
}

#[tauri::command]
fn add_snippet(state: State<AppState>, trigger: String, body: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::upsert_snippet(&conn, &trigger, &body)
}

#[tauri::command]
fn delete_snippet(state: State<AppState>, id: String) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::delete_snippet(&conn, &id)
}

#[tauri::command]
fn last_dictation_id(state: State<AppState>) -> Option<String> {
    state.db.lock().ok().and_then(|c| db::last_dictation_id(&c))
}

/// Saves a corrected dictation and returns dictionary suggestions learned from the fix.
#[tauri::command]
fn fix_dictation(
    state: State<AppState>,
    id: String,
    text: String,
) -> Result<Vec<app_core::learn::Suggestion>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let before = db::get_dictation(&conn, &id)?;
    db::update_final_text(&conn, &id, &text)?;
    Ok(app_core::learn::suggest(&before.final_text, &text))
}

#[tauri::command]
fn learn_terms(state: State<AppState>, items: Vec<(String, String)>) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    for (heard, term) in items {
        db::upsert_term(&conn, &term, &[heard], "learned", true)?;
    }
    Ok(())
}

/// First run: enable the Finance and Tech word packs (editable afterwards).
fn seed_packs(conn: &Connection) {
    if db::get_setting(conn, "packs_seeded").is_some() {
        return;
    }
    for pack in ["finance", "tech"] {
        for t in app_core::dictionary::pack(pack) {
            let _ = db::upsert_term(conn, &t.term, &t.spoken_variants, "pack", t.auto_replace);
        }
    }
    let _ = db::set_setting(conn, "packs_seeded", "finance,tech");
}

#[derive(Serialize)]
struct ModelRow {
    #[serde(flatten)]
    info: models::ModelInfo,
    installed: bool,
}

#[derive(Serialize)]
struct Status {
    app_name: &'static str,
    version: &'static str,
    gpu_build: bool,
    running_as_admin: bool,
    hook_error: Option<String>,
    selected_model: String,
    model_installed: bool,
    words_this_week: i64,
    shortcuts: Vec<(&'static str, &'static str)>,
    /// Cloud speech recognition is on (and agreed to).
    cloud_speech: bool,
}

#[tauri::command]
fn get_status(state: State<AppState>) -> Status {
    let selected = state.setting("model");
    let installed =
        models::find(&selected).is_some_and(|m| models::is_installed(&state.models_dir(), m));
    let week_ago = (chrono::Utc::now() - chrono::Duration::days(7)).to_rfc3339();
    let words = state
        .db
        .lock()
        .map(|c| db::words_since(&c, &week_ago))
        .unwrap_or(0);
    Status {
        app_name: app_core::brand::APP_NAME,
        version: env!("CARGO_PKG_VERSION"),
        gpu_build: cfg!(feature = "gpu"),
        running_as_admin: insert::target::self_is_elevated(),
        hook_error: state.hook_error.lock().ok().and_then(|e| e.clone()),
        selected_model: selected,
        model_installed: installed,
        words_this_week: words,
        shortcuts: vec![
            ("Hold Ctrl + Win", "Dictate while held"),
            (
                "Double-tap Ctrl + Win, or Ctrl + Win + H",
                "Hands-free on / off",
            ),
            ("Esc", "Cancel while recording"),
            ("Alt + Shift + Z", "Paste last dictation"),
            ("Alt + Shift + X", "Copy last dictation"),
            (
                "Alt + Shift + F",
                "Fix last dictation (teaches the dictionary)",
            ),
            (
                "Alt + Shift + H",
                "Switch language: English + Hinglish / English only / Hinglish only",
            ),
            (
                "Alt + Shift + C",
                "Command Mode: select text, press, say what to do (\"make this a bulleted list\"), press again",
            ),
            ("Alt + Shift + N", "Open the Scratchpad (notes you can dictate into)"),
        ],
        cloud_speech: asr_config(&state).is_some(),
    }
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> std::collections::BTreeMap<String, String> {
    SETTINGS
        .iter()
        .map(|(k, _)| (k.to_string(), state.setting(k)))
        .collect()
}

#[tauri::command]
fn set_setting(state: State<AppState>, key: String, value: String) -> Result<(), String> {
    if !SETTINGS.iter().any(|(k, _)| *k == key) {
        return Err(format!("unknown setting '{key}'"));
    }
    state
        .db
        .lock()
        .map_err(|e| e.to_string())
        .and_then(|c| db::set_setting(&c, &key, &value))?;
    let _ = state.ctl.lock().map(|tx| tx.send(Msg::SettingsChanged));
    Ok(())
}

#[tauri::command]
fn list_dictations(
    state: State<AppState>,
    limit: Option<u32>,
) -> Result<Vec<db::Dictation>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::list_dictations(&conn, limit.unwrap_or(200))
}

#[tauri::command]
fn delete_dictation(state: State<AppState>, id: String) -> Result<(), String> {
    let audio = state
        .db
        .lock()
        .map_err(|e| e.to_string())
        .and_then(|c| db::delete_dictation(&c, &id))?;
    if let Some(path) = audio {
        let _ = std::fs::remove_file(path);
    }
    Ok(())
}

/// Copies text privately (kept out of Win+V history, like every dictation).
#[tauri::command]
fn copy_text(text: String) -> Result<(), String> {
    insert::clipboard::write_text_private(&text).map(|_| ())
}

#[tauri::command]
fn list_models(state: State<AppState>) -> Vec<ModelRow> {
    let dir = state.models_dir();
    models::CATALOGUE
        .iter()
        .map(|m| ModelRow {
            info: *m,
            installed: models::is_installed(&dir, m),
        })
        .collect()
}

#[derive(Clone, Serialize)]
struct ModelProgress {
    id: String,
    done: u64,
    total: u64,
    finished: bool,
    error: Option<String>,
}

#[tauri::command]
fn download_model(app: AppHandle, state: State<AppState>, id: String) -> Result<(), String> {
    let model = *models::find(&id).ok_or("unknown model")?;
    let dir = state.models_dir();
    std::thread::spawn(move || {
        let last = std::cell::Cell::new(std::time::Instant::now());
        let report = |done: u64, total: u64| {
            if last.get().elapsed().as_millis() >= 200 {
                last.set(std::time::Instant::now());
                let _ = app.emit(
                    "model-progress",
                    ModelProgress {
                        id: model.id.into(),
                        done,
                        total,
                        finished: false,
                        error: None,
                    },
                );
            }
        };
        let result = models::download(&dir, &model, &report);
        let _ = app.emit(
            "model-progress",
            ModelProgress {
                id: model.id.into(),
                done: 0,
                total: 0,
                finished: true,
                error: result.err(),
            },
        );
    });
    Ok(())
}

#[tauri::command]
fn delete_model(state: State<AppState>, id: String) -> Result<(), String> {
    let model = models::find(&id).ok_or("unknown model")?;
    let _ = state.ctl.lock().map(|tx| tx.send(Msg::UnloadModel));
    match std::fs::remove_file(models::path_for(&state.models_dir(), model)) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("Could not delete the model (is it in use?): {e}")),
    }
}

/// Transcribes a WAV file through the full pipeline. With `insert`, the result is inserted
/// into whichever window is in front after `delay_ms`.
#[tauri::command]
async fn transcribe_file(
    state: State<'_, AppState>,
    path: String,
    insert: bool,
    delay_ms: Option<u64>,
) -> Result<DictationResult, String> {
    let (reply, rx) = channel();
    state
        .ctl
        .lock()
        .map_err(|e| e.to_string())?
        .send(Msg::File {
            path: PathBuf::from(path),
            insert,
            delay_ms: delay_ms.unwrap_or(0),
            command: false,
            reply,
        })
        .map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || rx.recv().map_err(|e| e.to_string())?)
        .await
        .map_err(|e| e.to_string())?
}

fn show_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("main") {
        let _ = w.show();
        let _ = w.unminimize();
        let _ = w.set_focus();
    }
}

/// Automated end-to-end check, used by `scripts/selftest-insert.ps1`. When the environment
/// variable `APP_SELFTEST_WAV` names a WAV file, the app transcribes it through the normal
/// pipeline, optionally inserts the result into the window in front, writes the outcome as
/// JSON to `APP_SELFTEST_OUT`, and exits. Does nothing in normal use.
fn self_test(app: &AppHandle) {
    let Some(wav) = std::env::var_os("APP_SELFTEST_WAV") else {
        return;
    };
    let out = std::env::var_os("APP_SELFTEST_OUT").map(PathBuf::from);
    let insert = std::env::var("APP_SELFTEST_INSERT").is_ok_and(|v| v == "1");
    // Command Mode self-test: the WAV is a spoken instruction for the selection in front.
    let command = std::env::var("APP_SELFTEST_COMMAND").is_ok_and(|v| v == "1");
    let delay_ms = std::env::var("APP_SELFTEST_DELAY_MS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(0);
    // Safety: a self-test only ever pastes into a window with this exact title. Without it,
    // or if that window is not in front within the wait, nothing is pasted anywhere.
    let expect_title = std::env::var("APP_SELFTEST_EXPECT_TITLE").unwrap_or_default();
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.hide(); // the main window would otherwise take focus at start-up
    }
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(1500));
        if insert {
            let deadline = std::time::Instant::now()
                + std::time::Duration::from_millis(delay_ms.max(1000) + 8000);
            let in_front = || {
                !expect_title.is_empty()
                    && insert::target::foreground().is_some_and(|t| t.title == expect_title)
            };
            while !in_front() && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
            if !in_front() {
                let msg = format!("self-test target window '{expect_title}' never came to the front; nothing was pasted");
                if let Some(path) = &out {
                    let json = serde_json::json!({ "ok": false, "error": msg });
                    let _ = std::fs::write(path, json.to_string());
                }
                app.exit(2);
                return;
            }
        }
        // The target is confirmed in front, so no extra delay is needed.
        let delay_ms = if insert { 0 } else { delay_ms };
        let (reply, rx) = channel();
        let sent = app
            .state::<AppState>()
            .ctl
            .lock()
            .map_err(|e| e.to_string())
            .and_then(|tx| {
                tx.send(Msg::File {
                    path: PathBuf::from(wav),
                    insert,
                    delay_ms,
                    command,
                    reply,
                })
                .map_err(|e| e.to_string())
            });
        let result = match sent {
            Ok(()) => rx.recv().map_err(|e| e.to_string()).and_then(|r| r),
            Err(e) => Err(e),
        };
        let json = match &result {
            Ok(r) => serde_json::json!({ "ok": true, "result": r }),
            Err(e) => serde_json::json!({ "ok": false, "error": e }),
        };
        if let Some(path) = out {
            let _ = std::fs::write(
                path,
                serde_json::to_string_pretty(&json).unwrap_or_default(),
            );
        }
        // Leave the pill visible for a moment so the paste has settled, then quit.
        std::thread::sleep(std::time::Duration::from_millis(1200));
        app.exit(0);
    });
}

pub fn run() {
    let mut builder = tauri::Builder::default();
    // One copy at a time in normal use. A self-test run (separate data folder) must not hand
    // over to, or disturb, the copy the owner is using.
    if std::env::var_os("APP_SELFTEST_WAV").is_none() {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app)
        }));
    }
    builder
        .invoke_handler(tauri::generate_handler![
            get_status,
            get_settings,
            set_setting,
            list_dictations,
            delete_dictation,
            copy_text,
            list_models,
            download_model,
            delete_model,
            transcribe_file,
            get_ai,
            set_ai,
            forget_ai_key,
            test_ai,
            list_uploads,
            list_terms,
            add_term,
            delete_term,
            list_snippets,
            add_snippet,
            delete_snippet,
            last_dictation_id,
            fix_dictation,
            learn_terms,
            mic_status,
            set_cloud_asr,
            pill_action,
            start_meeting,
            meeting_status,
            stop_meeting
        ])
        .setup(|app| {
            let handle = app.handle().clone();
            // APP_DATA_DIR lets tests and development runs use a throwaway folder.
            let data_dir = match std::env::var_os("APP_DATA_DIR") {
                Some(dir) => PathBuf::from(dir),
                None => app.path().app_data_dir()?,
            };
            std::fs::create_dir_all(data_dir.join("models"))?;
            std::fs::create_dir_all(data_dir.join("audio"))?;
            disable_power_throttling();
            let conn = db::open(&data_dir.join("app.sqlite3"))?;
            seed_packs(&conn);

            let (ctl_tx, ctl_rx) = channel::<Msg>();
            // Keyboard hook -> controller.
            let (key_tx, key_rx) = channel::<hotkey::Action>();
            let hook_error = hotkey::start(key_tx).err();
            {
                let ctl_tx = ctl_tx.clone();
                std::thread::spawn(move || {
                    for action in key_rx {
                        if ctl_tx.send(Msg::Key(action)).is_err() {
                            break;
                        }
                    }
                });
            }

            let recorder = audio::Recorder::spawn();
            app.manage(AppState {
                db: Mutex::new(conn),
                data_dir,
                ctl: Mutex::new(ctl_tx),
                hook_error: Mutex::new(hook_error),
                recorder: recorder.clone(),
                meeting: Mutex::new(None),
            });
            overlay::prepare(&handle);
            controller::spawn(handle.clone(), ctl_rx, recorder);

            // Tray icon: the app keeps running in the background when its window is closed.
            let open = MenuItem::with_id(app, "open", "Open", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &quit])?;
            let mut tray = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip(app_core::brand::APP_NAME)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "open" => show_main(app),
                    "quit" => app.exit(0),
                    _ => {}
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            self_test(&handle);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                if window.label() == "main" {
                    let _ = window.hide();
                    api.prevent_close();
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running the app");
}

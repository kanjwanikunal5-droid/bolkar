//! Desktop app: system-wide dictation for Windows.

mod audio;
mod controller;
pub mod credentials;
mod db;
pub mod diag;
mod hotkey;
mod insert;
mod insights;
mod library;
mod meeting;
mod models;
mod overlay;
pub mod providers;
mod update;

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
    pub meeting: Mutex<Option<library::ActiveMeeting>>,
}

/// Deletes a file the app created. Only files inside the app's data folder are deleted; a
/// file that is already gone counts as deleted. Errors name the file.
pub fn remove_app_file(data_dir: &std::path::Path, path: &str) -> Result<(), String> {
    let p = std::path::Path::new(path);
    let base = data_dir
        .canonicalize()
        .unwrap_or_else(|_| data_dir.to_path_buf());
    let target = match p.canonicalize() {
        Ok(c) => c,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(format!("{path}: {e}")),
    };
    if !target.starts_with(&base) {
        return Err(format!(
            "{path}: outside the app's data folder, not deleted"
        ));
    }
    match std::fs::remove_file(&target) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("{path}: {e}")),
    }
}

/// Other dictation apps that are running now. They may listen to the same keys or hold the
/// microphone, so the app warns about them (it cannot tell which keys they use).
fn other_dictation_apps() -> Vec<String> {
    const KNOWN: &[(&str, &str)] = &[
        ("wispr flow.exe", "Wispr Flow"),
        ("typeless.exe", "Typeless"),
        ("genspark speakly.exe", "Genspark Speakly"),
        ("superwhisper.exe", "Superwhisper"),
        ("aqua voice.exe", "Aqua Voice"),
        ("willow voice.exe", "Willow Voice"),
    ];
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    let mut found = Vec::new();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return found;
        };
        let mut entry = PROCESSENTRY32W {
            dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut ok = Process32FirstW(snap, &mut entry).is_ok();
        while ok {
            let len = entry
                .szExeFile
                .iter()
                .position(|c| *c == 0)
                .unwrap_or(entry.szExeFile.len());
            let name = String::from_utf16_lossy(&entry.szExeFile[..len]).to_lowercase();
            if let Some((_, label)) = KNOWN.iter().find(|(exe, _)| *exe == name) {
                if !found.iter().any(|f| f == label) {
                    found.push(label.to_string());
                }
            }
            ok = Process32NextW(snap, &mut entry).is_ok();
        }
        let _ = CloseHandle(snap);
    }
    found
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
    // Ask GitHub for a newer version at start-up. Off by default (local first).
    ("check_updates", "off"),
    // Appearance: "system", "light" or "dark".
    ("theme", "system"),
    // Shown in the greeting on Home; empty = no name.
    ("user_name", ""),
    // Writing style per kind of app (Style page): "casual", "clear" or "formal".
    ("style_chat", "casual"),
    ("style_email", "formal"),
    ("style_docs", "clear"),
    ("style_other", "clear"),
    // Words per day for the Home challenge.
    ("daily_goal", "100"),
    // Short sounds when dictation starts and ends.
    ("sounds", "on"),
    // Hands-free lists: "natural" (paragraphs; lists only when spoken: "first ..., second
    // ...", "bullet point"), "auto" (also short independent requests) or "always". Replaces
    // the 0.4.0 "handsfree_points" switch, which made every sentence a bullet.
    ("list_format", "natural"),
    // Retention in days; "0" keeps forever. "audio_retention_days" is dictation audio.
    ("meeting_audio_retention_days", "30"),
    ("history_retention_days", "0"),
    ("meeting_retention_days", "0"),
    // Content-free diagnostics log (Settings > Privacy).
    ("diagnostics", "on"),
    // Meeting devices by name; empty = the Windows default.
    ("meeting_mic_device", ""),
    ("meeting_output_device", ""),
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

/// Speech models from best to least accurate (measured, see docs/STATUS.md). Large v3 needs
/// AI clean-up for Roman Hinglish, so it only ranks first when AI clean-up is on.
/// Groq speech recognition is on (read from the environment-independent store).
fn cloud_on() -> bool {
    CLOUD_ON.load(std::sync::atomic::Ordering::Relaxed)
}

static CLOUD_ON: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn model_rank(id: &str, ai_on: bool) -> u8 {
    match id {
        // Without a graphics card the big models are far too slow: prefer the small ones.
        "large-v3" | "large-v3-turbo" if !gpu_available() => 8,
        // With Groq doing the recognition, the local model is only a backup: the fast one wins.
        "large-v3" if ai_on && !cloud_on() => 0,
        "large-v3-turbo" => 1,
        "large-v3" => 2,
        "small" => 3,
        "base" => 4,
        _ => 9,
    }
}

/// True when this build can use a graphics card and one is present.
fn gpu_available() -> bool {
    cfg!(feature = "gpu") && app_asr::best_gpu().is_some()
}

/// Recognition on the processor is many times slower; with a large model a dictation can take
/// minutes, which looks like "dictation does not work" (owner's laptop, 15 Nov 2026: the
/// graphics-card switch was off and Large v3 was selected). Once: switch it back on when a
/// graphics card exists. The user can still turn it off again.
fn heal_gpu_setting(state: &AppState) {
    let Ok(conn) = state.db.lock() else { return };
    if db::get_setting(&conn, "gpu_heal").as_deref() == Some("v1") {
        return;
    }
    if gpu_available() && db::get_setting(&conn, "use_gpu").as_deref() == Some("false") {
        let _ = db::set_setting(&conn, "use_gpu", "true");
        db::log_event(
            &conn,
            "info",
            "graphics card switched back on (speech recognition was running on the processor)",
        );
    }
    let _ = db::set_setting(&conn, "gpu_heal", "v1");
}

/// Selects the best installed speech model when the selected one is missing, or when a
/// better one is installed and the user never picked a model by hand.
fn auto_pick_model(state: &AppState) {
    let ai_on = ai_config(state).is_some();
    CLOUD_ON.store(
        asr_config(state).is_some(),
        std::sync::atomic::Ordering::Relaxed,
    );
    let dir = state.models_dir();
    let best = models::CATALOGUE
        .iter()
        .filter(|m| models::is_installed(&dir, m))
        .min_by_key(|m| model_rank(m.id, ai_on));
    let Some(best) = best else { return };
    let Ok(conn) = state.db.lock() else { return };
    let current =
        db::get_setting(&conn, "model").unwrap_or_else(|| default_setting("model").into());
    let current_installed = models::find(&current).is_some_and(|m| models::is_installed(&dir, m));
    let chosen_by_hand = db::get_setting(&conn, "model_chosen").as_deref() == Some("1");
    let better = model_rank(best.id, ai_on) < model_rank(&current, ai_on);
    if !current_installed || (better && !chosen_by_hand) {
        let _ = db::set_setting(&conn, "model", best.id);
    }
}

/// The owner's chosen defaults (14 Nov 2026): when a Groq key is saved, AI clean-up and Groq
/// speech recognition are on, for the best measured accuracy. Applied once; both stay
/// switchable in Settings, and every upload is listed under "Data that left this computer".
fn apply_recommended_defaults(state: &AppState) {
    // Never in test runs (their own data folder): the Groq key in Credential Manager is the
    // owner's, and tests must not send audio or text with it.
    if std::env::var_os("APP_DATA_DIR").is_some() || std::env::var_os("APP_SELFTEST_WAV").is_some()
    {
        return;
    }
    let Ok(conn) = state.db.lock() else { return };
    if db::get_setting(&conn, "recommended_defaults").as_deref() == Some("v1") {
        return;
    }
    if credentials::load("groq").is_some() {
        let _ = enable_groq(&conn);
        db::log_event(
            &conn,
            "info",
            "recommended setup: AI clean-up and Groq speech recognition turned on (Groq key present)",
        );
    }
    let _ = db::set_setting(&conn, "recommended_defaults", "v1");
}

/// Turns on Groq for both AI clean-up (text) and speech recognition (audio), recording the
/// consent shown next to the controls that call this.
fn enable_groq(conn: &Connection) -> Result<(), String> {
    let preset = providers::preset("groq").ok_or("unknown provider")?;
    if db::get_setting(conn, AI_PROVIDER).as_deref() != Some("groq") {
        db::set_setting(conn, AI_PROVIDER, "groq")?;
        db::set_setting(conn, AI_MODEL, preset.model)?;
        db::set_setting(conn, AI_BASE_URL, "")?;
    }
    db::record_consent(
        conn,
        "groq",
        preset.base_url,
        preset.model,
        preset.data_note,
    )?;
    db::record_consent(
        conn,
        CLOUD_ASR_CONSENT,
        "https://api.groq.com/openai/v1",
        CLOUD_ASR_MODEL,
        CLOUD_ASR_NOTE,
    )?;
    db::set_setting(conn, CLOUD_ASR, "on")
}

/// One step: checks the Groq key with Groq, saves it in Windows Credential Manager, and turns
/// on AI clean-up and Groq speech recognition (the screen says so before the button).
#[tauri::command]
async fn connect_groq(state: State<'_, AppState>, key: String) -> Result<String, String> {
    let key = key.trim().to_string();
    if key.len() < 20 || key.contains(char::is_whitespace) {
        return Err(
            "That does not look like a Groq key. Copy it again from console.groq.com/keys.".into(),
        );
    }
    let preset = providers::preset("groq").ok_or("unknown provider")?;
    let cfg = providers::Config {
        provider: "groq".into(),
        base_url: preset.base_url.to_string(),
        model: preset.model.to_string(),
        key: Some(key.clone()),
    };
    let check = tauri::async_runtime::spawn_blocking(move || {
        providers::list_models(&cfg, std::time::Duration::from_secs(8))
    })
    .await
    .map_err(|e| e.to_string())?;
    let note = match check {
        Ok(_) => "Connected. Groq accepted the key.".to_string(),
        Err(providers::CallError::BadKey) => {
            return Err("Groq did not accept this key. Check it at console.groq.com/keys.".into())
        }
        Err(e) => format!(
            "Saved. Groq could not be reached to check the key ({e}); it will be used when it can."
        ),
    };
    credentials::save("groq", &key)?;
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        enable_groq(&conn)?;
        db::log_event(
            &conn,
            "info",
            "Groq connected: AI clean-up and Groq speech recognition on",
        );
    }
    auto_pick_model(&state);
    let _ = state.ctl.lock().map(|tx| tx.send(Msg::SettingsChanged));
    Ok(note)
}

/// AI clean-up on or off without touching the saved key.
#[tauri::command]
fn set_ai_enabled(state: State<AppState>, on: bool) -> Result<(), String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    if !on {
        if let Some(old) = db::get_setting(&conn, AI_PROVIDER) {
            db::revoke_consent(&conn, &old)?;
        }
        return db::set_setting(&conn, AI_PROVIDER, "none");
    }
    let provider = db::get_setting(&conn, AI_PROVIDER)
        .filter(|p| p != "none")
        .unwrap_or_else(|| "groq".into());
    let preset = providers::preset(&provider).ok_or("unknown provider")?;
    if preset.needs_key && credentials::load(&provider).is_none() {
        return Err("Connect Groq first: paste your key above.".into());
    }
    db::set_setting(&conn, AI_PROVIDER, &provider)?;
    if db::get_setting(&conn, AI_MODEL)
        .unwrap_or_default()
        .is_empty()
    {
        db::set_setting(&conn, AI_MODEL, preset.model)?;
    }
    db::record_consent(
        &conn,
        &provider,
        preset.base_url,
        preset.model,
        preset.data_note,
    )
}

#[tauri::command]
fn get_insights(state: State<AppState>) -> Result<insights::Insights, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    let rows = db::usage_rows(&conn)?;
    let to_local = |s: &str| {
        chrono::DateTime::parse_from_rfc3339(s)
            .ok()
            .map(|d| d.with_timezone(&chrono::Local).date_naive())
    };
    Ok(insights::compute(
        &rows,
        chrono::Local::now().date_naive(),
        &to_local,
        db::count_terms(&conn, "learned"),
        db::count_terms(&conn, "manual"),
    ))
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
    /// Last four characters of the saved Groq key, so the user can see which key is saved.
    groq_key_hint: Option<String>,
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
        groq_key_hint: credentials::load("groq").map(|k| {
            let tail: String = k
                .chars()
                .rev()
                .take(4)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            tail
        }),
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
    /// Speech recognition on this computer runs on the graphics card.
    gpu_on: bool,
    /// A usable graphics card exists.
    gpu_present: bool,
    /// Other dictation apps running now (they may use the same keys or the microphone).
    other_dictation_apps: Vec<String>,
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
        gpu_on: cfg!(feature = "gpu") && state.setting("use_gpu") == "true" && gpu_available(),
        gpu_present: gpu_available(),
        other_dictation_apps: other_dictation_apps(),
    }
}

/// Asks GitHub whether a newer version is published (user-initiated or opted in).
#[tauri::command]
async fn check_update() -> update::UpdateInfo {
    tauri::async_runtime::spawn_blocking(|| {
        update::check(app_core::brand::REPOSITORY, env!("CARGO_PKG_VERSION"))
    })
    .await
    .unwrap_or_else(|e| update::UpdateInfo {
        current: env!("CARGO_PKG_VERSION").into(),
        latest: None,
        newer: false,
        url: None,
        message: format!("Could not check: {e}"),
    })
}

/// Opens this project's release page in the browser. Only github.com release pages of this
/// repository are accepted.
#[tauri::command]
fn open_release_page(url: Option<String>) -> Result<(), String> {
    let base = format!(
        "https://github.com/{}/releases",
        app_core::brand::REPOSITORY
    );
    let url = url.unwrap_or_else(|| format!("{base}/latest"));
    if app_core::brand::REPOSITORY.is_empty()
        || !(url == format!("{base}/latest") || url.starts_with(&format!("{base}/")))
        || url.contains(['"', ' ', '&', '^', '|'])
    {
        return Err("not a release page of this project".into());
    }
    std::process::Command::new("explorer.exe")
        .arg(&url)
        .spawn()
        .map(|_| ())
        .map_err(|e| e.to_string())
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
    state.db.lock().map_err(|e| e.to_string()).and_then(|c| {
        if key == "model" {
            // Picked by hand: no automatic switching to another model afterwards.
            db::set_setting(&c, "model_chosen", "1")?;
        }
        db::set_setting(&c, &key, &value)
    })?;
    if key == "diagnostics" {
        diag::set_enabled(value != "off");
    }
    let _ = state.ctl.lock().map(|tx| tx.send(Msg::SettingsChanged));
    if key.ends_with("retention_days") {
        let _ = state.ctl.lock().map(|tx| tx.send(Msg::Retention));
    }
    Ok(())
}

#[derive(Serialize)]
struct DiagnosticsInfo {
    enabled: bool,
    path: Option<String>,
    bytes: u64,
}

#[tauri::command]
fn diagnostics_info(state: State<AppState>) -> DiagnosticsInfo {
    let path = diag::file();
    let bytes = path
        .iter()
        .flat_map(|p| [p.clone(), p.with_file_name("diagnostics.1.log")])
        .filter_map(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
        .sum();
    DiagnosticsInfo {
        enabled: state.setting("diagnostics") != "off",
        path: path.map(|p| p.to_string_lossy().into_owned()),
        bytes,
    }
}

#[tauri::command]
fn delete_logs() -> Result<(), String> {
    diag::delete()
}

/// A redacted diagnostics file for sending to whoever helps with a problem: versions, the
/// non-personal settings, the device names, recent log lines and recent local events. No
/// dictated text, transcripts, meeting titles, names or keys.
#[tauri::command]
fn export_diagnostics(app: AppHandle, state: State<AppState>) -> Result<String, String> {
    const SKIP: &[&str] = &["user_name", "scratchpad"];
    let mut out = String::new();
    out.push_str(&format!(
        "{} {} diagnostics, {}\n\n",
        app_core::brand::APP_NAME,
        env!("CARGO_PKG_VERSION"),
        chrono::Utc::now().to_rfc3339()
    ));
    out.push_str(&format!(
        "GPU build: {} | graphics card found: {} | Windows: {}\n",
        cfg!(feature = "gpu"),
        gpu_available(),
        std::env::var("OS").unwrap_or_default()
    ));
    out.push_str(&format!(
        "AI clean-up: {} | cloud speech: {} | other dictation apps running: {:?}\n\n",
        ai_config(&state)
            .map(|c| format!("{} ({})", c.provider, c.model))
            .unwrap_or_else(|| "off".into()),
        asr_config(&state).is_some(),
        other_dictation_apps()
    ));
    out.push_str("Settings:\n");
    for (k, _) in SETTINGS.iter().filter(|(k, _)| !SKIP.contains(k)) {
        out.push_str(&format!("  {k} = {}\n", state.setting(k)));
    }
    let d = meeting::list_devices();
    out.push_str(&format!(
        "\nInputs: {:?}\nOutputs: {:?}\n",
        d.inputs, d.outputs
    ));
    out.push_str("\nRecent events:\n");
    if let Ok(conn) = state.db.lock() {
        for kind in ["timing", "upload", "retention", "meeting", "deleted"] {
            for e in db::list_events(&conn, kind, 30).unwrap_or_default() {
                out.push_str(&format!(
                    "  {} [{}] {}\n",
                    e.at,
                    e.kind,
                    diag::redact(&e.detail)
                ));
            }
        }
    }
    out.push_str("\nDiagnostics log (last 500 lines):\n");
    for l in diag::tail(500) {
        out.push_str(&format!("  {l}\n"));
    }
    let dir = app
        .path()
        .download_dir()
        .map_err(|e| e.to_string())?
        .join(app_core::brand::APP_NAME);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!(
        "diagnostics {}.txt",
        chrono::Utc::now().format("%Y-%m-%d %H-%M")
    ));
    std::fs::write(&path, diag::redact(&out)).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
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
        if result.is_ok() {
            // A newly downloaded model is selected when it is better than the current one.
            let state = app.state::<AppState>();
            let ai_on = ai_config(&state).is_some();
            if let Ok(conn) = state.db.lock() {
                let current = db::get_setting(&conn, "model").unwrap_or_default();
                let current_ok =
                    models::find(&current).is_some_and(|m| models::is_installed(&dir, m));
                if !current_ok || model_rank(model.id, ai_on) < model_rank(&current, ai_on) {
                    let _ = db::set_setting(&conn, "model", model.id);
                }
            }
            let _ = state.ctl.lock().map(|tx| tx.send(Msg::SettingsChanged));
        }
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

/// Test-only: a self-test with `APP_SELFTEST_QUIET=1` shows nothing on screen (no pill, no
/// tray icon) and plays no sounds, so tests can run while the owner uses the computer.
pub(crate) fn quiet_test() -> bool {
    std::env::var_os("APP_SELFTEST_WAV").is_some()
        && std::env::var("APP_SELFTEST_QUIET").is_ok_and(|v| v == "1")
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
    // Meeting pipeline test: APP_SELFTEST_MEETING names a folder with mic.wav / system.wav.
    if let Some(dir) = std::env::var_os("APP_SELFTEST_MEETING") {
        if let Some(main) = app.get_webview_window("main") {
            let _ = main.hide();
        }
        library::self_test_meeting(app.clone(), PathBuf::from(dir), out);
        return;
    }
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
        // Long-run test: the same recording N times (never inserted), one JSON line per run.
        let repeat: u32 = std::env::var("APP_SELFTEST_REPEAT")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1);
        if repeat > 1 && !insert && !command {
            let mut lines = Vec::new();
            for run in 1..=repeat {
                let (reply, rx) = channel();
                let started = std::time::Instant::now();
                let sent = app
                    .state::<AppState>()
                    .ctl
                    .lock()
                    .map_err(|e| e.to_string())
                    .and_then(|tx| {
                        tx.send(Msg::File {
                            path: PathBuf::from(&wav),
                            insert: false,
                            delay_ms: 0,
                            command: false,
                            reply,
                        })
                        .map_err(|e| e.to_string())
                    });
                let result = match sent {
                    Ok(()) => rx.recv().map_err(|e| e.to_string()).and_then(|r| r),
                    Err(e) => Err(e),
                };
                let line = match &result {
                    Ok(r) => serde_json::json!({
                        "run": run, "ok": true, "total_ms": started.elapsed().as_millis() as u64,
                        "transcribe_ms": r.transcribe_ms, "text": r.final_text,
                    }),
                    Err(e) => serde_json::json!({ "run": run, "ok": false, "error": e }),
                };
                lines.push(line.to_string());
                if let Some(path) = &out {
                    let _ = std::fs::write(path, lines.join("\n"));
                }
            }
            app.exit(0);
            return;
        }
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
    // One copy at a time in normal use. A test run with its own data folder (APP_DATA_DIR, or
    // a self-test) must not hand over to, or disturb, the copy the owner is using.
    if std::env::var_os("APP_SELFTEST_WAV").is_none() && std::env::var_os("APP_DATA_DIR").is_none()
    {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app)
        }));
    }
    builder
        .invoke_handler(tauri::generate_handler![
            get_status,
            get_settings,
            set_setting,
            get_insights,
            connect_groq,
            set_ai_enabled,
            check_update,
            open_release_page,
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
            library::list_audio_devices,
            library::start_meeting,
            library::meeting_status,
            library::stop_meeting,
            library::list_meetings,
            library::get_meeting,
            library::rename_meeting,
            library::edit_meeting_line,
            library::rewrite_meeting_notes,
            library::transcribe_saved_meeting,
            library::export_meeting,
            library::reveal_file,
            library::delete_meeting,
            library::delete_all_meetings,
            diagnostics_info,
            delete_logs,
            export_diagnostics
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
            diag::init(
                &data_dir,
                db::get_setting(&conn, "diagnostics").as_deref() != Some("off"),
            );
            diag::log(
                "start",
                &format!(
                    "{} {} started (GPU build: {})",
                    app_core::brand::APP_NAME,
                    env!("CARGO_PKG_VERSION"),
                    cfg!(feature = "gpu")
                ),
            );

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
            {
                // Before the speech engine starts: owner's recommended defaults, best model.
                let state = handle.state::<AppState>();
                heal_gpu_setting(&state);
                apply_recommended_defaults(&state);
                auto_pick_model(&state);
                library::recover_interrupted(&state);
            }
            overlay::prepare(&handle);
            controller::spawn(handle.clone(), ctl_rx, recorder);
            {
                // Retention also runs every 6 hours for an app that is never closed.
                let h = handle.clone();
                std::thread::spawn(move || loop {
                    std::thread::sleep(std::time::Duration::from_secs(6 * 3600));
                    let sent = h
                        .state::<AppState>()
                        .ctl
                        .lock()
                        .map(|tx| tx.send(Msg::Retention).is_ok())
                        .unwrap_or(false);
                    if !sent {
                        break;
                    }
                });
            }

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
            if !quiet_test() {
                tray.build(app)?;
            }
            // Opt-in update check at start-up; the UI shows a notice if a newer version exists.
            let auto_check = handle.state::<AppState>().setting("check_updates") == "on"
                && std::env::var_os("APP_SELFTEST_WAV").is_none();
            if auto_check {
                let h = handle.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(20));
                    let info =
                        update::check(app_core::brand::REPOSITORY, env!("CARGO_PKG_VERSION"));
                    if info.newer {
                        let _ = h.emit("update-available", &info);
                    }
                });
            }
            // The window starts hidden (tauri.conf.json) so a test run never flashes it.
            // "--background" starts in the tray only (used after an install or at sign-in).
            let background = std::env::args().any(|a| a == "--background");
            if std::env::var_os("APP_SELFTEST_WAV").is_none() && !background {
                show_main(&handle);
            }
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

#[cfg(test)]
mod tests {
    use super::remove_app_file;

    #[test]
    fn only_files_inside_the_data_folder_are_deleted() {
        let root = std::env::temp_dir().join(format!("rm-guard-{}", std::process::id()));
        let data = root.join("data");
        let outside = root.join("outside.wav");
        std::fs::create_dir_all(data.join("meetings")).unwrap();
        let inside = data.join("meetings").join("mic.wav");
        std::fs::write(&inside, b"x").unwrap();
        std::fs::write(&outside, b"y").unwrap();

        assert!(remove_app_file(&data, inside.to_str().unwrap()).is_ok());
        assert!(!inside.exists(), "file inside the data folder is deleted");
        assert!(
            remove_app_file(&data, inside.to_str().unwrap()).is_ok(),
            "already gone counts as deleted"
        );
        let err = remove_app_file(&data, outside.to_str().unwrap()).unwrap_err();
        assert!(err.contains("outside"), "{err}");
        assert!(
            outside.exists(),
            "file outside the data folder is never deleted"
        );
        let sneaky = data.join("..").join("outside.wav");
        assert!(remove_app_file(&data, sneaky.to_str().unwrap()).is_err());
        assert!(outside.exists());
        let _ = std::fs::remove_dir_all(root);
    }
}

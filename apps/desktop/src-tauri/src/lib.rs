//! Desktop app: system-wide dictation for Windows.

mod audio;
mod controller;
pub mod credentials;
mod db;
mod hotkey;
mod insert;
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
    ("language", "auto"),
    ("level", "LIGHT"),
    ("use_gpu", "true"),
    ("keep_mic_open", "false"),
    ("spoken_commands", "true"),
    ("save_audio", "true"),
    ("audio_retention_days", "7"),
    ("ui_scale", "125"),
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
    let cfg = ai_config(&state).ok_or("AI clean-up is not set up yet.")?;
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
                    app_core::llm::accept(
                        &r,
                        &prepared,
                        sample,
                        &[],
                        app_core::pipeline::Level::Light,
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
        ],
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
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main(app)
        }))
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
            learn_terms
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

            app.manage(AppState {
                db: Mutex::new(conn),
                data_dir,
                ctl: Mutex::new(ctl_tx),
                hook_error: Mutex::new(hook_error),
            });
            overlay::prepare(&handle);
            controller::spawn(handle.clone(), ctl_rx);

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

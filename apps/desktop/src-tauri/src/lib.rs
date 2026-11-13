//! Desktop app: system-wide dictation for Windows.

mod audio;
mod controller;
mod db;
mod hotkey;
mod insert;
mod models;
mod overlay;

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

fn default_setting(key: &str) -> &'static str {
    SETTINGS
        .iter()
        .find(|(k, _)| *k == key)
        .map_or("", |(_, v)| v)
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
            transcribe_file
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

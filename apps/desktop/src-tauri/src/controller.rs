//! The dictation controller: one thread that owns the speech engine and runs each
//! dictation from key-down to inserted text.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use app_asr::{audio as asr_audio, Options, WhisperEngine};
use app_core::dictionary::Dictionary;
use app_core::pipeline::{self, Level, Settings};
use app_core::snippets::SnippetSet;

use crate::audio::Recorder;
use crate::hotkey::{self, Action};
use crate::insert::{self, app_rules, Outcome, Target};
use crate::overlay::{self, PillState};
use crate::{db, models, AppState};

/// Hands-free recording warns at 19 minutes and stops (and transcribes) at 20.
const HANDS_FREE_WARN: Duration = Duration::from_secs(19 * 60);
const HANDS_FREE_CAP: Duration = Duration::from_secs(20 * 60);

pub enum Msg {
    Key(Action),
    SettingsChanged,
    UnloadModel,
    File {
        path: PathBuf,
        insert: bool,
        delay_ms: u64,
        reply: Sender<Result<DictationResult, String>>,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct DictationResult {
    pub id: String,
    pub raw_text: String,
    pub final_text: String,
    pub outcome: Option<Outcome>,
    pub audio_ms: u64,
    pub transcribe_ms: u64,
    pub engine: String,
    pub target_process: String,
}

struct Session {
    target: Option<Target>,
    started: Instant,
    hands_free: bool,
}

struct Loaded {
    id: String,
    gpu: bool,
    label: String,
    engine: WhisperEngine,
}

struct Controller {
    app: AppHandle,
    recorder: Recorder,
    session: Option<Session>,
    engine: Option<Loaded>,
    /// Bumped on every pill update so a delayed "hide" never hides a newer message.
    pill_generation: Arc<AtomicU64>,
}

pub fn spawn(app: AppHandle, rx: Receiver<Msg>) {
    std::thread::Builder::new()
        .name("controller".into())
        .spawn(move || {
            let recorder = Recorder::spawn();
            let mut c = Controller {
                app,
                recorder,
                session: None,
                engine: None,
                pill_generation: Arc::new(AtomicU64::new(0)),
            };
            c.apply_settings();
            c.expire_old_audio();
            c.run(rx);
        })
        .expect("controller thread");
}

impl Controller {
    fn state(&self) -> tauri::State<'_, AppState> {
        self.app.state::<AppState>()
    }

    fn apply_settings(&mut self) {
        self.recorder
            .set_keep_open(self.state().setting("keep_mic_open") == "true");
    }

    fn expire_old_audio(&self) {
        let days: i64 = self
            .state()
            .setting("audio_retention_days")
            .parse()
            .unwrap_or(7);
        let before = (chrono::Utc::now() - chrono::Duration::days(days)).to_rfc3339();
        let paths = self
            .state()
            .db
            .lock()
            .map(|c| db::expire_audio(&c, &before))
            .unwrap_or_default();
        for p in paths {
            let _ = std::fs::remove_file(p);
        }
    }

    fn pill(&self, state: PillState, target_hwnd: Option<isize>, hide_after: Option<Duration>) {
        let generation = self.pill_generation.fetch_add(1, Ordering::SeqCst) + 1;
        overlay::emit(&self.app, &state);
        overlay::show(&self.app, target_hwnd);
        if let Some(delay) = hide_after {
            let (app, counter) = (self.app.clone(), Arc::clone(&self.pill_generation));
            std::thread::spawn(move || {
                std::thread::sleep(delay);
                if counter.load(Ordering::SeqCst) == generation {
                    overlay::hide(&app);
                    overlay::emit(
                        &app,
                        &PillState {
                            state: "hidden",
                            ..Default::default()
                        },
                    );
                }
            });
        }
    }

    fn hide_pill(&self) {
        self.pill_generation.fetch_add(1, Ordering::SeqCst);
        overlay::hide(&self.app);
        overlay::emit(
            &self.app,
            &PillState {
                state: "hidden",
                ..Default::default()
            },
        );
    }

    fn message(&self, state: &'static str, message: impl Into<String>, target: Option<isize>) {
        let secs = if state == "error" { 4 } else { 2 };
        self.pill(
            PillState {
                state,
                message: message.into(),
                ..Default::default()
            },
            target,
            Some(Duration::from_secs(secs)),
        );
    }

    fn engine_label(&self) -> String {
        let id = self.state().setting("model");
        let name = models::find(&id)
            .map_or(id.as_str(), |m| {
                m.label.split(" (").next().unwrap_or(m.label)
            })
            .to_string();
        let gpu = cfg!(feature = "gpu") && self.state().setting("use_gpu") == "true";
        format!("Local - {name}{}", if gpu { " (GPU)" } else { "" })
    }

    fn run(&mut self, rx: Receiver<Msg>) {
        loop {
            let msg = if self.session.is_some() {
                match rx.recv_timeout(Duration::from_millis(70)) {
                    Ok(m) => Some(m),
                    Err(RecvTimeoutError::Timeout) => None,
                    Err(RecvTimeoutError::Disconnected) => return,
                }
            } else {
                match rx.recv() {
                    Ok(m) => Some(m),
                    Err(_) => return,
                }
            };
            match msg {
                None => self.tick(),
                Some(Msg::Key(action)) => self.on_action(action),
                Some(Msg::SettingsChanged) => self.apply_settings(),
                Some(Msg::UnloadModel) => self.engine = None,
                Some(Msg::File {
                    path,
                    insert,
                    delay_ms,
                    reply,
                }) => {
                    let result = self.run_file(&path, insert, delay_ms);
                    let _ = reply.send(result);
                }
            }
        }
    }

    fn on_action(&mut self, action: Action) {
        match action {
            Action::PttStart => self.begin(false),
            Action::HandsFreeStart => self.begin(true),
            Action::HandsFreeLock => {
                if let Some(s) = self.session.as_mut() {
                    s.hands_free = true;
                }
            }
            Action::PttStop | Action::HandsFreeStop => self.finish(),
            Action::Cancel => {
                if self.session.take().is_some() {
                    self.recorder.cancel();
                    self.hide_pill();
                }
            }
            Action::PasteLast => self.paste_last(),
            Action::CopyLast => {
                let text = self.state().db.lock().ok().and_then(|c| db::last_text(&c));
                match text.map(|t| insert::clipboard::write_text_private(&t)) {
                    Some(Ok(_)) => self.message("info", "Copied last dictation", None),
                    Some(Err(e)) => self.message("error", e, None),
                    None => self.message("info", "Nothing dictated yet", None),
                }
            }
        }
    }

    fn begin(&mut self, hands_free: bool) {
        if self.session.is_some() {
            return;
        }
        let target = insert::target::foreground();
        let hwnd = target.as_ref().map(|t| t.hwnd);
        if let Err(e) = self.recorder.start() {
            hotkey::force_idle();
            self.message("error", e, hwnd);
            return;
        }
        self.session = Some(Session {
            target,
            started: Instant::now(),
            hands_free,
        });
        self.pill(
            PillState {
                state: "listening",
                hands_free,
                engine: self.engine_label(),
                ..Default::default()
            },
            hwnd,
            None,
        );
    }

    /// Runs about 14 times a second while recording: level meter, timer, time cap.
    fn tick(&mut self) {
        let Some(session) = self.session.as_ref() else {
            return;
        };
        let elapsed = session.started.elapsed();
        if self.recorder.device_failed() {
            // Keep what was captured: transcribe it rather than throw it away.
            self.finish();
            return;
        }
        if session.hands_free && elapsed >= HANDS_FREE_CAP {
            hotkey::force_idle();
            self.finish();
            return;
        }
        let warn = session.hands_free && elapsed >= HANDS_FREE_WARN;
        let state = PillState {
            state: "listening",
            message: if warn {
                "Stopping at 20:00".into()
            } else {
                String::new()
            },
            level: self.recorder.level(),
            seconds: elapsed.as_secs(),
            hands_free: session.hands_free,
            engine: self.engine_label(),
        };
        overlay::emit(&self.app, &state);
    }

    fn finish(&mut self) {
        let Some(session) = self.session.take() else {
            return;
        };
        let hwnd = session.target.as_ref().map(|t| t.hwnd);
        let samples = match self.recorder.stop() {
            Ok(s) => s,
            Err(e) => {
                self.message("error", e, hwnd);
                return;
            }
        };
        match self.process(samples, session.target, true) {
            Ok(result) => match &result.outcome {
                None => self.message("info", "Didn't hear anything", hwnd),
                Some(o) if o.inserted() => self.message("done", o.message(), hwnd),
                Some(o @ Outcome::Copied { .. }) => self.message("info", o.message(), hwnd),
                Some(o) => self.message("error", o.message(), hwnd),
            },
            Err(e) => self.message("error", e, hwnd),
        }
    }

    fn ensure_engine(&mut self) -> Result<(), String> {
        let id = self.state().setting("model");
        let gpu = cfg!(feature = "gpu") && self.state().setting("use_gpu") == "true";
        if self
            .engine
            .as_ref()
            .is_some_and(|e| e.id == id && e.gpu == gpu)
        {
            return Ok(());
        }
        let info =
            models::find(&id).ok_or("Unknown speech model selected. Choose one in Settings.")?;
        let path = models::path_for(&self.state().models_dir(), info);
        if !path.is_file() {
            return Err("No speech model installed. Open the app and download one.".into());
        }
        self.engine = None; // free the old model before loading the new one
        let engine = WhisperEngine::load(&path, gpu)?;
        let label = self.engine_label();
        self.engine = Some(Loaded {
            id,
            gpu,
            label,
            engine,
        });
        Ok(())
    }

    /// Audio -> text -> cleanup -> (optional) insertion -> history.
    fn process(
        &mut self,
        mut samples: Vec<f32>,
        target: Option<Target>,
        do_insert: bool,
    ) -> Result<DictationResult, String> {
        let hwnd = target.as_ref().map(|t| t.hwnd);
        let audio_ms = samples.len() as u64 * 1000 / u64::from(app_asr::SAMPLE_RATE);
        self.pill(
            PillState {
                state: "transcribing",
                message: "Transcribing...".into(),
                engine: self.engine_label(),
                ..Default::default()
            },
            hwnd,
            None,
        );
        self.ensure_engine()?;
        // Quiet speech ("whisper mode"): gentle gain only, no noise reduction.
        asr_audio::normalise_quiet(&mut samples, 0.05, 8.0);

        let language = self.state().setting("language");
        let options = Options {
            language: (language != "auto").then(|| language.clone()),
            ..Default::default()
        };
        let loaded = self.engine.as_ref().ok_or("speech engine not loaded")?;
        let transcript = loaded.engine.transcribe(&samples, &options)?;
        let engine_label = loaded.label.clone();
        let raw = transcript.text.trim().to_string();

        let id = uuid::Uuid::new_v4().to_string();
        let process_name = target
            .as_ref()
            .map(|t| t.process.clone())
            .unwrap_or_default();
        if raw.is_empty() {
            return Ok(DictationResult {
                id,
                raw_text: raw,
                final_text: String::new(),
                outcome: None,
                audio_ms,
                transcribe_ms: transcript.elapsed_ms,
                engine: engine_label,
                target_process: process_name,
            });
        }

        self.pill(
            PillState {
                state: "polishing",
                message: "Polishing...".into(),
                ..Default::default()
            },
            hwnd,
            None,
        );
        let level = match self.state().setting("level").as_str() {
            "LITERAL" => Level::Literal,
            "PROFESSIONAL" => Level::Professional,
            _ => Level::Light,
        };
        let settings = Settings {
            level,
            spoken_commands: self.state().setting("spoken_commands") == "true",
        };
        let cleaned = pipeline::run(
            &raw,
            &settings,
            &Dictionary::default(),
            &SnippetSet::default(),
        );
        let final_text = cleaned.text;

        let outcome = if do_insert {
            Some(match &target {
                Some(t) => insert::insert(&final_text, t),
                None => match insert::clipboard::write_text_private(&final_text) {
                    Ok(_) => Outcome::Copied {
                        reason: insert::CopyReason::FocusChanged,
                    },
                    Err(message) => Outcome::Failed { message },
                },
            })
        } else {
            None
        };

        // Keep the audio for replay / re-transcription (retention applies).
        let audio_path = if self.state().setting("save_audio") == "true" {
            let path = self.state().audio_dir().join(format!("{id}.wav"));
            asr_audio::write_wav_16k_mono(&path, &samples)
                .ok()
                .map(|_| path.to_string_lossy().into_owned())
        } else {
            None
        };
        let rule = target
            .as_ref()
            .map(|t| app_rules::rule_for(&t.process, &t.title));
        let record = db::Dictation {
            id: id.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            app_process: process_name.clone(),
            app_title: target.as_ref().map(|t| t.title.clone()).unwrap_or_default(),
            app_category: rule
                .map(|r| format!("{:?}", r.category))
                .unwrap_or_default(),
            language,
            engine: engine_label.clone(),
            level: format!("{level:?}").to_uppercase(),
            raw_text: raw.clone(),
            final_text: final_text.clone(),
            inserted: outcome.as_ref().is_some_and(Outcome::inserted),
            insert_method: outcome
                .as_ref()
                .map(|o| o.method().to_string())
                .unwrap_or_default(),
            duration_ms: audio_ms as i64,
            audio_path,
            word_count: final_text.split_whitespace().count() as i64,
            error: match &outcome {
                Some(Outcome::Failed { message }) => Some(message.clone()),
                _ => None,
            },
        };
        if let Ok(conn) = self.state().db.lock() {
            let _ = db::insert_dictation(&conn, &record);
        }
        let _ = self.app.emit("dictation-added", &record.id);

        Ok(DictationResult {
            id,
            raw_text: raw,
            final_text,
            outcome,
            audio_ms,
            transcribe_ms: transcript.elapsed_ms,
            engine: engine_label,
            target_process: process_name,
        })
    }

    fn paste_last(&mut self) {
        let target = insert::target::foreground();
        let hwnd = target.as_ref().map(|t| t.hwnd);
        let text = self.state().db.lock().ok().and_then(|c| db::last_text(&c));
        match (text, target) {
            (Some(text), Some(target)) => {
                let outcome = insert::insert(&text, &target);
                let state = if outcome.inserted() { "done" } else { "info" };
                self.message(state, outcome.message(), hwnd);
            }
            (None, _) => self.message("info", "Nothing dictated yet", hwnd),
            (_, None) => self.message("info", "No window to paste into", hwnd),
        }
    }

    fn run_file(
        &mut self,
        path: &std::path::Path,
        do_insert: bool,
        delay_ms: u64,
    ) -> Result<DictationResult, String> {
        let samples = asr_audio::read_wav_16k_mono(path)?;
        if delay_ms > 0 {
            std::thread::sleep(Duration::from_millis(delay_ms));
        }
        let target = if do_insert {
            insert::target::foreground()
        } else {
            None
        };
        let hwnd = target.as_ref().map(|t| t.hwnd);
        let result = self.process(samples, target, do_insert);
        match &result {
            Ok(r) => match &r.outcome {
                Some(o) if o.inserted() => self.message("done", o.message(), hwnd),
                Some(o) => self.message("info", o.message(), hwnd),
                None if r.final_text.is_empty() => {
                    self.message("info", "Didn't hear anything", hwnd)
                }
                None => self.hide_pill(),
            },
            Err(e) => self.message("error", e.clone(), hwnd),
        }
        result
    }
}

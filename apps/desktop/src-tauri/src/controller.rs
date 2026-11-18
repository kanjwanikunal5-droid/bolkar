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
use app_core::command::{self, Command};
use app_core::dictionary::{Dictionary, Term};
use app_core::language::{self, LanguageMode, RecogniserLanguage, RecognitionPlan};
use app_core::llm;
use app_core::pipeline::{self, Level, Settings};
use app_core::snippets::{Snippet, SnippetSet};

use crate::audio::Recorder;
use crate::hotkey::{self, Action};
use crate::insert::{self, app_rules, Outcome, Target};
use crate::overlay::{self, PillState};
use crate::{db, models, providers, AppState};

/// The AI clean-up must answer within this time, or the rules are used instead.
const AI_TIMEOUT: Duration = Duration::from_millis(2500);
/// A Command Mode edit may rewrite a whole paragraph; it gets longer.
const AI_COMMAND_TIMEOUT: Duration = Duration::from_secs(8);
/// Shown on the pill while Command Mode listens.
const COMMAND_HINT: &str = "Say what to do, then Alt+Shift+C";
/// Cloud speech recognition normally answers in under half a second; after this, use the
/// model on this computer instead.
const CLOUD_ASR_TIMEOUT: Duration = Duration::from_secs(6);

/// Hands-free recording warns at 19 minutes and stops (and transcribes) at 20.
const HANDS_FREE_WARN: Duration = Duration::from_secs(19 * 60);
const HANDS_FREE_CAP: Duration = Duration::from_secs(20 * 60);

pub enum Msg {
    Key(Action),
    SettingsChanged,
    UnloadModel,
    /// Transcribe meeting tracks: (WAV path, source label such as "Microphone"), each from the
    /// given sample position. While recording (`final_pass` false) only complete pieces are
    /// taken; at the end everything left is.
    TranscribeMeeting {
        tracks: Vec<(PathBuf, String)>,
        from: Vec<usize>,
        final_pass: bool,
        reply: Sender<Result<MeetingChunk, String>>,
    },
    /// Apply the retention settings (at start-up and every few hours).
    Retention,
    File {
        path: PathBuf,
        insert: bool,
        delay_ms: u64,
        /// Treat the recording as a Command Mode instruction for the selection in front.
        command: bool,
        reply: Sender<Result<DictationResult, String>>,
    },
}

/// New transcript lines of a meeting and where each track continues next time.
#[derive(Debug, Clone, Default)]
pub struct MeetingChunk {
    pub segments: Vec<app_core::summary::Segment>,
    pub next: Vec<usize>,
    /// False when live transcription is not possible (no graphics card): the transcript is
    /// then made after the meeting.
    pub live_ok: bool,
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
    /// Command Mode: the selected text the spoken instruction applies to.
    command: Option<String>,
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

pub fn spawn(app: AppHandle, rx: Receiver<Msg>, recorder: Recorder) {
    std::thread::Builder::new()
        .name("controller".into())
        .spawn(move || {
            let mut c = Controller {
                app,
                recorder,
                session: None,
                engine: None,
                pill_generation: Arc::new(AtomicU64::new(0)),
            };
            c.apply_settings();
            c.apply_retention();
            // Load and warm up the local model now, so the first dictation is not the slow one.
            // (Only on the graphics card: there it prepares the GPU programs; on the processor a
            // warm-up of a large model takes long and delays the first key press.)
            if c.ensure_engine().is_ok() {
                if let Some(loaded) = c.engine.as_ref().filter(|l| l.gpu) {
                    loaded.engine.warm_up();
                }
            }
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

    /// Deletes what the retention settings say is due. Files are deleted first and their
    /// database entry is cleared only when the file is really gone; failures are recorded,
    /// never reported as deleted. Only files inside the app's data folder are touched. A
    /// meeting being recorded has no database entry yet, so it is never affected.
    fn apply_retention(&self) {
        let state = self.state();
        let data_dir = state.data_dir.clone();
        let days = |key: &str| state.setting(key).trim().parse::<i64>().unwrap_or(0);
        let before = |d: i64| (chrono::Utc::now() - chrono::Duration::days(d)).to_rfc3339();
        let mut lines = Vec::new();

        let d = days("audio_retention_days");
        if d > 0 {
            let due = state
                .db
                .lock()
                .map(|c| db::dictation_audio_before(&c, &before(d)))
                .unwrap_or_default();
            let (mut ok, mut failed) = (0, Vec::new());
            for (id, path) in due {
                match crate::remove_app_file(&data_dir, &path) {
                    Ok(()) => {
                        if let Ok(c) = state.db.lock() {
                            let _ = db::clear_dictation_audio(&c, &id);
                        }
                        ok += 1;
                    }
                    Err(e) => failed.push(e),
                }
            }
            if ok + failed.len() > 0 {
                lines.push(format!(
                    "dictation audio older than {d} days: {ok} deleted, {} failed {failed:?}",
                    failed.len()
                ));
            }
        }
        let d = days("meeting_audio_retention_days");
        if d > 0 {
            let due = state
                .db
                .lock()
                .map(|c| db::meeting_audio_before(&c, &before(d)))
                .unwrap_or_default();
            let (mut ok, mut failed) = (0, Vec::new());
            for (id, paths) in due {
                let errors: Vec<String> = paths
                    .iter()
                    .filter_map(|p| crate::remove_app_file(&data_dir, p).err())
                    .collect();
                if errors.is_empty() {
                    if let Ok(c) = state.db.lock() {
                        let _ = db::clear_meeting_audio(&c, &id);
                    }
                    let _ = std::fs::remove_dir(data_dir.join("meetings").join(&id));
                    ok += 1;
                } else {
                    failed.extend(errors);
                }
            }
            if ok + failed.len() > 0 {
                lines.push(format!("meeting audio older than {d} days: {ok} meetings cleared, {} files failed {failed:?}", failed.len()));
            }
        }
        let d = days("history_retention_days");
        if d > 0 {
            let due = state
                .db
                .lock()
                .map(|c| db::dictations_before(&c, &before(d)))
                .unwrap_or_default();
            let mut ok = 0;
            for id in due {
                let audio = state
                    .db
                    .lock()
                    .ok()
                    .and_then(|c| db::delete_dictation(&c, &id).ok())
                    .flatten();
                if let Some(p) = audio {
                    let _ = crate::remove_app_file(&data_dir, &p);
                }
                ok += 1;
            }
            if ok > 0 {
                lines.push(format!(
                    "dictation history older than {d} days: {ok} deleted"
                ));
            }
        }
        let d = days("meeting_retention_days");
        if d > 0 {
            let due = state
                .db
                .lock()
                .map(|c| db::meetings_before(&c, &before(d)))
                .unwrap_or_default();
            let mut ok = 0;
            for id in due {
                if crate::library::delete_meeting_everywhere(&state, &id).is_ok() {
                    ok += 1;
                }
            }
            if ok > 0 {
                lines.push(format!("meetings older than {d} days: {ok} deleted"));
            }
        }
        if let Ok(c) = state.db.lock() {
            for l in lines {
                db::log_event(&c, "retention", &l);
            }
        };
    }

    fn pill(&self, mut state: PillState, target_hwnd: Option<isize>, hide_after: Option<Duration>) {
        let quiet = crate::quiet_test();
        state.sounds = !quiet && self.state().setting("sounds") != "off";
        let generation = self.pill_generation.fetch_add(1, Ordering::SeqCst) + 1;
        overlay::emit(&self.app, &state);
        if !quiet {
            overlay::show(&self.app, target_hwnd);
        }
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
        let message = message.into();
        if state == "error" {
            // Error texts describe what went wrong, never what was said.
            crate::diag::log("error", &message);
        }
        if let Some(path) = std::env::var_os("APP_TEST_LOG") {
            if std::env::var_os("APP_DATA_DIR").is_some() {
                use std::io::Write;
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                {
                    let _ = writeln!(
                        f,
                        "{} pill {state}: {message}",
                        chrono::Utc::now().to_rfc3339()
                    );
                }
            }
        }
        let secs = if state == "error" { 4 } else { 2 };
        self.pill(
            PillState {
                state,
                message,
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
                Some(Msg::TranscribeMeeting {
                    tracks,
                    from,
                    final_pass,
                    reply,
                }) => {
                    let _ = reply.send(self.transcribe_meeting(&tracks, &from, final_pass));
                }
                Some(Msg::Retention) => self.apply_retention(),
                Some(Msg::File {
                    path,
                    insert,
                    delay_ms,
                    command,
                    reply,
                }) => {
                    let result = if command {
                        self.run_command_file(&path)
                    } else {
                        self.run_file(&path, insert, delay_ms)
                    };
                    let _ = reply.send(result);
                }
            }
        }
    }

    fn on_action(&mut self, action: Action) {
        // Test runs only: a trace of shortcut actions, for automated end-to-end checks.
        if let Some(path) = std::env::var_os("APP_TEST_LOG") {
            if std::env::var_os("APP_DATA_DIR").is_some() {
                use std::io::Write;
                if let Ok(mut f) = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                {
                    let _ = writeln!(f, "{} {action:?}", chrono::Utc::now().to_rfc3339());
                }
            }
        }
        match action {
            Action::PttStart => self.begin(false),
            Action::HandsFreeStart => self.begin(true),
            Action::HandsFreeLock => {
                if let Some(s) = self.session.as_mut() {
                    s.hands_free = true;
                }
            }
            Action::PttStop | Action::HandsFreeStop | Action::CommandStop => self.finish(),
            Action::CommandStart => self.begin_command(),
            Action::Cancel => {
                if self.session.take().is_some() {
                    self.recorder.cancel();
                    self.hide_pill();
                }
            }
            Action::PasteLast => self.paste_last(),
            Action::CycleLanguage => {
                let next = match self.state().setting("language").as_str() {
                    "en-hinglish" => "en",
                    "en" => "hinglish",
                    _ => "en-hinglish",
                };
                if let Ok(conn) = self.state().db.lock() {
                    let _ = db::set_setting(&conn, "language", next);
                }
                let label = match next {
                    "en" => "English only",
                    "hinglish" => "Hinglish only",
                    _ => "English + Hinglish (auto)",
                };
                let _ = self.app.emit("dictation-added", "");
                self.message("info", label, None);
            }
            Action::OpenScratchpad => {
                if let Some(w) = self.app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.unminimize();
                    let _ = w.set_focus();
                }
                let _ = self.app.emit("open-scratchpad", ());
            }
            Action::FixLast => {
                // Opens the app on the last dictation, ready to correct it.
                if let Some(w) = self.app.get_webview_window("main") {
                    let _ = w.show();
                    let _ = w.unminimize();
                    let _ = w.set_focus();
                }
                let _ = self.app.emit("fix-last", ());
            }
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
            command: None,
        });
        self.pill(
            PillState {
                state: "listening",
                hands_free,
                engine: String::new(),
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
            } else if session.command.is_some() {
                COMMAND_HINT.into()
            } else {
                String::new()
            },
            level: self.recorder.level(),
            seconds: elapsed.as_secs(),
            hands_free: session.hands_free,
            engine: String::new(),
            // Level updates while listening never replay the start sound.
            sounds: false,
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
        if let Some(selection) = session.command {
            match self.run_command(samples, session.target, &selection) {
                Ok(r) => match &r.outcome {
                    Some(o) if o.inserted() => self.hide_pill(),
                    Some(o) => self.message("info", o.message(), hwnd),
                    None => self.message("info", "Didn't hear an instruction", hwnd),
                },
                Err(e) => self.message("error", e, hwnd),
            }
            return;
        }
        // Test runs with a trace (APP_TEST_LOG + own data folder) never type into other windows.
        let test_run = std::env::var_os("APP_TEST_LOG").is_some()
            && std::env::var_os("APP_DATA_DIR").is_some();
        match self.process(samples, session.target, !test_run, session.hands_free) {
            Ok(result) => match &result.outcome {
                None => self.message("info", "Didn't hear anything", hwnd),
                // Text went in: no pop-up, the pill just disappears.
                Some(o) if o.inserted() => self.hide_pill(),
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

    /// Meeting tracks -> time-stamped lines, on this computer. Each track is read from where
    /// the last pass stopped and split at quiet moments into pieces of at most ~30 s; silent
    /// pieces are skipped (no invented text). Every line keeps Whisper's own time stamps plus
    /// the piece's offset, so a citation points at the sentence, not at a 30 s window.
    ///
    /// Language: the Settings mode is mapped to a real recogniser language for every piece
    /// ("en" or "hi"); "en-hinglish" and "hinglish" are never passed to Whisper.
    fn transcribe_meeting(
        &mut self,
        tracks: &[(PathBuf, String)],
        from: &[usize],
        final_pass: bool,
    ) -> Result<MeetingChunk, String> {
        let mut next: Vec<usize> = (0..tracks.len())
            .map(|i| from.get(i).copied().unwrap_or(0))
            .collect();
        // A dictation is in progress: the live pass waits for the next round.
        if !final_pass && self.session.is_some() {
            return Ok(MeetingChunk {
                segments: Vec::new(),
                next,
                live_ok: true,
            });
        }
        self.ensure_engine()?;
        let gpu = self.engine.as_ref().is_some_and(|l| l.gpu);
        // Without a graphics card a live pass would slow everything down: after the meeting only.
        if !final_pass && !gpu {
            return Ok(MeetingChunk {
                segments: Vec::new(),
                next,
                live_ok: false,
            });
        }
        let mode = LanguageMode::from_setting(&self.state().setting("language"));
        let plan = language::plan(mode);
        let (dictionary, _) = self.vocabulary();
        let hint = dictionary.recogniser_hint(400);
        let rate = u64::from(app_asr::SAMPLE_RATE);
        let mut segments = Vec::new();
        for (i, (path, speaker)) in tracks.iter().enumerate() {
            if !path.is_file() {
                continue;
            }
            let total = asr_audio::wav_len_16k(path)?;
            let start = next[i];
            if total <= start {
                continue;
            }
            let region = asr_audio::read_wav_range_16k_mono(path, start, total - start)?;
            let (pieces, consumed) = crate::meeting::pieces_to_take(&region, final_pass);
            for (offset, piece) in pieces {
                if !asr_audio::has_signal(piece) {
                    continue;
                }
                let loaded = self.engine.as_ref().ok_or("speech engine not loaded")?;
                let detected = match plan {
                    RecognitionPlan::DetectPerPiece => loaded
                        .engine
                        .language_probabilities(piece, &["en", "hi"])
                        .ok()
                        .map(|p| app_asr::is_hinglish(p[0], p[1])),
                    RecognitionPlan::Fixed(_) => None,
                };
                let lang = language::choose(plan, detected);
                let options = match lang {
                    // Hindi recognition steered to Roman letters by an example; no glossary
                    // (an English-only hint pushes Whisper into translating).
                    RecogniserLanguage::HindiRoman => Options {
                        language: Some(lang.code().into()),
                        initial_prompt: Some(llm::ROMAN_HINGLISH_EXAMPLE.into()),
                        ..Default::default()
                    },
                    RecogniserLanguage::English => Options {
                        language: Some(lang.code().into()),
                        initial_prompt: (!hint.is_empty()).then(|| hint.clone()),
                        ..Default::default()
                    },
                };
                let t = loaded.engine.transcribe(piece, &options)?;
                let base_ms = (start + offset) as u64 * 1000 / rate;
                let piece_ms = piece.len() as u64 * 1000 / rate;
                for (s, e, text) in app_asr::guard::kept_segments(&t.segments) {
                    let text =
                        if lang == RecogniserLanguage::HindiRoman && !llm::has_devanagari(&text) {
                            app_core::hinglish::normalise(&text)
                        } else {
                            text
                        };
                    if text.trim().is_empty() {
                        continue;
                    }
                    let s = (s.max(0) as u64).min(piece_ms);
                    let e = (e.max(0) as u64).clamp(s, piece_ms);
                    segments.push(app_core::summary::Segment {
                        id: String::new(),
                        start_ms: base_ms + s,
                        end_ms: base_ms + e,
                        speaker: speaker.clone(),
                        text: text.trim().to_string(),
                    });
                }
            }
            next[i] = start + consumed;
        }
        self.romanise_segments(&mut segments, mode);
        Ok(MeetingChunk {
            segments,
            next,
            live_ok: gpu,
        })
    }

    /// Hinglish lines that came back in Devanagari are written in Roman letters, as in
    /// dictation, when AI clean-up is set up. Without it they are kept as they are (never
    /// translated), and the diagnostics log notes it.
    fn romanise_segments(&self, segments: &mut [app_core::summary::Segment], mode: LanguageMode) {
        if !mode.allows_hinglish() {
            return;
        }
        let mut kept = 0;
        for s in segments.iter_mut().filter(|s| llm::has_devanagari(&s.text)) {
            match self
                .ai_text(
                    &llm::roman_spelling_prompt(),
                    &s.text,
                    "Roman Hinglish (meeting)",
                )
                .and_then(|content| llm::parse_transliteration(&content, &s.text))
            {
                Ok(roman) => s.text = app_core::hinglish::normalise(&roman),
                Err(_) => kept += 1,
            }
        }
        if kept > 0 {
            crate::diag::log(
                "meeting",
                &format!("{kept} line(s) kept in Devanagari (AI clean-up off or failed)"),
            );
        }
    }

    /// Hinglish spelling polish (bahut, karunga, ...) and obvious mis-hearings; checked by
    /// `llm::parse_polish`, which rejects rewrites.
    fn polish_hinglish(&self, text: &str) -> Result<String, String> {
        self.ai_text(&llm::hinglish_polish_prompt(), text, "Hinglish spelling")
            .and_then(|content| llm::parse_polish(&content, text))
    }

    /// Sends dictation text to the user's AI provider and logs the upload.
    fn ai_text(&self, system: &str, text: &str, purpose: &str) -> Result<String, String> {
        let user = serde_json::json!({ "text": text }).to_string();
        self.ai_raw(system, &user, purpose, AI_TIMEOUT)
    }

    fn ai_raw(
        &self,
        system: &str,
        user: &str,
        purpose: &str,
        timeout: Duration,
    ) -> Result<String, String> {
        let cfg = crate::ai_config(&self.state()).ok_or("AI clean-up is off")?;
        let result = providers::chat(&cfg, system, user, timeout);
        if let Ok(conn) = self.state().db.lock() {
            let outcome = result
                .as_ref()
                .map(|_| "ok".to_string())
                .unwrap_or_else(|e| e.to_string());
            db::log_event(
                &conn,
                "upload",
                &format!(
                    "{} ({}): dictation text for {purpose}, {} bytes, {outcome}",
                    cfg.provider,
                    cfg.model,
                    system.len() + user.len()
                ),
            );
        }
        result.map(|a| a.content).map_err(|e| e.to_string())
    }

    /// The user's dictionary and snippets, read fresh for each dictation.
    fn vocabulary(&self) -> (Dictionary, SnippetSet) {
        let state = self.state();
        let Ok(conn) = state.db.lock() else {
            return (Dictionary::default(), SnippetSet::default());
        };
        let terms = db::list_terms(&conn)
            .unwrap_or_default()
            .into_iter()
            .map(|t| Term {
                term: t.term,
                spoken_variants: t.spoken_variants,
                auto_replace: t.auto_replace,
            })
            .collect();
        let snippets = db::list_snippets(&conn)
            .unwrap_or_default()
            .into_iter()
            .map(|s| Snippet {
                trigger: s.trigger,
                body: s.body,
            })
            .collect();
        (Dictionary::new(terms), SnippetSet::new(snippets))
    }

    /// Clean-up: the AI provider when one is set up and agreed to, otherwise (or when it
    /// fails, is slow, or its answer fails the checks) the built-in rules.
    /// Returns the text and who produced it.
    #[allow(clippy::too_many_arguments)]
    fn clean(
        &self,
        raw: &str,
        settings: &Settings,
        dictionary: &Dictionary,
        snippets: &SnippetSet,
        category: app_rules::Category,
        language: &str,
        hwnd: Option<isize>,
    ) -> (String, String) {
        let prepared = pipeline::prepare(raw, settings, dictionary, snippets);
        let rules_text = pipeline::finish(&prepared, settings).text;
        let rules = || rules_text.clone();
        if settings.level == Level::Literal {
            return (rules(), "rules".into());
        }
        let Some(mut cfg) = crate::ai_config(&self.state()) else {
            return (rules(), "rules".into());
        };
        self.pill(
            PillState {
                state: "polishing",
                message: "Polishing (AI)...".into(),
                ..Default::default()
            },
            hwnd,
            None,
        );
        let app_category = llm::AppCategory::parse(&format!("{category:?}"));
        // The style the user chose for this kind of app on the Style page.
        let style_key = match app_category {
            llm::AppCategory::Chat => "style_chat",
            llm::AppCategory::Email => "style_email",
            llm::AppCategory::Docs => "style_docs",
            _ => "style_other",
        };
        let style = self.state().setting(style_key);
        let request = llm::build_cleanup_styled(
            &prepared,
            settings.level,
            app_category,
            language,
            dictionary.terms(),
            Some(style.as_str()).filter(|s| !s.is_empty()),
        );
        let mut result = providers::chat(&cfg, &request.system, &request.user, AI_TIMEOUT);
        // Providers retire models. If ours is gone, pick a current one and try once more.
        if matches!(&result, Err(providers::CallError::Http(404, body)) if body.contains("model")) {
            if let Some(model) = crate::heal_ai_model(&self.state(), &cfg) {
                cfg.model = model;
                result = providers::chat(&cfg, &request.system, &request.user, AI_TIMEOUT);
            }
        }
        if let Ok(conn) = self.state().db.lock() {
            let bytes = request.system.len() + request.user.len();
            let outcome = match &result {
                Ok(_) => "ok".to_string(),
                Err(e) => e.to_string(),
            };
            db::log_event(
                &conn,
                "upload",
                &format!(
                    "{} ({}): dictation text for clean-up, {bytes} bytes, {outcome}",
                    cfg.provider, cfg.model
                ),
            );
        }
        let verdict = result
            .map_err(|e| e.to_string())
            .and_then(|answer| llm::parse_reply(&answer.content).map_err(|e| e.to_string()))
            .and_then(|reply| {
                llm::accept(
                    &reply,
                    &prepared,
                    raw,
                    dictionary.terms(),
                    settings.level,
                    &rules_text,
                )
                .map_err(|e| e.to_string())
            });
        match verdict {
            Ok(text) => (text, cfg.provider.clone()),
            Err(why) => (rules(), format!("rules (AI skipped: {why})")),
        }
    }

    /// Audio -> text -> cleanup -> (optional) insertion -> history.
    fn process(
        &mut self,
        mut samples: Vec<f32>,
        target: Option<Target>,
        do_insert: bool,
        hands_free: bool,
    ) -> Result<DictationResult, String> {
        let hwnd = target.as_ref().map(|t| t.hwnd);
        let audio_ms = samples.len() as u64 * 1000 / u64::from(app_asr::SAMPLE_RATE);
        self.pill(
            PillState {
                state: "transcribing",
                message: "Transcribing...".into(),
                engine: String::new(),
                ..Default::default()
            },
            hwnd,
            None,
        );
        // Quiet speech ("whisper mode"): gentle gain only, no noise reduction.
        let quiet = asr_audio::rms(&samples) < 0.01;
        asr_audio::normalise_quiet(&mut samples, 0.05, 8.0);

        // The Settings value is read through the typed mode, so only "en", "en-hinglish" or
        // "hinglish" can appear below (an old or unknown value means "en-hinglish").
        let mode = LanguageMode::from_setting(&self.state().setting("language"));
        let language = mode.setting().to_string();
        let (dictionary, snippets) = self.vocabulary();
        let hint = dictionary.recogniser_hint(400);
        // "en-hinglish" = English or Hinglish, decided per dictation; "hinglish" = always Hinglish.
        let hinglish_mode = mode.allows_hinglish();
        let lang = match mode {
            LanguageMode::EnglishHinglish => None,
            LanguageMode::Hinglish => Some(RecogniserLanguage::HindiRoman.code().to_string()),
            LanguageMode::English => Some(RecogniserLanguage::English.code().to_string()),
        };
        let started = Instant::now();

        let has_speech = samples.len() >= app_asr::SAMPLE_RATE as usize / 4
            && asr_audio::has_signal(&samples)
            && asr_audio::looks_like_speech(&samples);
        let mut hinglish_route = language == "hinglish";
        let cloud_cfg = crate::asr_config(&self.state()).filter(|_| has_speech);
        // English or Hinglish? Without Groq, Whisper's own language detector decides on this
        // computer (measured threshold, see app_asr::ENGLISH_SURE). With Groq there is no local
        // step at all: one request in Hindi mode keeps English as English (measured 16 Nov
        // 2026: 15 English clips, same 0.62% word errors as English mode) and Hinglish is
        // recognised from the answer below. Faster: no model run before the upload.
        if language == "en-hinglish"
            && has_speech
            && cloud_cfg.is_none()
            && self.ensure_engine().is_ok()
        {
            if let Some(loaded) = self.engine.as_ref() {
                if let Ok(p) = loaded
                    .engine
                    .language_probabilities(&samples, &["en", "hi"])
                {
                    hinglish_route = app_asr::is_hinglish(p[0], p[1]);
                }
            }
        }
        // The user's name, spelled their way, helps both recognisers.
        let name = self.state().setting("user_name");
        let name_hint = if name.trim().is_empty() {
            String::new()
        } else {
            format!(" {}.", name.trim())
        };

        // Cloud recognition (only after the user agreed to send audio), else on this computer.
        let mut cloud_note = None;
        let mut recognised: Option<(String, String)> = None;
        let mut used_cloud = false;
        if let Some(cfg) = cloud_cfg {
            // Compressed (Ogg/Opus, about 12 times smaller than WAV, same accuracy measured on
            // 45 clips): the upload is most of the waiting time on a slow connection.
            let body = app_asr::opus::encode_ogg_opus(&samples)
                .unwrap_or_else(|_| providers::wav_bytes(&samples));
            let bytes = body.len();
            let (cloud_lang, prompt) = if language == "en" {
                (Some("en".to_string()), format!("{hint}{name_hint}"))
            } else if hinglish_mode {
                (
                    Some("hi".to_string()),
                    format!("{}{name_hint}", llm::ROMAN_HINGLISH_EXAMPLE),
                )
            } else {
                (lang.clone(), format!("{hint}{name_hint}"))
            };
            let result = providers::transcribe(
                &cfg,
                body,
                cloud_lang.as_deref(),
                Some(prompt.trim()),
                CLOUD_ASR_TIMEOUT,
            );
            if let Ok(conn) = self.state().db.lock() {
                let outcome = result
                    .as_ref()
                    .map(|(_, took)| format!("ok in {} ms", took.as_millis()))
                    .unwrap_or_else(|e| e.to_string());
                db::log_event(
                    &conn,
                    "upload",
                    &format!(
                        "{} ({}): dictation audio for speech recognition, {bytes} bytes, {outcome}",
                        cfg.provider, cfg.model
                    ),
                );
            }
            match result {
                Ok((text, _)) => {
                    used_cloud = true;
                    recognised = Some((
                        app_asr::guard::filter_text(&text, quiet),
                        format!("Groq - {}", cfg.model),
                    ));
                }
                Err(e) => {
                    cloud_note = Some(format!(
                        "cloud recognition failed ({e}), used this computer"
                    ))
                }
            }
        }
        let (raw, engine_label) = match recognised {
            Some(r) => r,
            None => {
                self.ensure_engine()?;
                // Cloud failed: decide English or Hinglish here, as without Groq.
                if language == "en-hinglish" && has_speech && !hinglish_route {
                    if let Some(loaded) = self.engine.as_ref() {
                        if let Ok(p) = loaded
                            .engine
                            .language_probabilities(&samples, &["en", "hi"])
                        {
                            hinglish_route = app_asr::is_hinglish(p[0], p[1]);
                        }
                    }
                }
                let loaded = self.engine.as_ref().ok_or("speech engine not loaded")?;
                let options = if hinglish_route {
                    // Hindi recognition steered to Roman letters by an example. No glossary
                    // here: an English-only hint pushes Whisper into translating.
                    Options {
                        language: Some("hi".into()),
                        initial_prompt: Some(format!("{}{name_hint}", llm::ROMAN_HINGLISH_EXAMPLE)),
                        ..Default::default()
                    }
                } else {
                    Options {
                        language: if language == "en-hinglish" {
                            Some("en".into())
                        } else {
                            lang.clone()
                        },
                        // Vocabulary hint: dictionary terms (Whisper reads about 224 tokens of it).
                        initial_prompt: Some(format!("{hint}{name_hint}"))
                            .filter(|p| !p.trim().is_empty()),
                        ..Default::default()
                    }
                };
                let transcript = loaded.engine.transcribe(&samples, &options)?;
                (transcript.text.trim().to_string(), loaded.label.clone())
            }
        };
        // A capitals-only answer gets normal sentence capitals (words unchanged).
        let mut raw = pipeline::fix_all_caps(raw.trim());
        let mut engine_label = engine_label;
        // With Groq in Hindi mode, the answer tells whether it was Hinglish.
        if used_cloud && language == "en-hinglish" {
            hinglish_route = llm::has_devanagari(&raw) || app_core::hinglish::looks_hinglish(&raw);
        }
        // Hinglish is typed in Roman letters, as in WhatsApp, with WhatsApp spellings. One AI
        // call does both the script and the spelling (faster, and measured more accurate than
        // two calls: 12.5% vs 13.5% word errors on 30 clips, 16 Nov 2026).
        let mut to_clean = raw.clone();
        if hinglish_mode && llm::has_devanagari(&raw) {
            match self
                .ai_text(&llm::roman_spelling_prompt(), &raw, "Roman Hinglish")
                .and_then(|content| llm::parse_transliteration(&content, &raw))
            {
                Ok(roman) => {
                    raw = roman;
                    to_clean = app_core::hinglish::normalise(&raw);
                    engine_label.push_str(" + Roman Hinglish (AI)");
                }
                Err(why) => engine_label.push_str(&format!(" (Devanagari kept: {why})")),
            }
        } else if hinglish_route && !raw.is_empty() {
            // Already Roman: offline spelling table, then the AI spelling polish when allowed.
            to_clean = app_core::hinglish::normalise(&raw);
            match self.polish_hinglish(&to_clean) {
                Ok(p) => {
                    to_clean = p;
                    engine_label.push_str(" + Hinglish spelling (AI)");
                }
                Err(why) => {
                    if !why.contains("is off") {
                        engine_label.push_str(&format!(" (spelling polish skipped: {why})"));
                    }
                }
            }
        }
        let transcribe_ms = started.elapsed().as_millis() as u64;

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
                transcribe_ms,
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
        let category = target
            .as_ref()
            .map(|t| app_rules::rule_for(&t.process, &t.title).category)
            .unwrap_or(app_rules::Category::Other);
        let clean_started = Instant::now();
        let (final_text, cleanup) = self.clean(
            &to_clean,
            &settings,
            &dictionary,
            &snippets,
            category,
            if hinglish_route || llm::has_devanagari(&raw) {
                "hi-en"
            } else {
                &language
            },
            hwnd,
        );
        // Natural prose first. Spoken enumerations ("first thing ..., second thing ...")
        // become a numbered list, also after the AI clean-up; "bullet point" commands were
        // handled by the rules. Hands-free bullets only with the opt-in list setting
        // (Settings > Writing > Lists); a full stop alone never starts a new bullet.
        let mut final_text = pipeline::format_spoken_lists(&final_text);
        if hands_free {
            let format = pipeline::ListFormat::from_setting(&self.state().setting("list_format"));
            final_text = pipeline::hands_free_structure(&final_text, format);
        }
        let clean_ms = clean_started.elapsed().as_millis();
        let insert_started = Instant::now();
        let engine_label = match cloud_note {
            Some(note) => format!("{engine_label} ({note})"),
            None => engine_label,
        };

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
        // Local timing log, so speed can be measured on real use (no text is stored here).
        if let Ok(conn) = self.state().db.lock() {
            db::log_event(
                &conn,
                "timing",
                &format!(
                    "audio {audio_ms} ms | recognition {transcribe_ms} ms ({engine_label}) | clean-up {clean_ms} ms ({cleanup}) | insert {} ms",
                    insert_started.elapsed().as_millis()
                ),
            );
        }

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
            cleanup,
        };
        // The text is already in the app; a failed history save must not undo that, but the
        // user is told (instead of finding the dictation missing later).
        let saved = match self.state().db.lock() {
            Ok(conn) => db::insert_dictation(&conn, &record),
            Err(e) => Err(e.to_string()),
        };
        match saved {
            Ok(()) => {
                let _ = self.app.emit("dictation-added", &record.id);
            }
            Err(e) => {
                crate::diag::log("history", &format!("dictation not saved: {e}"));
                if do_insert {
                    self.message(
                        "info",
                        "Text inserted, but it could not be saved in History.",
                        hwnd,
                    );
                }
            }
        }

        Ok(DictationResult {
            id,
            raw_text: raw,
            final_text,
            outcome,
            audio_ms,
            transcribe_ms,
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
                if !outcome.inserted() {
                    self.message("info", outcome.message(), hwnd);
                }
            }
            (None, _) => self.message("info", "Nothing dictated yet", hwnd),
            (_, None) => self.message("info", "No window to paste into", hwnd),
        }
    }

    /// Alt+Shift+C: copy the selection of the window in front, then listen for an instruction.
    fn begin_command(&mut self) {
        if self.session.is_some() {
            return;
        }
        let Some(target) = insert::target::foreground() else {
            hotkey::force_idle();
            return;
        };
        let hwnd = Some(target.hwnd);
        let selection = match insert::copy_selection(&target) {
            Ok(s) if !s.trim().is_empty() => s,
            Ok(_) => {
                hotkey::force_idle();
                self.message(
                    "info",
                    "Select some text first, then press Alt+Shift+C",
                    hwnd,
                );
                return;
            }
            Err(e) => {
                hotkey::force_idle();
                self.message("error", e, hwnd);
                return;
            }
        };
        if let Err(e) = self.recorder.start() {
            hotkey::force_idle();
            self.message("error", e, hwnd);
            return;
        }
        self.session = Some(Session {
            target: Some(target),
            started: Instant::now(),
            hands_free: true,
            command: Some(selection),
        });
        self.pill(
            PillState {
                state: "listening",
                message: COMMAND_HINT.into(),
                hands_free: true,
                ..Default::default()
            },
            hwnd,
            None,
        );
    }

    /// The spoken instruction, recognised on this computer (instructions are short; the
    /// local model is fast and needs no consent). English or Hinglish as for dictation.
    fn recognise_command(&mut self, mut samples: Vec<f32>) -> Result<String, String> {
        if samples.len() < app_asr::SAMPLE_RATE as usize / 4 || !asr_audio::has_signal(&samples) {
            return Ok(String::new());
        }
        let quiet = asr_audio::rms(&samples) < 0.01;
        asr_audio::normalise_quiet(&mut samples, 0.05, 8.0);
        self.ensure_engine()?;
        let loaded = self.engine.as_ref().ok_or("speech engine not loaded")?;
        let p = loaded
            .engine
            .language_probabilities(&samples, &["en", "hi"])
            .unwrap_or_else(|_| vec![1.0, 0.0]);
        let options = if app_asr::is_hinglish(p[0], p[1]) {
            Options {
                language: Some("hi".into()),
                initial_prompt: Some(llm::ROMAN_HINGLISH_EXAMPLE.into()),
                ..Default::default()
            }
        } else {
            Options {
                language: Some("en".into()),
                ..Default::default()
            }
        };
        let t = loaded.engine.transcribe(&samples, &options)?;
        Ok(app_asr::guard::filter_text(&t.text, quiet)
            .trim()
            .to_string())
    }

    /// Applies a spoken instruction to `selection` and pastes the result over it.
    fn run_command(
        &mut self,
        samples: Vec<f32>,
        target: Option<Target>,
        selection: &str,
    ) -> Result<DictationResult, String> {
        let hwnd = target.as_ref().map(|t| t.hwnd);
        let audio_ms = samples.len() as u64 * 1000 / u64::from(app_asr::SAMPLE_RATE);
        self.pill(
            PillState {
                state: "transcribing",
                message: "Understanding...".into(),
                ..Default::default()
            },
            hwnd,
            None,
        );
        let started = Instant::now();
        let instruction = self.recognise_command(samples)?;
        let transcribe_ms = started.elapsed().as_millis() as u64;
        let id = uuid::Uuid::new_v4().to_string();
        let process_name = target
            .as_ref()
            .map(|t| t.process.clone())
            .unwrap_or_default();
        let mut result = DictationResult {
            id: id.clone(),
            raw_text: instruction.clone(),
            final_text: String::new(),
            outcome: None,
            audio_ms,
            transcribe_ms,
            engine: String::new(),
            target_process: process_name.clone(),
        };
        if instruction.is_empty() {
            return Ok(result);
        }
        let cmd = command::parse(&instruction);
        let (edit, how) = match command::apply(&cmd, selection) {
            Some(edit) => (edit, format!("Command (built in): {cmd:?}")),
            None => {
                self.pill(
                    PillState {
                        state: "polishing",
                        message: "Editing...".into(),
                        ..Default::default()
                    },
                    hwnd,
                    None,
                );
                let answer = self
                    .ai_raw(
                        &command::command_prompt(),
                        &command::command_request(&instruction, selection),
                        "a Command Mode edit",
                        AI_COMMAND_TIMEOUT,
                    )
                    .map_err(|e| {
                        if e.contains("is off") {
                            format!(
                                "\"{instruction}\" needs AI clean-up (Settings). Without AI: bulleted or numbered list, all caps, lowercase, title case, clean up."
                            )
                        } else {
                            e
                        }
                    })?;
                let text = command::accept_ai_edit(&answer, &instruction, selection)
                    .map_err(|why| format!("AI edit not used: {why}"))?;
                // AI lists ("- a" / "1. a" lines) also get the HTML version for Word/Docs.
                let lines: Vec<&str> = text.lines().collect();
                let html = if lines.len() > 1 && lines.iter().all(|l| l.starts_with("- ")) {
                    command::apply(&Command::BulletList, &text).and_then(|e| e.html)
                } else if lines.len() > 1
                    && lines
                        .iter()
                        .enumerate()
                        .all(|(i, l)| l.starts_with(&format!("{}. ", i + 1)))
                {
                    command::apply(&Command::NumberedList, &text).and_then(|e| e.html)
                } else {
                    None
                };
                (command::Edit { text, html }, "Command (AI)".to_string())
            }
        };
        let target = target.ok_or("No window to paste into")?;
        let outcome = insert::insert_rich(&edit.text, edit.html.as_deref(), &target);
        let record = db::Dictation {
            id: id.clone(),
            created_at: chrono::Utc::now().to_rfc3339(),
            app_process: process_name,
            app_title: target.title.clone(),
            app_category: format!(
                "{:?}",
                app_rules::rule_for(&target.process, &target.title).category
            ),
            language: self.state().setting("language"),
            engine: how.clone(),
            level: "COMMAND".into(),
            raw_text: format!("Command: {instruction}\n\nSelection:\n{selection}"),
            final_text: edit.text.clone(),
            inserted: outcome.inserted(),
            insert_method: outcome.method().to_string(),
            duration_ms: audio_ms as i64,
            audio_path: None,
            word_count: edit.text.split_whitespace().count() as i64,
            error: match &outcome {
                Outcome::Failed { message } => Some(message.clone()),
                _ => None,
            },
            cleanup: how.clone(),
        };
        match self
            .state()
            .db
            .lock()
            .map_err(|e| e.to_string())
            .and_then(|c| db::insert_dictation(&c, &record))
        {
            Ok(()) => {
                let _ = self.app.emit("dictation-added", &record.id);
            }
            Err(e) => crate::diag::log("history", &format!("command edit not saved: {e}")),
        }
        result.final_text = edit.text;
        result.outcome = Some(outcome);
        result.engine = how;
        Ok(result)
    }

    /// Self-test: copy the selection of the window in front and apply the instruction
    /// spoken in a WAV file.
    fn run_command_file(&mut self, path: &std::path::Path) -> Result<DictationResult, String> {
        let samples = asr_audio::read_wav_16k_mono(path)?;
        let target = insert::target::foreground().ok_or("no window in front")?;
        let selection = insert::copy_selection(&target)?;
        if selection.trim().is_empty() {
            return Err("nothing selected in the window in front".into());
        }
        let hwnd = Some(target.hwnd);
        let result = self.run_command(samples, Some(target), &selection);
        match &result {
            Ok(r) => self.message("done", format!("Command: {}", r.raw_text), hwnd),
            Err(e) => self.message("error", e.clone(), hwnd),
        }
        result
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
        // Test-only: APP_SELFTEST_HANDSFREE=1 processes the file as a hands-free dictation.
        let hands_free = std::env::var("APP_SELFTEST_HANDSFREE").is_ok_and(|v| v == "1");
        let result = self.process(samples, target, do_insert, hands_free);
        match &result {
            Ok(r) => match &r.outcome {
                // Text went in: no pop-up, the pill just disappears.
                Some(o) if o.inserted() => self.hide_pill(),
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

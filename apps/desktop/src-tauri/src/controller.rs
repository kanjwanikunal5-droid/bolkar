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
use app_core::dictionary::{Dictionary, Term};
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
    /// Transcribe meeting tracks: (WAV path, speaker label such as "Me" or "Others").
    TranscribeMeeting {
        tracks: Vec<(PathBuf, String)>,
        reply: Sender<Result<Vec<app_core::summary::Segment>, String>>,
    },
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
            c.expire_old_audio();
            // Load and warm up the local model now, so the first dictation is not the slow one.
            if c.ensure_engine().is_ok() {
                if let Some(loaded) = c.engine.as_ref() {
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
                Some(Msg::TranscribeMeeting { tracks, reply }) => {
                    let _ = reply.send(self.transcribe_meeting(&tracks));
                }
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
            } else {
                String::new()
            },
            level: self.recorder.level(),
            seconds: elapsed.as_secs(),
            hands_free: session.hands_free,
            engine: String::new(),
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

    /// Meeting tracks -> time-stamped segments, on this computer. Each track is split at quiet
    /// moments into pieces of at most ~30 s; silent pieces are skipped (no invented text).
    fn transcribe_meeting(
        &mut self,
        tracks: &[(PathBuf, String)],
    ) -> Result<Vec<app_core::summary::Segment>, String> {
        self.ensure_engine()?;
        let language = self.state().setting("language");
        let (dictionary, _) = self.vocabulary();
        let hint = dictionary.recogniser_hint(400);
        let options = Options {
            language: (language != "auto").then_some(language),
            initial_prompt: (!hint.is_empty()).then_some(hint),
            ..Default::default()
        };
        let mut segments = Vec::new();
        for (path, speaker) in tracks {
            let samples = asr_audio::read_wav_16k_mono(path)?;
            for (start, piece) in crate::meeting::split_for_recognition(&samples) {
                if !asr_audio::has_signal(piece) {
                    continue;
                }
                let loaded = self.engine.as_ref().ok_or("speech engine not loaded")?;
                let t = loaded.engine.transcribe(piece, &options)?;
                let text = t.text.trim().to_string();
                if text.is_empty() {
                    continue;
                }
                let start_ms = start as u64 * 1000 / u64::from(app_asr::SAMPLE_RATE);
                segments.push(app_core::summary::Segment {
                    id: String::new(),
                    start_ms,
                    end_ms: start_ms + piece.len() as u64 * 1000 / u64::from(app_asr::SAMPLE_RATE),
                    speaker: speaker.clone(),
                    text,
                });
            }
        }
        segments.sort_by_key(|s| s.start_ms);
        // Also renumbers the ids s1, s2, ...
        app_core::summary::drop_echo(&mut segments);
        Ok(segments)
    }

    /// Devanagari -> Roman Hinglish with the AI provider (script only; checked by
    /// `llm::parse_transliteration`).
    fn to_roman_hinglish(&self, text: &str) -> Result<String, String> {
        self.ai_text(&llm::transliterate_prompt(), text, "Roman Hinglish")
            .and_then(|content| llm::parse_transliteration(&content, text))
    }

    /// Hinglish spelling polish (bahut, karunga, ...) and obvious mis-hearings; checked by
    /// `llm::parse_polish`, which rejects rewrites.
    fn polish_hinglish(&self, text: &str) -> Result<String, String> {
        self.ai_text(&llm::hinglish_polish_prompt(), text, "Hinglish spelling")
            .and_then(|content| llm::parse_polish(&content, text))
    }

    /// Sends dictation text to the user's AI provider and logs the upload.
    fn ai_text(&self, system: &str, text: &str, purpose: &str) -> Result<String, String> {
        let cfg = crate::ai_config(&self.state()).ok_or("AI clean-up is off")?;
        let user = serde_json::json!({ "text": text }).to_string();
        let result = providers::chat(&cfg, system, &user, AI_TIMEOUT);
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
        let request = llm::build_cleanup(
            &prepared,
            settings.level,
            llm::AppCategory::parse(&format!("{category:?}")),
            language,
            dictionary.terms(),
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

        let language = self.state().setting("language");
        let (dictionary, snippets) = self.vocabulary();
        let hint = dictionary.recogniser_hint(400);
        // "en-hinglish" = English or Hinglish, decided per dictation; "hinglish" = always Hinglish.
        let hinglish_mode = matches!(language.as_str(), "en-hinglish" | "hinglish");
        let lang = match language.as_str() {
            "auto" | "en-hinglish" => None,
            "hinglish" => Some("hi".to_string()),
            other => Some(other.to_string()),
        };
        let started = Instant::now();

        let has_speech =
            samples.len() >= app_asr::SAMPLE_RATE as usize / 4 && asr_audio::has_signal(&samples);
        // English or Hinglish? Whisper's own language detector decides on this computer
        // (measured threshold, see app_asr::ENGLISH_SURE), for local and cloud recognition.
        let mut hinglish_route = language == "hinglish";
        let mut detected = false;
        if language == "en-hinglish" && has_speech && self.ensure_engine().is_ok() {
            if let Some(loaded) = self.engine.as_ref() {
                if let Ok(p) = loaded
                    .engine
                    .language_probabilities(&samples, &["en", "hi"])
                {
                    hinglish_route = app_asr::is_hinglish(p[0], p[1]);
                    detected = true;
                }
            }
        }

        // Cloud recognition (only after the user agreed to send audio), else on this computer.
        let mut cloud_note = None;
        let mut recognised: Option<(String, String)> = None;
        if let Some(cfg) = crate::asr_config(&self.state()).filter(|_| has_speech) {
            let wav = providers::wav_bytes(&samples);
            let bytes = wav.len();
            // Groq's Whisper answers Hinglish in mixed Hindi/Roman letters whatever the
            // prompt; the AI script conversion below turns it into Roman (measured on the
            // owner's voice notes, 14 Nov 2026).
            let (cloud_lang, prompt) = if hinglish_route {
                (Some("hi".to_string()), llm::ROMAN_HINGLISH_EXAMPLE)
            } else if detected {
                (Some("en".to_string()), hint.as_str())
            } else {
                (lang.clone(), hint.as_str())
            };
            let result = providers::transcribe(
                &cfg,
                wav,
                cloud_lang.as_deref(),
                Some(prompt),
                CLOUD_ASR_TIMEOUT,
            );
            if let Ok(conn) = self.state().db.lock() {
                let outcome = result
                    .as_ref()
                    .map(|_| "ok".to_string())
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
                let loaded = self.engine.as_ref().ok_or("speech engine not loaded")?;
                let options = if hinglish_route {
                    // Hindi recognition steered to Roman letters by an example. No glossary
                    // here: an English-only hint pushes Whisper into translating.
                    Options {
                        language: Some("hi".into()),
                        initial_prompt: Some(llm::ROMAN_HINGLISH_EXAMPLE.into()),
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
                        initial_prompt: (!hint.is_empty()).then_some(hint.clone()),
                        ..Default::default()
                    }
                };
                let transcript = loaded.engine.transcribe(&samples, &options)?;
                (transcript.text.trim().to_string(), loaded.label.clone())
            }
        };
        let mut raw = raw.trim().to_string();
        let mut engine_label = engine_label;
        // Hinglish is typed in Roman letters, as in WhatsApp. The example prompt above
        // normally gives Roman text already; Devanagari left over (cloud recognition, or
        // Whisper ignoring the example) is converted by the AI when it is on.
        if hinglish_mode && llm::has_devanagari(&raw) {
            match self.to_roman_hinglish(&raw) {
                Ok(roman) => {
                    raw = roman;
                    engine_label.push_str(" + Roman script (AI)");
                }
                Err(why) => engine_label.push_str(&format!(" (Devanagari kept: {why})")),
            }
        }
        // Common WhatsApp spellings and obvious mis-hearings. The raw transcript keeps what
        // the recogniser heard; only the text sent on to clean-up is polished.
        let mut to_clean = raw.clone();
        if hinglish_route && !raw.is_empty() && !llm::has_devanagari(&raw) {
            match self.polish_hinglish(&raw) {
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

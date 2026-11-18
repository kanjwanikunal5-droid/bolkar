//! Meetings: recording with a near-live transcript, notes for long meetings, and the
//! library of past meetings (open, rename, correct lines, write notes again, export, delete).

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::channel;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Manager, State};

use app_core::summary::{self, Check, Segment, Summary};

use crate::controller::{MeetingChunk, Msg};
use crate::{db, meeting, providers, AppState};

/// Characters of transcript per notes request (long meetings are split into windows).
const NOTES_WINDOW_CHARS: usize = 12_000;
/// How often the near-live transcript asks for new lines while recording.
const LIVE_EVERY: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Default, Serialize)]
pub struct Live {
    /// Lines transcribed so far while recording (provisional: echo removal and Roman
    /// Hinglish are applied again when the meeting is saved).
    pub segments: Vec<Segment>,
    #[serde(skip)]
    pub next: Vec<usize>,
    /// "starting", "live" (lines appear while recording) or "after" (no graphics card: the
    /// transcript is made after the meeting).
    pub mode: String,
    pub error: Option<String>,
}

pub struct ActiveMeeting {
    rec: meeting::Recording,
    started_at: String,
    tracks: Vec<(PathBuf, String)>,
    live: Arc<Mutex<Live>>,
    live_stop: Arc<AtomicBool>,
    live_thread: Option<std::thread::JoinHandle<()>>,
}

fn ask_controller(
    state: &AppState,
    tracks: Vec<(PathBuf, String)>,
    from: Vec<usize>,
    final_pass: bool,
) -> Result<MeetingChunk, String> {
    let (reply, rx) = channel();
    state
        .ctl
        .lock()
        .map_err(|e| e.to_string())?
        .send(Msg::TranscribeMeeting {
            tracks,
            from,
            final_pass,
            reply,
        })
        .map_err(|e| e.to_string())?;
    rx.recv().map_err(|e| e.to_string())?
}

fn live_loop(
    app: AppHandle,
    tracks: Vec<(PathBuf, String)>,
    live: Arc<Mutex<Live>>,
    stop: Arc<AtomicBool>,
) {
    let mut waited = Duration::ZERO;
    while !stop.load(Ordering::SeqCst) {
        std::thread::sleep(Duration::from_millis(250));
        waited += Duration::from_millis(250);
        if waited < LIVE_EVERY {
            continue;
        }
        waited = Duration::ZERO;
        let from = live.lock().map(|l| l.next.clone()).unwrap_or_default();
        let state = app.state::<AppState>();
        match ask_controller(&state, tracks.clone(), from, false) {
            Ok(chunk) => {
                let Ok(mut l) = live.lock() else { return };
                l.segments.extend(chunk.segments);
                l.segments.sort_by_key(|s| s.start_ms);
                l.next = chunk.next;
                l.mode = if chunk.live_ok { "live" } else { "after" }.into();
                if !chunk.live_ok {
                    return;
                }
            }
            Err(e) => {
                crate::diag::log("meeting", &format!("live transcript paused: {e}"));
                if let Ok(mut l) = live.lock() {
                    l.error = Some(format!("The live transcript paused ({e}). Everything is transcribed when you stop."));
                }
                return;
            }
        }
    }
}

#[tauri::command]
pub fn list_audio_devices() -> meeting::Devices {
    meeting::list_devices()
}

#[tauri::command]
pub fn start_meeting(
    app: AppHandle,
    state: State<AppState>,
    mic: bool,
    system: bool,
    consent: bool,
    mic_device: Option<String>,
    system_device: Option<String>,
) -> Result<(), String> {
    if !consent {
        return Err("Please confirm that the participants know they are being recorded.".into());
    }
    let mut slot = state.meeting.lock().map_err(|e| e.to_string())?;
    if slot.is_some() {
        return Err("A meeting is already being recorded.".into());
    }
    let rec = meeting::Recording::start(
        &state.data_dir.join("meetings"),
        mic,
        system,
        mic_device.as_deref().unwrap_or(""),
        system_device.as_deref().unwrap_or(""),
    )?;
    crate::diag::log(
        "meeting",
        &format!("recording started (mic: {mic}, system audio: {system})"),
    );
    let tracks = rec.track_paths();
    let live = Arc::new(Mutex::new(Live {
        next: vec![0; tracks.len()],
        mode: "starting".into(),
        ..Default::default()
    }));
    let live_stop = Arc::new(AtomicBool::new(false));
    let live_thread = {
        let (app, tracks, live, stop) = (
            app.clone(),
            tracks.clone(),
            Arc::clone(&live),
            Arc::clone(&live_stop),
        );
        std::thread::Builder::new()
            .name("meeting-live".into())
            .spawn(move || live_loop(app, tracks, live, stop))
            .ok()
    };
    *slot = Some(ActiveMeeting {
        rec,
        started_at: chrono::Utc::now().to_rfc3339(),
        tracks,
        live,
        live_stop,
        live_thread,
    });
    Ok(())
}

#[derive(Serialize)]
pub struct MeetingStatus {
    #[serde(flatten)]
    pub rec: meeting::LiveStatus,
    pub live: Live,
}

#[tauri::command]
pub fn meeting_status(state: State<AppState>) -> Option<MeetingStatus> {
    let slot = state.meeting.lock().ok()?;
    let m = slot.as_ref()?;
    Some(MeetingStatus {
        rec: m.rec.status(),
        live: m.live.lock().map(|l| l.clone()).unwrap_or_default(),
    })
}

#[derive(Debug, Clone, Serialize)]
pub struct ViewSegment {
    pub id: String,
    pub start_ms: u64,
    pub end_ms: u64,
    pub speaker: String,
    /// The corrected text if the user corrected the line, else the original.
    pub text: String,
    pub original: String,
    pub edited: bool,
}

/// Everything the meeting screen shows, for a just-finished or a saved meeting.
#[derive(Debug, Clone, Serialize)]
pub struct MeetingView {
    pub id: String,
    pub title: String,
    pub started_at: String,
    pub duration_ms: u64,
    pub status: String,
    pub has_audio: bool,
    pub segments: Vec<ViewSegment>,
    pub thoughts: Vec<(u64, String)>,
    pub summary: Option<Summary>,
    /// Items removed (cited no transcript line that exists) and fields blanked, with reasons.
    pub dropped: Vec<String>,
    /// Items kept but marked "check this", with reasons.
    pub flags: Vec<String>,
    pub markdown: String,
    /// Why there are no notes, or which part is missing.
    pub note: Option<String>,
    /// Recording problems (a device stopped, a file could not be written).
    pub problems: Vec<String>,
}

struct Notes {
    summary: Option<Summary>,
    dropped: Vec<String>,
    flags: Vec<String>,
    note: Option<String>,
    provider: String,
}

/// Writes the notes, window by window for long meetings, then checks them once against the
/// whole transcript. The transcript is never lost when this fails.
fn write_notes(
    app: &AppHandle,
    title: &str,
    segments: &[Segment],
    thoughts: &[(u64, String)],
    duration_ms: u64,
) -> Notes {
    let state = app.state::<AppState>();
    let mut notes = Notes {
        summary: None,
        dropped: Vec::new(),
        flags: Vec::new(),
        note: None,
        provider: String::new(),
    };
    if segments.is_empty() {
        notes.note = Some("Nothing was heard in the recording.".into());
        return notes;
    }
    let Some(cfg) = crate::ai_config(&state) else {
        notes.note = Some(
            "Turn on AI clean-up in Settings > Groq & AI to get organised notes. The transcript is below."
                .into(),
        );
        return notes;
    };
    notes.provider = cfg.provider.clone();
    let system = summary::summary_prompt(
        "the language of the meeting",
        "the script used in the transcript (Hinglish in Roman letters)",
    );
    let windows = summary::windows(segments, NOTES_WINDOW_CHARS);
    let n = windows.len();
    let mut parts = Vec::new();
    let mut failed = Vec::new();
    for (i, w) in windows.iter().enumerate() {
        let from = if i == 0 { 0 } else { w[0].start_ms };
        let until = windows.get(i + 1).map(|x| x[0].start_ms);
        let mine = summary::thoughts_for(thoughts, from, until);
        let user = summary::summary_request_part(title, w, &mine, i + 1, n);
        let bytes = system.len() + user.len();
        // Free tiers limit requests per minute: on "rate limited", wait and try again (up to
        // three more times, 20/40/60 s) instead of losing that part of the notes.
        let mut answer = providers::chat(&cfg, &system, &user, Duration::from_secs(90));
        for wait_s in [20, 40, 60] {
            if !matches!(answer, Err(providers::CallError::RateLimited)) {
                break;
            }
            crate::diag::log(
                "meeting",
                &format!(
                    "notes part {}/{n}: rate limited, retrying in {wait_s} s",
                    i + 1
                ),
            );
            std::thread::sleep(Duration::from_secs(wait_s));
            answer = providers::chat(&cfg, &system, &user, Duration::from_secs(90));
        }
        if let Ok(conn) = state.db.lock() {
            let outcome = answer
                .as_ref()
                .map(|_| "ok".to_string())
                .unwrap_or_else(|e| e.to_string());
            db::log_event(
                &conn,
                "upload",
                &format!(
                    "{}: meeting transcript part {}/{n} for notes, {bytes} bytes, {outcome}",
                    cfg.provider,
                    i + 1
                ),
            );
        }
        match answer
            .map_err(|e| e.to_string())
            .and_then(|a| summary::parse_summary(&a.content))
        {
            Ok(s) => parts.push(s),
            Err(e) => failed.push(format!("part {} of {n}: {e}", i + 1)),
        }
    }
    if parts.is_empty() {
        notes.note = Some(format!(
            "Notes could not be written ({}). The transcript is kept; use \"Write notes again\" later.",
            failed.join("; ")
        ));
        return notes;
    }
    let mut v = summary::validate(summary::merge(parts), segments);
    v.summary.overview.title = title.to_string();
    v.summary.overview.duration_min = Some((duration_ms as f64 / 60000.0 * 10.0).round() / 10.0);
    if !failed.is_empty() {
        notes.note = Some(format!(
            "These notes cover only part of the meeting ({} could not be written). Use \"Write notes again\" to retry.",
            failed.join("; ")
        ));
    }
    notes.summary = Some(v.summary);
    notes.dropped = v.dropped;
    notes.flags = v.flags;
    notes
}

fn flags_of(s: &Summary) -> Vec<String> {
    let mut out = Vec::new();
    let mut add = |what: &str, check: Check, note: &Option<String>| {
        if check == Check::NeedsReview {
            out.push(format!(
                "\"{what}\": {}",
                note.as_deref().unwrap_or("check this item")
            ));
        }
    };
    for p in s
        .executive_summary
        .iter()
        .chain(s.risks.iter())
        .chain(s.open_questions.iter())
        .chain(s.next_steps.iter())
        .chain(s.takeaway.iter())
        .chain(
            s.topics
                .iter()
                .flat_map(|t| t.narrative.iter().chain(t.points.iter())),
        )
    {
        add(&p.text, p.check, &p.note);
    }
    for d in &s.decisions {
        add(&d.text, d.check, &d.note);
    }
    for a in &s.action_items {
        add(&a.action, a.check, &a.note);
    }
    out
}

fn view_from_saved(m: db::SavedMeeting) -> MeetingView {
    let segments: Vec<ViewSegment> = m
        .segments
        .iter()
        .map(|s| ViewSegment {
            id: s.id.clone(),
            start_ms: s.start_ms.max(0) as u64,
            end_ms: s.end_ms.max(0) as u64,
            speaker: s.speaker.clone(),
            text: s.edited.clone().unwrap_or_else(|| s.original.clone()),
            original: s.original.clone(),
            edited: s.edited.is_some(),
        })
        .collect();
    let summary: Option<Summary> = m
        .summary_json
        .as_deref()
        .and_then(|j| serde_json::from_str(j).ok());
    let flags = summary.as_ref().map(flags_of).unwrap_or_default();
    let segs = effective(&segments);
    let markdown = markdown_for(&m.row.title, summary.as_ref(), &segs, &m.thoughts);
    let note = match (m.row.status.as_str(), summary.is_some()) {
        ("interrupted", _) => Some(
            "This recording was interrupted (the app closed while recording). Transcribe it to get the transcript and notes."
                .to_string(),
        ),
        (_, false) if !segments.is_empty() => {
            Some("No notes yet. Use \"Write notes again\" (needs AI clean-up).".to_string())
        }
        _ => None,
    };
    MeetingView {
        id: m.row.id,
        title: m.row.title,
        started_at: m.row.started_at,
        duration_ms: m.row.duration_ms.max(0) as u64,
        status: m.row.status,
        has_audio: m.row.has_audio,
        segments,
        thoughts: m.thoughts,
        summary,
        dropped: Vec::new(),
        flags,
        markdown,
        note,
        problems: Vec::new(),
    }
}

/// Transcript lines as the summary sees them (corrections applied).
fn effective(view: &[ViewSegment]) -> Vec<Segment> {
    view.iter()
        .map(|s| Segment {
            id: s.id.clone(),
            start_ms: s.start_ms,
            end_ms: s.end_ms,
            speaker: s.speaker.clone(),
            text: s.text.clone(),
        })
        .collect()
}

fn markdown_for(
    title: &str,
    summary: Option<&Summary>,
    segments: &[Segment],
    thoughts: &[(u64, String)],
) -> String {
    let empty = Summary {
        overview: summary::Overview {
            title: title.to_string(),
            ..Default::default()
        },
        ..Default::default()
    };
    let mut s = summary.cloned().unwrap_or(empty);
    s.overview.title = title.to_string();
    summary::to_markdown(&s, segments, thoughts)
}

fn load_view(state: &AppState, id: &str) -> Result<MeetingView, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::get_meeting(&conn, id).map(view_from_saved)
}

/// Stops the recording, transcribes what the live transcript has not done yet, writes the
/// notes (if AI clean-up is set up) and saves everything.
#[tauri::command]
pub async fn stop_meeting(
    app: AppHandle,
    state: State<'_, AppState>,
    title: String,
    thoughts: Vec<(u64, String)>,
) -> Result<MeetingView, String> {
    let mut active = state
        .meeting
        .lock()
        .map_err(|e| e.to_string())?
        .take()
        .ok_or("No meeting is being recorded.")?;
    active.live_stop.store(true, Ordering::SeqCst);
    let live_thread = active.live_thread.take();
    let rec = active.rec;
    let finished = tauri::async_runtime::spawn_blocking(move || {
        if let Some(t) = live_thread {
            let _ = t.join();
        }
        rec.stop()
    })
    .await
    .map_err(|e| e.to_string())??;
    let (mut segments, from) = active
        .live
        .lock()
        .map(|l| (l.segments.clone(), l.next.clone()))
        .unwrap_or_default();
    let tracks = active.tracks.clone();
    let app2 = app.clone();
    let rest = tauri::async_runtime::spawn_blocking(move || {
        ask_controller(&app2.state::<AppState>(), tracks, from, true)
    })
    .await
    .map_err(|e| e.to_string())??;
    segments.extend(rest.segments);
    segments.sort_by_key(|s| s.start_ms);
    // Also renumbers the ids s1, s2, ...
    summary::drop_echo(&mut segments);

    let title = if title.trim().is_empty() {
        "Meeting".to_string()
    } else {
        title.trim().to_string()
    };
    let (app3, t2, s2, th2, dur) = (
        app.clone(),
        title.clone(),
        segments.clone(),
        thoughts.clone(),
        finished.duration_ms,
    );
    let notes =
        tauri::async_runtime::spawn_blocking(move || write_notes(&app3, &t2, &s2, &th2, dur))
            .await
            .map_err(|e| e.to_string())?;
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        let json = notes
            .summary
            .as_ref()
            .and_then(|s| serde_json::to_string(s).ok());
        db::save_meeting(
            &conn,
            &finished.id,
            &title,
            &active.started_at,
            finished.duration_ms,
            finished.mic_path.as_deref(),
            finished.system_path.as_deref(),
            &segments,
            &thoughts,
            json.as_deref(),
            &notes.provider,
        )?;
    }
    crate::diag::log(
        "meeting",
        &format!(
            "saved: {} s, {} lines, notes: {}",
            finished.duration_ms / 1000,
            segments.len(),
            if notes.summary.is_some() { "yes" } else { "no" }
        ),
    );
    let mut view = load_view(&state, &finished.id)?;
    view.dropped = notes.dropped;
    view.flags = notes.flags;
    view.note = notes.note.or(view.note);
    view.problems = finished.problems;
    Ok(view)
}

#[tauri::command]
pub fn list_meetings(
    state: State<AppState>,
    query: Option<String>,
) -> Result<Vec<db::MeetingRow>, String> {
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::list_meetings(&conn, query.as_deref().unwrap_or(""))
}

#[tauri::command]
pub fn get_meeting(state: State<AppState>, id: String) -> Result<MeetingView, String> {
    load_view(&state, &id)
}

#[tauri::command]
pub fn rename_meeting(
    state: State<AppState>,
    id: String,
    title: String,
) -> Result<MeetingView, String> {
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::rename_meeting(&conn, &id, &title)?;
    }
    load_view(&state, &id)
}

/// Saves a correction of one transcript line (the recogniser's original is kept).
#[tauri::command]
pub fn edit_meeting_line(
    state: State<AppState>,
    id: String,
    line: String,
    text: String,
) -> Result<MeetingView, String> {
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::edit_segment(&conn, &id, &line, &text)?;
    }
    load_view(&state, &id)
}

/// Writes the notes again from the saved (and corrected) transcript. A new version is saved;
/// earlier versions stay in the database. The transcript and "My thoughts" are not changed.
#[tauri::command]
pub async fn rewrite_meeting_notes(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<MeetingView, String> {
    let view = load_view(&state, &id)?;
    let segments = effective(&view.segments);
    let (app2, title, th, dur) = (
        app.clone(),
        view.title.clone(),
        view.thoughts.clone(),
        view.duration_ms,
    );
    let notes = tauri::async_runtime::spawn_blocking(move || {
        write_notes(&app2, &title, &segments, &th, dur)
    })
    .await
    .map_err(|e| e.to_string())?;
    if let Some(s) = &notes.summary {
        let json = serde_json::to_string(s).map_err(|e| e.to_string())?;
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::save_summary_version(&conn, &id, &json, &notes.provider)?;
    }
    let mut out = load_view(&state, &id)?;
    out.dropped = notes.dropped;
    if notes.summary.is_some() {
        out.flags = notes.flags;
    }
    out.note = notes.note.or(out.note);
    Ok(out)
}

/// Transcribes a saved recording from the start (an interrupted one, or again after a model
/// change), then writes the notes. Corrections of the old transcript are replaced.
#[tauri::command]
pub async fn transcribe_saved_meeting(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
) -> Result<MeetingView, String> {
    let saved = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::get_meeting(&conn, &id)?
    };
    let mut tracks = Vec::new();
    if let Some(p) = &saved.mic_path {
        tracks.push((PathBuf::from(p), summary::MIC.to_string()));
    }
    if let Some(p) = &saved.system_path {
        tracks.push((PathBuf::from(p), summary::SYSTEM.to_string()));
    }
    if tracks.is_empty() {
        return Err(
            "The audio of this meeting was deleted, so it cannot be transcribed again.".into(),
        );
    }
    let n = tracks.len();
    let app2 = app.clone();
    let chunk = tauri::async_runtime::spawn_blocking(move || {
        ask_controller(&app2.state::<AppState>(), tracks, vec![0; n], true)
    })
    .await
    .map_err(|e| e.to_string())??;
    let mut segments = chunk.segments;
    segments.sort_by_key(|s| s.start_ms);
    summary::drop_echo(&mut segments);
    {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::replace_segments(&conn, &id, &segments, "transcribed")?;
        if saved.row.status == "interrupted" && saved.row.title == "Interrupted recording" {
            let _ = db::rename_meeting(&conn, &id, "Recovered meeting");
        }
    }
    rewrite_meeting_notes(app, state, id).await
}

/// The Downloads\<app name> folder used for exports.
fn export_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .download_dir()
        .map_err(|e| e.to_string())?
        .join(app_core::brand::APP_NAME);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn safe_file_name(s: &str) -> String {
    let cleaned: String = s
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' {
                c
            } else {
                ' '
            }
        })
        .collect();
    let joined = cleaned.split_whitespace().collect::<Vec<_>>().join(" ");
    let short: String = joined.chars().take(60).collect();
    if short.is_empty() {
        "Meeting".into()
    } else {
        short
    }
}

/// Saves the notes as Markdown ("md") or plain text ("txt") in Downloads\Speak Flow and
/// returns the file path.
#[tauri::command]
pub fn export_meeting(
    app: AppHandle,
    state: State<AppState>,
    id: String,
    format: String,
) -> Result<String, String> {
    let view = load_view(&state, &id)?;
    let (ext, body) = match format.as_str() {
        "md" => ("md", view.markdown.clone()),
        "txt" => ("txt", summary::to_text(&view.markdown)),
        _ => return Err("Choose Markdown or plain text.".into()),
    };
    let date = view.started_at.get(..10).unwrap_or("").to_string();
    let path = export_dir(&app)?.join(format!("{} {date}.{ext}", safe_file_name(&view.title)));
    std::fs::write(&path, body).map_err(|e| format!("Could not save {}: {e}", path.display()))?;
    crate::diag::log("meeting", &format!("notes exported as {ext}"));
    Ok(path.to_string_lossy().into_owned())
}

/// Shows a file this app saved (an export or the diagnostics log) in File Explorer.
#[tauri::command]
pub fn reveal_file(app: AppHandle, state: State<AppState>, path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    let allowed = [
        export_dir(&app).ok(),
        crate::diag::dir().cloned(),
        Some(state.data_dir.clone()),
    ];
    let inside = allowed
        .iter()
        .flatten()
        .any(|d| p.starts_with(d) || p == *d);
    if !inside || path.contains('"') {
        return Err("Only files saved by this app can be shown.".into());
    }
    let mut cmd = std::process::Command::new("explorer.exe");
    if p.is_file() {
        cmd.arg(format!("/select,{path}"));
    } else {
        cmd.arg(&path);
    }
    cmd.spawn().map(|_| ()).map_err(|e| e.to_string())
}

/// Deletes one meeting: its audio files first (if one cannot be deleted, nothing else is
/// deleted and the error says which file), then its transcript, notes and summaries.
pub fn delete_meeting_everywhere(state: &AppState, id: &str) -> Result<(), String> {
    let saved = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::get_meeting(&conn, id)?
    };
    let mut errors = Vec::new();
    for p in [saved.mic_path, saved.system_path].into_iter().flatten() {
        if let Err(e) = crate::remove_app_file(&state.data_dir, &p) {
            errors.push(e);
        }
    }
    if !errors.is_empty() {
        crate::diag::log("meeting", &format!("delete failed: {errors:?}"));
        return Err(format!(
            "Could not delete: {}. Nothing else was deleted.",
            errors.join("; ")
        ));
    }
    let dir = state.data_dir.join("meetings").join(id);
    if dir.is_dir() {
        std::fs::remove_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let conn = state.db.lock().map_err(|e| e.to_string())?;
    db::delete_meeting(&conn, id)?;
    db::log_event(
        &conn,
        "deleted",
        "one meeting with its audio, transcript and notes",
    );
    Ok(())
}

#[tauri::command]
pub fn delete_meeting(state: State<AppState>, id: String) -> Result<(), String> {
    delete_meeting_everywhere(&state, &id)
}

/// Deletes every saved meeting. Returns how many were deleted; failures are listed.
#[tauri::command]
pub fn delete_all_meetings(state: State<AppState>) -> Result<usize, String> {
    let ids = {
        let conn = state.db.lock().map_err(|e| e.to_string())?;
        db::all_meeting_ids(&conn)
    };
    let mut done = 0;
    let mut failed = Vec::new();
    for id in ids {
        match delete_meeting_everywhere(&state, &id) {
            Ok(()) => done += 1,
            Err(e) => failed.push(e),
        }
    }
    if failed.is_empty() {
        Ok(done)
    } else {
        Err(format!(
            "{done} deleted, {} failed: {}",
            failed.len(),
            failed.join(" | ")
        ))
    }
}

/// At start-up: recording folders without a saved meeting are recordings the app could not
/// finish (crash, power cut). They are listed as "interrupted" so the user can transcribe or
/// delete them. Empty folders are removed.
pub fn recover_interrupted(state: &AppState) {
    let dir = state.data_dir.join("meetings");
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let id = entry.file_name().to_string_lossy().into_owned();
        let exists = state
            .db
            .lock()
            .map(|c| db::meeting_exists(&c, &id))
            .unwrap_or(true);
        if exists {
            continue;
        }
        let file = |n: &str| {
            let p = path.join(n);
            (p.is_file() && std::fs::metadata(&p).is_ok_and(|m| m.len() > 44))
                .then(|| p.to_string_lossy().into_owned())
        };
        let (mic, sys) = (file("mic.wav"), file("system.wav"));
        if mic.is_none() && sys.is_none() {
            let _ = std::fs::remove_dir_all(&path);
            continue;
        }
        let duration_ms = [&mic, &sys]
            .iter()
            .filter_map(|p| p.as_deref())
            .filter_map(|p| app_asr::audio::wav_len_16k(Path::new(p)).ok())
            .max()
            .unwrap_or(0) as u64
            * 1000
            / u64::from(app_asr::SAMPLE_RATE);
        let started: chrono::DateTime<chrono::Utc> = entry
            .metadata()
            .and_then(|m| m.created())
            .map(Into::into)
            .unwrap_or_else(|_| chrono::Utc::now());
        if let Ok(conn) = state.db.lock() {
            if db::insert_interrupted(
                &conn,
                &id,
                &started.to_rfc3339(),
                duration_ms,
                mic.as_deref(),
                sys.as_deref(),
            )
            .is_ok()
            {
                db::log_event(
                    &conn,
                    "meeting",
                    &format!("interrupted recording found ({} s)", duration_ms / 1000),
                );
            }
        }
    }
}

/// Quiet end-to-end test of the meeting pipeline (no window, no recording device): the WAV
/// files `mic.wav` / `system.wav` in `src` are copied in as a recording, transcribed with one
/// live pass and the final pass (as during a real meeting), turned into notes, saved, and read
/// back from the library. The result is written as JSON to `out`; then the app exits.
pub fn self_test_meeting(app: AppHandle, src: PathBuf, out: Option<PathBuf>) {
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(1500));
        let state = app.state::<AppState>();
        let result = (|| -> Result<serde_json::Value, String> {
            let id = uuid::Uuid::new_v4().to_string();
            let dir = state.data_dir.join("meetings").join(&id);
            std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
            let mut tracks = Vec::new();
            for (name, label) in [("mic.wav", summary::MIC), ("system.wav", summary::SYSTEM)] {
                let from = src.join(name);
                if from.is_file() {
                    std::fs::copy(&from, dir.join(name)).map_err(|e| e.to_string())?;
                    tracks.push((dir.join(name), label.to_string()));
                }
            }
            let n = tracks.len();
            let t0 = std::time::Instant::now();
            let live = ask_controller(&state, tracks.clone(), vec![0; n], false)?;
            let live_ms = t0.elapsed().as_millis() as u64;
            let t1 = std::time::Instant::now();
            let rest = ask_controller(&state, tracks.clone(), live.next.clone(), true)?;
            let final_ms = t1.elapsed().as_millis() as u64;
            let mut segments = live.segments.clone();
            segments.extend(rest.segments.clone());
            segments.sort_by_key(|s| s.start_ms);
            summary::drop_echo(&mut segments);
            let duration_ms = tracks
                .iter()
                .filter_map(|(p, _)| app_asr::audio::wav_len_16k(p).ok())
                .max()
                .unwrap_or(0) as u64
                * 1000
                / u64::from(app_asr::SAMPLE_RATE);
            let path = |name: &str| {
                let p = dir.join(name);
                p.is_file().then(|| p.to_string_lossy().into_owned())
            };
            {
                let conn = state.db.lock().map_err(|e| e.to_string())?;
                db::insert_interrupted(
                    &conn,
                    &id,
                    &chrono::Utc::now().to_rfc3339(),
                    duration_ms,
                    path("mic.wav").as_deref(),
                    path("system.wav").as_deref(),
                )?;
                db::replace_segments(&conn, &id, &segments, "transcribed")?;
                db::rename_meeting(&conn, &id, "Self-test meeting")?;
            }
            let t2 = std::time::Instant::now();
            let notes = write_notes(&app, "Self-test meeting", &segments, &[], duration_ms);
            let notes_ms = t2.elapsed().as_millis() as u64;
            if let Some(s) = &notes.summary {
                let json = serde_json::to_string(s).map_err(|e| e.to_string())?;
                let conn = state.db.lock().map_err(|e| e.to_string())?;
                db::save_summary_version(&conn, &id, &json, &notes.provider)?;
            }
            let listed = {
                let conn = state.db.lock().map_err(|e| e.to_string())?;
                db::list_meetings(&conn, "")?.iter().any(|m| m.id == id)
            };
            let view = load_view(&state, &id)?;
            Ok(serde_json::json!({
                "ok": true,
                "audio_ms": duration_ms,
                "live_ok": live.live_ok,
                "live_lines": live.segments.len(),
                "final_lines": rest.segments.len(),
                "live_ms": live_ms,
                "final_ms": final_ms,
                "notes_ms": notes_ms,
                "listed_in_library": listed,
                "note": notes.note,
                "flags": notes.flags,
                "dropped": notes.dropped,
                "view": view,
            }))
        })();
        let json = result.unwrap_or_else(|e| serde_json::json!({ "ok": false, "error": e }));
        if let Some(path) = out {
            let _ = std::fs::write(
                path,
                serde_json::to_string_pretty(&json).unwrap_or_default(),
            );
        }
        app.exit(0);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn export_names_are_safe() {
        assert_eq!(
            safe_file_name("Q3 review: \"budget\"/hiring?"),
            "Q3 review budget hiring"
        );
        assert_eq!(safe_file_name("..\\..\\x"), "x");
        assert_eq!(safe_file_name("   "), "Meeting");
    }
}

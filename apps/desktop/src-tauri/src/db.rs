//! Local SQLite storage. Schema changes go in `MIGRATIONS`, one entry per version.

use std::path::Path;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

const MIGRATIONS: &[&str] = &[
    // v1: full schema from the build plan (FTS tables arrive with search, in a later version).
    r#"
    CREATE TABLE settings(key TEXT PRIMARY KEY, value TEXT);
    CREATE TABLE providers(id TEXT PRIMARY KEY, kind TEXT CHECK(kind IN('asr','llm')), name TEXT, endpoint TEXT,
        model TEXT, enabled INT, consent_at TEXT, data_policy_note TEXT);
    CREATE TABLE dictations(id TEXT PRIMARY KEY, created_at TEXT, app_process TEXT, app_title TEXT, app_category TEXT,
        language TEXT, engine TEXT, llm_provider TEXT, level TEXT, style TEXT, raw_text TEXT, clean_text TEXT,
        final_text TEXT, inserted INT, insert_method TEXT, duration_ms INT, audio_path TEXT, word_count INT, error TEXT);
    CREATE INDEX dictations_created ON dictations(created_at DESC);
    CREATE TABLE dictionary_terms(id TEXT PRIMARY KEY, term TEXT, spoken_variants TEXT, language TEXT,
        source TEXT CHECK(source IN('manual','learned','pack')), created_at TEXT, uses INT);
    CREATE TABLE snippets(id TEXT PRIMARY KEY, trigger TEXT UNIQUE, body TEXT, created_at TEXT, uses INT);
    CREATE TABLE styles(id TEXT PRIMARY KEY, name TEXT, description TEXT, is_builtin INT);
    CREATE TABLE app_rules(id TEXT PRIMARY KEY, match_process TEXT, match_title_regex TEXT, match_url_regex TEXT,
        category TEXT, style_id TEXT, paste_key TEXT, newline_mode TEXT, priority INT);
    CREATE TABLE meetings(id TEXT PRIMARY KEY, title TEXT, type TEXT, attendees TEXT, languages TEXT, started_at TEXT,
        ended_at TEXT, duration_ms INT, consent_confirmed INT, mic_track_path TEXT, system_track_path TEXT, status TEXT);
    CREATE TABLE segments(id TEXT PRIMARY KEY, meeting_id TEXT REFERENCES meetings(id) ON DELETE CASCADE,
        track TEXT CHECK(track IN('mic','system','mixed')), speaker TEXT, start_ms INT, end_ms INT, text TEXT,
        is_final INT, engine TEXT);
    CREATE TABLE thoughts(id TEXT PRIMARY KEY, meeting_id TEXT REFERENCES meetings(id) ON DELETE CASCADE, at_ms INT,
        text TEXT, created_at TEXT);
    CREATE TABLE summaries(id TEXT PRIMARY KEY, meeting_id TEXT REFERENCES meetings(id) ON DELETE CASCADE, version INT,
        created_at TEXT, provider TEXT, json TEXT, edited_json TEXT, is_current INT);
    CREATE TABLE notes(id TEXT PRIMARY KEY, created_at TEXT, updated_at TEXT, title TEXT, body TEXT);
    CREATE TABLE events(id INTEGER PRIMARY KEY AUTOINCREMENT, at TEXT, kind TEXT, detail TEXT);
    "#,
    // v2: hint-only dictionary terms (never forced into the text).
    r#"
    ALTER TABLE dictionary_terms ADD COLUMN auto_replace INT NOT NULL DEFAULT 1;
    CREATE UNIQUE INDEX dictionary_terms_term ON dictionary_terms(term);
    "#,
    // v3: remember which dictations the user corrected (for the Insights page).
    r#"
    ALTER TABLE dictations ADD COLUMN fixed INT NOT NULL DEFAULT 0;
    "#,
    // v4 (0.4.2): meeting library. Hand-corrected transcript lines are stored next to the
    // recogniser's original, which is never overwritten.
    r#"
    ALTER TABLE segments ADD COLUMN edited_text TEXT;
    CREATE INDEX IF NOT EXISTS segments_meeting ON segments(meeting_id, start_ms);
    CREATE INDEX IF NOT EXISTS summaries_meeting ON summaries(meeting_id, version);
    "#,
];

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct TermRow {
    pub id: String,
    pub term: String,
    pub spoken_variants: Vec<String>,
    pub source: String,
    pub auto_replace: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SnippetRow {
    pub id: String,
    pub trigger: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct EventRow {
    pub at: String,
    pub kind: String,
    pub detail: String,
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

/// Adds a term, or updates it if the same spelling exists. Manual and learned entries win
/// over pack entries.
pub fn upsert_term(
    conn: &Connection,
    term: &str,
    variants: &[String],
    source: &str,
    auto_replace: bool,
) -> Result<(), String> {
    let term = term.trim();
    if term.is_empty() {
        return Err("A dictionary word cannot be empty".into());
    }
    let variants: Vec<String> = variants
        .iter()
        .map(|v| v.trim().to_lowercase())
        .filter(|v| !v.is_empty() && !v.eq_ignore_ascii_case(term))
        .collect();
    let existing: Option<(String, String)> = conn
        .query_row(
            "SELECT spoken_variants, source FROM dictionary_terms WHERE term = ?1",
            [term],
            |r| {
                Ok((
                    r.get::<_, Option<String>>(0)?.unwrap_or_default(),
                    r.get(1)?,
                ))
            },
        )
        .optional()
        .map_err(|e| e.to_string())?;
    match existing {
        Some((old, old_source)) => {
            let mut merged: Vec<String> = serde_json::from_str(&old).unwrap_or_default();
            for v in variants {
                if !merged.contains(&v) {
                    merged.push(v);
                }
            }
            let source = if old_source == "pack" {
                source
            } else {
                &old_source
            };
            conn.execute(
                "UPDATE dictionary_terms SET spoken_variants = ?2, source = ?3, auto_replace = ?4 WHERE term = ?1",
                params![term, serde_json::to_string(&merged).unwrap_or_default(), source, auto_replace],
            )
            .map_err(|e| e.to_string())?;
        }
        None => {
            conn.execute(
                "INSERT INTO dictionary_terms(id, term, spoken_variants, language, source, created_at, uses, auto_replace)
                 VALUES(?1, ?2, ?3, '', ?4, ?5, 0, ?6)",
                params![
                    uuid::Uuid::new_v4().to_string(),
                    term,
                    serde_json::to_string(&variants).unwrap_or_default(),
                    source,
                    now(),
                    auto_replace
                ],
            )
            .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Manual first, then learned, then packs: that is the priority for the recogniser hint.
pub fn list_terms(conn: &Connection) -> Result<Vec<TermRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, term, spoken_variants, source, auto_replace FROM dictionary_terms
             ORDER BY CASE source WHEN 'manual' THEN 0 WHEN 'learned' THEN 1 ELSE 2 END, created_at DESC, term",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(TermRow {
                id: r.get(0)?,
                term: r.get(1)?,
                spoken_variants: serde_json::from_str(
                    &r.get::<_, Option<String>>(2)?.unwrap_or_default(),
                )
                .unwrap_or_default(),
                source: r.get(3)?,
                auto_replace: r.get(4)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn delete_term(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM dictionary_terms WHERE id = ?1", [id])
        .map(|_| ())
        .map_err(|e| e.to_string())
}

pub fn upsert_snippet(conn: &Connection, trigger: &str, body: &str) -> Result<(), String> {
    let trigger = trigger.trim().to_lowercase();
    if trigger.split_whitespace().count() < 2 {
        return Err("Use at least two words for a trigger, so it is never said by accident".into());
    }
    if body.trim().is_empty() {
        return Err("The snippet text cannot be empty".into());
    }
    conn.execute(
        "INSERT INTO snippets(id, trigger, body, created_at, uses) VALUES(?1, ?2, ?3, ?4, 0)
         ON CONFLICT(trigger) DO UPDATE SET body = excluded.body",
        params![uuid::Uuid::new_v4().to_string(), trigger, body, now()],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

pub fn list_snippets(conn: &Connection) -> Result<Vec<SnippetRow>, String> {
    let mut stmt = conn
        .prepare("SELECT id, trigger, body FROM snippets ORDER BY trigger")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(SnippetRow {
                id: r.get(0)?,
                trigger: r.get(1)?,
                body: r.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn delete_snippet(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute("DELETE FROM snippets WHERE id = ?1", [id])
        .map(|_| ())
        .map_err(|e| e.to_string())
}

/// Records the user's consent to send text to a provider.
pub fn record_consent(
    conn: &Connection,
    provider: &str,
    endpoint: &str,
    model: &str,
    note: &str,
) -> Result<(), String> {
    conn.execute(
        "INSERT INTO providers(id, kind, name, endpoint, model, enabled, consent_at, data_policy_note)
         VALUES(?1, 'llm', ?1, ?2, ?3, 1, ?4, ?5)
         ON CONFLICT(id) DO UPDATE SET endpoint = excluded.endpoint, model = excluded.model, enabled = 1,
             consent_at = excluded.consent_at, data_policy_note = excluded.data_policy_note",
        params![provider, endpoint, model, now(), note],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

pub fn has_consent(conn: &Connection, provider: &str) -> bool {
    conn.query_row(
        "SELECT consent_at FROM providers WHERE id = ?1 AND enabled = 1",
        [provider],
        |r| r.get::<_, Option<String>>(0),
    )
    .ok()
    .flatten()
    .is_some()
}

pub fn revoke_consent(conn: &Connection, provider: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE providers SET enabled = 0, consent_at = NULL WHERE id = ?1",
        [provider],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// Local-only log. Every upload is written here ("Data that left this device").
pub fn log_event(conn: &Connection, kind: &str, detail: &str) {
    crate::diag::log(kind, detail);
    if let Err(e) = conn.execute(
        "INSERT INTO events(at, kind, detail) VALUES(?1, ?2, ?3)",
        params![now(), kind, detail],
    ) {
        crate::diag::log("database", &format!("could not record an event: {e}"));
    }
}

/// The id of the newest event (0 when there is none).
pub fn last_event_id(conn: &Connection) -> i64 {
    conn.query_row("SELECT COALESCE(MAX(id), 0) FROM events", [], |r| r.get(0))
        .unwrap_or(0)
}

/// Uploads logged after event `after`: (audio requests, text requests).
pub fn uploads_since(conn: &Connection, after: i64) -> (i64, i64) {
    let count = |like: &str, not: bool| -> i64 {
        let sql = if not {
            "SELECT COUNT(*) FROM events WHERE kind = 'upload' AND id > ?1 AND detail NOT LIKE ?2"
        } else {
            "SELECT COUNT(*) FROM events WHERE kind = 'upload' AND id > ?1 AND detail LIKE ?2"
        };
        conn.query_row(sql, params![after, like], |r| r.get(0))
            .unwrap_or(0)
    };
    (count("%audio%", false), count("%audio%", true))
}

pub fn list_events(conn: &Connection, kind: &str, limit: u32) -> Result<Vec<EventRow>, String> {
    let mut stmt = conn
        .prepare("SELECT at, kind, detail FROM events WHERE kind = ?1 ORDER BY id DESC LIMIT ?2")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![kind, limit], |r| {
            Ok(EventRow {
                at: r.get(0)?,
                kind: r.get(1)?,
                detail: r.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn get_dictation(conn: &Connection, id: &str) -> Result<Dictation, String> {
    conn.query_row(
        &format!("SELECT {DICTATION_COLUMNS} FROM dictations WHERE id = ?1"),
        [id],
        row_to_dictation,
    )
    .map_err(|e| e.to_string())
}

pub fn last_dictation_id(conn: &Connection) -> Option<String> {
    conn.query_row(
        "SELECT id FROM dictations WHERE final_text <> '' ORDER BY created_at DESC, rowid DESC LIMIT 1",
        [],
        |r| r.get(0),
    )
    .optional()
    .ok()
    .flatten()
}

/// Stores a finished meeting with its transcript, the user's notes and (if any) the summary.
#[allow(clippy::too_many_arguments)]
pub fn save_meeting(
    conn: &Connection,
    id: &str,
    title: &str,
    started_at: &str,
    duration_ms: u64,
    mic_path: Option<&str>,
    system_path: Option<&str>,
    segments: &[app_core::summary::Segment],
    thoughts: &[(u64, String)],
    summary_json: Option<&str>,
    provider: &str,
) -> Result<(), String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO meetings(id, title, type, attendees, languages, started_at, ended_at, duration_ms,
            consent_confirmed, mic_track_path, system_track_path, status)
         VALUES(?1, ?2, 'Meeting', '', '', ?3, ?4, ?5, 1, ?6, ?7, ?8)",
        params![
            id,
            title,
            started_at,
            now(),
            duration_ms as i64,
            mic_path,
            system_path,
            if summary_json.is_some() { "summarised" } else { "transcribed" }
        ],
    )
    .map_err(|e| e.to_string())?;
    for s in segments {
        tx.execute(
            "INSERT INTO segments(id, meeting_id, track, speaker, start_ms, end_ms, text, is_final, engine)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, 'local')",
            params![
                format!("{id}:{}", s.id),
                id,
                if s.speaker == "Me" || s.speaker == app_core::summary::MIC {
                    "mic"
                } else {
                    "system"
                },
                s.speaker,
                s.start_ms as i64,
                s.end_ms as i64,
                s.text
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    for (at, text) in thoughts {
        tx.execute(
            "INSERT INTO thoughts(id, meeting_id, at_ms, text, created_at) VALUES(?1, ?2, ?3, ?4, ?5)",
            params![uuid::Uuid::new_v4().to_string(), id, *at as i64, text, now()],
        )
        .map_err(|e| e.to_string())?;
    }
    if let Some(json) = summary_json {
        tx.execute(
            "INSERT INTO summaries(id, meeting_id, version, created_at, provider, json, edited_json, is_current)
             VALUES(?1, ?2, 1, ?3, ?4, ?5, NULL, 1)",
            params![uuid::Uuid::new_v4().to_string(), id, now(), provider, json],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())
}

// ---------- Meeting library ----------

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct MeetingRow {
    pub id: String,
    pub title: String,
    pub started_at: String,
    pub duration_ms: i64,
    /// "summarised", "transcribed" or "interrupted".
    pub status: String,
    pub has_audio: bool,
    pub has_summary: bool,
    pub segments: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct SavedSegment {
    pub id: String,
    pub start_ms: i64,
    pub end_ms: i64,
    pub speaker: String,
    /// What the recogniser wrote (never changed).
    pub original: String,
    /// The user's correction, if any.
    pub edited: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SavedMeeting {
    pub row: MeetingRow,
    pub mic_path: Option<String>,
    pub system_path: Option<String>,
    pub segments: Vec<SavedSegment>,
    pub thoughts: Vec<(u64, String)>,
    /// The current summary JSON, if any.
    pub summary_json: Option<String>,
}

const MEETING_ROW: &str = "m.id, m.title, m.started_at, m.duration_ms, m.status,
    (m.mic_track_path IS NOT NULL OR m.system_track_path IS NOT NULL),
    EXISTS(SELECT 1 FROM summaries s WHERE s.meeting_id = m.id AND s.is_current = 1),
    (SELECT COUNT(*) FROM segments g WHERE g.meeting_id = m.id)";

fn row_to_meeting(r: &rusqlite::Row) -> rusqlite::Result<MeetingRow> {
    Ok(MeetingRow {
        id: r.get(0)?,
        title: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
        started_at: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
        duration_ms: r.get::<_, Option<i64>>(3)?.unwrap_or(0),
        status: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
        has_audio: r.get(5)?,
        has_summary: r.get(6)?,
        segments: r.get(7)?,
    })
}

/// Saved meetings, newest first. `query` searches titles, transcript lines (original and
/// corrected) and the notes.
pub fn list_meetings(conn: &Connection, query: &str) -> Result<Vec<MeetingRow>, String> {
    let q = query.trim();
    let like = format!("%{}%", q.replace(['%', '_'], ""));
    let mut stmt = conn
        .prepare(&format!(
            "SELECT {MEETING_ROW} FROM meetings m
             WHERE ?1 = '' OR m.title LIKE ?2
                OR EXISTS(SELECT 1 FROM segments g WHERE g.meeting_id = m.id
                          AND (g.text LIKE ?2 OR g.edited_text LIKE ?2))
                OR EXISTS(SELECT 1 FROM summaries s WHERE s.meeting_id = m.id
                          AND s.is_current = 1 AND s.json LIKE ?2)
             ORDER BY m.started_at DESC"
        ))
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![q, like], row_to_meeting)
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn get_meeting(conn: &Connection, id: &str) -> Result<SavedMeeting, String> {
    let (row, mic_path, system_path) = conn
        .query_row(
            &format!(
                "SELECT {MEETING_ROW}, m.mic_track_path, m.system_track_path FROM meetings m WHERE m.id = ?1"
            ),
            [id],
            |r| Ok((row_to_meeting(r)?, r.get(8)?, r.get(9)?)),
        )
        .map_err(|_| "That meeting no longer exists.".to_string())?;
    let mut stmt = conn
        .prepare(
            "SELECT id, start_ms, end_ms, speaker, text, edited_text FROM segments
             WHERE meeting_id = ?1 ORDER BY start_ms, id",
        )
        .map_err(|e| e.to_string())?;
    let segments = stmt
        .query_map([id], |r| {
            let full: String = r.get(0)?;
            Ok(SavedSegment {
                id: full
                    .split_once(':')
                    .map_or(full.clone(), |(_, s)| s.to_string()),
                start_ms: r.get(1)?,
                end_ms: r.get(2)?,
                speaker: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
                original: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
                edited: r.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let mut stmt = conn
        .prepare("SELECT at_ms, text FROM thoughts WHERE meeting_id = ?1 ORDER BY at_ms")
        .map_err(|e| e.to_string())?;
    let thoughts = stmt
        .query_map([id], |r| Ok((r.get::<_, i64>(0)? as u64, r.get(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let summary_json = conn
        .query_row(
            "SELECT COALESCE(edited_json, json) FROM summaries WHERE meeting_id = ?1 AND is_current = 1
             ORDER BY version DESC LIMIT 1",
            [id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    Ok(SavedMeeting {
        row,
        mic_path,
        system_path,
        segments,
        thoughts,
        summary_json,
    })
}

pub fn rename_meeting(conn: &Connection, id: &str, title: &str) -> Result<(), String> {
    let title = title.trim();
    if title.is_empty() {
        return Err("The title cannot be empty.".into());
    }
    match conn.execute(
        "UPDATE meetings SET title = ?2 WHERE id = ?1",
        params![id, title],
    ) {
        Ok(1) => Ok(()),
        Ok(_) => Err("That meeting no longer exists.".into()),
        Err(e) => Err(e.to_string()),
    }
}

/// Stores the user's correction of one transcript line. The original stays in `text`.
/// An empty or unchanged correction removes the edit.
pub fn edit_segment(
    conn: &Connection,
    meeting_id: &str,
    segment_id: &str,
    text: &str,
) -> Result<(), String> {
    let full = format!("{meeting_id}:{segment_id}");
    let original: String = conn
        .query_row("SELECT text FROM segments WHERE id = ?1", [&full], |r| {
            r.get(0)
        })
        .map_err(|_| "That transcript line no longer exists.".to_string())?;
    let edited = (!text.trim().is_empty() && text.trim() != original.trim()).then(|| text.trim());
    conn.execute(
        "UPDATE segments SET edited_text = ?2 WHERE id = ?1",
        params![full, edited],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// Replaces the transcript of a meeting (used when an interrupted recording is transcribed).
pub fn replace_segments(
    conn: &Connection,
    meeting_id: &str,
    segments: &[app_core::summary::Segment],
    status: &str,
) -> Result<(), String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM segments WHERE meeting_id = ?1", [meeting_id])
        .map_err(|e| e.to_string())?;
    for s in segments {
        tx.execute(
            "INSERT INTO segments(id, meeting_id, track, speaker, start_ms, end_ms, text, is_final, engine)
             VALUES(?1, ?2, ?3, ?4, ?5, ?6, ?7, 1, 'local')",
            params![
                format!("{meeting_id}:{}", s.id),
                meeting_id,
                if s.speaker == "Me" || s.speaker == app_core::summary::MIC {
                    "mic"
                } else {
                    "system"
                },
                s.speaker,
                s.start_ms as i64,
                s.end_ms as i64,
                s.text
            ],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.execute(
        "UPDATE meetings SET status = ?2 WHERE id = ?1",
        params![meeting_id, status],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

/// Saves a new version of the notes; older versions are kept but no longer current.
pub fn save_summary_version(
    conn: &Connection,
    meeting_id: &str,
    json: &str,
    provider: &str,
) -> Result<(), String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let version: i64 = tx
        .query_row(
            "SELECT COALESCE(MAX(version), 0) + 1 FROM summaries WHERE meeting_id = ?1",
            [meeting_id],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE summaries SET is_current = 0 WHERE meeting_id = ?1",
        [meeting_id],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "INSERT INTO summaries(id, meeting_id, version, created_at, provider, json, edited_json, is_current)
         VALUES(?1, ?2, ?3, ?4, ?5, ?6, NULL, 1)",
        params![
            uuid::Uuid::new_v4().to_string(),
            meeting_id,
            version,
            now(),
            provider,
            json
        ],
    )
    .map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE meetings SET status = 'summarised' WHERE id = ?1",
        [meeting_id],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}

/// A recording whose app stopped before the meeting was saved (crash, power cut).
pub fn insert_interrupted(
    conn: &Connection,
    id: &str,
    started_at: &str,
    duration_ms: u64,
    mic_path: Option<&str>,
    system_path: Option<&str>,
) -> Result<(), String> {
    conn.execute(
        "INSERT OR IGNORE INTO meetings(id, title, type, attendees, languages, started_at, ended_at,
            duration_ms, consent_confirmed, mic_track_path, system_track_path, status)
         VALUES(?1, 'Interrupted recording', 'Meeting', '', '', ?2, NULL, ?3, 1, ?4, ?5, 'interrupted')",
        params![id, started_at, duration_ms as i64, mic_path, system_path],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

pub fn meeting_exists(conn: &Connection, id: &str) -> bool {
    conn.query_row("SELECT 1 FROM meetings WHERE id = ?1", [id], |_| Ok(()))
        .optional()
        .ok()
        .flatten()
        .is_some()
}

/// Deletes a meeting with its transcript, notes and summaries (one transaction). Returns its
/// audio file paths so the caller can delete the files.
pub fn delete_meeting(conn: &Connection, id: &str) -> Result<Vec<String>, String> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    let paths: (Option<String>, Option<String>) = tx
        .query_row(
            "SELECT mic_track_path, system_track_path FROM meetings WHERE id = ?1",
            [id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .map_err(|_| "That meeting no longer exists.".to_string())?;
    for table in ["segments", "thoughts", "summaries"] {
        tx.execute(&format!("DELETE FROM {table} WHERE meeting_id = ?1"), [id])
            .map_err(|e| e.to_string())?;
    }
    tx.execute("DELETE FROM meetings WHERE id = ?1", [id])
        .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok([paths.0, paths.1].into_iter().flatten().collect())
}

pub fn all_meeting_ids(conn: &Connection) -> Vec<String> {
    conn.prepare("SELECT id FROM meetings")
        .and_then(|mut s| s.query_map([], |r| r.get(0))?.collect())
        .unwrap_or_default()
}

// ---------- Retention ----------

/// Dictation audio files recorded before `before`: (dictation id, path).
pub fn dictation_audio_before(conn: &Connection, before: &str) -> Vec<(String, String)> {
    conn.prepare(
        "SELECT id, audio_path FROM dictations WHERE audio_path IS NOT NULL AND created_at < ?1",
    )
    .and_then(|mut s| {
        s.query_map([before], |r| Ok((r.get(0)?, r.get(1)?)))?
            .collect()
    })
    .unwrap_or_default()
}

/// Called once the file is gone.
pub fn clear_dictation_audio(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE dictations SET audio_path = NULL WHERE id = ?1",
        [id],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

/// Meeting audio recorded before `before`: (meeting id, file paths).
pub fn meeting_audio_before(conn: &Connection, before: &str) -> Vec<(String, Vec<String>)> {
    conn.prepare(
        "SELECT id, mic_track_path, system_track_path FROM meetings
         WHERE (mic_track_path IS NOT NULL OR system_track_path IS NOT NULL) AND started_at < ?1",
    )
    .and_then(|mut s| {
        s.query_map([before], |r| {
            let a: Option<String> = r.get(1)?;
            let b: Option<String> = r.get(2)?;
            Ok((r.get(0)?, [a, b].into_iter().flatten().collect()))
        })?
        .collect()
    })
    .unwrap_or_default()
}

/// Called once the files are gone. The transcript and notes stay.
pub fn clear_meeting_audio(conn: &Connection, id: &str) -> Result<(), String> {
    conn.execute(
        "UPDATE meetings SET mic_track_path = NULL, system_track_path = NULL WHERE id = ?1",
        [id],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

pub fn dictations_before(conn: &Connection, before: &str) -> Vec<String> {
    conn.prepare("SELECT id FROM dictations WHERE created_at < ?1")
        .and_then(|mut s| s.query_map([before], |r| r.get(0))?.collect())
        .unwrap_or_default()
}

pub fn meetings_before(conn: &Connection, before: &str) -> Vec<String> {
    conn.prepare("SELECT id FROM meetings WHERE started_at < ?1")
        .and_then(|mut s| s.query_map([before], |r| r.get(0))?.collect())
        .unwrap_or_default()
}

/// Saves a hand-corrected text. The recogniser's original stays untouched in `raw_text`.
pub fn update_final_text(conn: &Connection, id: &str, text: &str) -> Result<(), String> {
    let n = conn
        .execute(
            "UPDATE dictations SET final_text = ?2, word_count = ?3, fixed = 1 WHERE id = ?1",
            params![id, text, text.split_whitespace().count() as i64],
        )
        .map_err(|e| e.to_string())?;
    if n == 1 {
        Ok(())
    } else {
        Err("That dictation no longer exists".into())
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Dictation {
    pub id: String,
    pub created_at: String,
    pub app_process: String,
    pub app_title: String,
    pub app_category: String,
    pub language: String,
    pub engine: String,
    pub level: String,
    pub raw_text: String,
    pub final_text: String,
    pub inserted: bool,
    pub insert_method: String,
    pub duration_ms: i64,
    pub audio_path: Option<String>,
    pub word_count: i64,
    pub error: Option<String>,
    /// Who cleaned the text: "rules", or the AI provider id, or "rules (AI skipped: ...)".
    pub cleanup: String,
}

pub fn open(path: &Path) -> Result<Connection, String> {
    let conn = Connection::open(path).map_err(|e| format!("could not open database: {e}"))?;
    migrate(&conn)?;
    Ok(conn)
}

#[cfg(test)]
pub fn open_in_memory() -> Result<Connection, String> {
    let conn = Connection::open_in_memory().map_err(|e| e.to_string())?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<(), String> {
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")
        .map_err(|e| e.to_string())?;
    let version: i64 = conn
        .query_row("PRAGMA user_version", [], |r| r.get(0))
        .map_err(|e| e.to_string())?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(version as usize) {
        conn.execute_batch(&format!(
            "BEGIN; {sql} PRAGMA user_version = {}; COMMIT;",
            i + 1
        ))
        .map_err(|e| format!("database migration {} failed: {e}", i + 1))?;
    }
    Ok(())
}

pub fn get_setting(conn: &Connection, key: &str) -> Option<String> {
    conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |r| {
        r.get(0)
    })
    .optional()
    .ok()
    .flatten()
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO settings(key, value) VALUES(?1, ?2) ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

pub fn insert_dictation(conn: &Connection, d: &Dictation) -> Result<(), String> {
    conn.execute(
        "INSERT INTO dictations(id, created_at, app_process, app_title, app_category, language, engine, level,
            raw_text, clean_text, final_text, inserted, insert_method, duration_ms, audio_path, word_count, error,
            llm_provider)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?10,?11,?12,?13,?14,?15,?16,?17)",
        params![
            d.id, d.created_at, d.app_process, d.app_title, d.app_category, d.language, d.engine, d.level, d.raw_text,
            d.final_text, d.inserted, d.insert_method, d.duration_ms, d.audio_path, d.word_count, d.error,
            d.cleanup
        ],
    )
    .map(|_| ())
    .map_err(|e| e.to_string())
}

fn row_to_dictation(r: &rusqlite::Row) -> rusqlite::Result<Dictation> {
    Ok(Dictation {
        id: r.get(0)?,
        created_at: r.get(1)?,
        app_process: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
        app_title: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
        app_category: r.get::<_, Option<String>>(4)?.unwrap_or_default(),
        language: r.get::<_, Option<String>>(5)?.unwrap_or_default(),
        engine: r.get::<_, Option<String>>(6)?.unwrap_or_default(),
        level: r.get::<_, Option<String>>(7)?.unwrap_or_default(),
        raw_text: r.get::<_, Option<String>>(8)?.unwrap_or_default(),
        final_text: r.get::<_, Option<String>>(9)?.unwrap_or_default(),
        inserted: r.get::<_, Option<bool>>(10)?.unwrap_or(false),
        insert_method: r.get::<_, Option<String>>(11)?.unwrap_or_default(),
        duration_ms: r.get::<_, Option<i64>>(12)?.unwrap_or(0),
        audio_path: r.get(13)?,
        word_count: r.get::<_, Option<i64>>(14)?.unwrap_or(0),
        error: r.get(15)?,
        cleanup: r.get::<_, Option<String>>(16)?.unwrap_or_default(),
    })
}

const DICTATION_COLUMNS: &str =
    "id, created_at, app_process, app_title, app_category, language, engine, level,
    raw_text, final_text, inserted, insert_method, duration_ms, audio_path, word_count, error, llm_provider";

pub fn list_dictations(conn: &Connection, limit: u32) -> Result<Vec<Dictation>, String> {
    let mut stmt = conn
        .prepare(&format!("SELECT {DICTATION_COLUMNS} FROM dictations ORDER BY created_at DESC, rowid DESC LIMIT ?1"))
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([limit], row_to_dictation)
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

/// The most recent dictation that produced text.
pub fn last_text(conn: &Connection) -> Option<String> {
    conn.query_row(
        "SELECT final_text FROM dictations WHERE final_text <> '' ORDER BY created_at DESC, rowid DESC LIMIT 1",
        [],
        |r| r.get(0),
    )
    .optional()
    .ok()
    .flatten()
}

/// Deletes one dictation and returns its audio file path, if it had one.
pub fn delete_dictation(conn: &Connection, id: &str) -> Result<Option<String>, String> {
    let audio: Option<String> = conn
        .query_row(
            "SELECT audio_path FROM dictations WHERE id = ?1",
            [id],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .flatten();
    conn.execute("DELETE FROM dictations WHERE id = ?1", [id])
        .map_err(|e| e.to_string())?;
    Ok(audio)
}

/// One dictation, reduced to what the Insights page counts (no text).
#[derive(Debug, Clone, PartialEq)]
pub struct UsageRow {
    pub created_at: String,
    pub words: i64,
    pub duration_ms: i64,
    pub app_process: String,
    pub level: String,
    pub fixed: bool,
}

/// Every dictation that produced text, oldest first, for local statistics.
pub fn usage_rows(conn: &Connection) -> Result<Vec<UsageRow>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT created_at, COALESCE(word_count, 0), COALESCE(duration_ms, 0),
                    COALESCE(app_process, ''), COALESCE(level, ''), COALESCE(fixed, 0)
             FROM dictations WHERE COALESCE(final_text, '') <> '' ORDER BY created_at",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(UsageRow {
                created_at: r.get(0)?,
                words: r.get(1)?,
                duration_ms: r.get(2)?,
                app_process: r.get(3)?,
                level: r.get(4)?,
                fixed: r.get::<_, i64>(5)? != 0,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

/// Deletes every word learned from the user's corrections; returns how many were deleted.
pub fn delete_learned_terms(conn: &Connection) -> Result<usize, String> {
    conn.execute("DELETE FROM dictionary_terms WHERE source = 'learned'", [])
        .map_err(|e| e.to_string())
}

/// Number of dictionary entries from one source ("manual", "learned" or "pack").
pub fn count_terms(conn: &Connection, source: &str) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM dictionary_terms WHERE source = ?1",
        [source],
        |r| r.get(0),
    )
    .unwrap_or(0)
}

/// Words dictated since `since` (an ISO timestamp). A local statistic only.
pub fn words_since(conn: &Connection, since: &str) -> i64 {
    conn.query_row(
        "SELECT COALESCE(SUM(word_count), 0) FROM dictations WHERE created_at >= ?1",
        [since],
        |r| r.get(0),
    )
    .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample(id: &str, at: &str, text: &str) -> Dictation {
        Dictation {
            id: id.into(),
            created_at: at.into(),
            app_process: "notepad.exe".into(),
            app_title: "Untitled".into(),
            app_category: "Docs".into(),
            language: "en".into(),
            engine: "Local - base".into(),
            level: "LIGHT".into(),
            raw_text: format!("um {text}"),
            final_text: text.into(),
            inserted: true,
            insert_method: "clipboard:ctrl+v".into(),
            duration_ms: 1200,
            audio_path: Some(format!("{id}.wav")),
            word_count: text.split_whitespace().count() as i64,
            error: None,
            cleanup: "rules".into(),
        }
    }

    #[test]
    fn uploads_of_one_dictation_are_counted() {
        let conn = open_in_memory().unwrap();
        log_event(
            &conn,
            "upload",
            "groq (whisper-large-v3): dictation audio for speech recognition, 10 bytes, ok",
        );
        let before = last_event_id(&conn);
        log_event(&conn, "upload", "groq (whisper-large-v3): dictation audio for speech recognition (no fixed language), 10 bytes, ok");
        log_event(
            &conn,
            "upload",
            "groq (whisper-large-v3): dictation audio for speech recognition, 10 bytes, ok",
        );
        log_event(
            &conn,
            "upload",
            "groq (qwen): dictation text for clean-up, 10 bytes, ok",
        );
        log_event(&conn, "timing", "audio 1 ms");
        assert_eq!(uploads_since(&conn, before), (2, 1));
        let (forgot, kept) = (
            upsert_term(&conn, "Henglish", &[], "learned", true),
            upsert_term(&conn, "Groq", &[], "manual", false),
        );
        assert!(forgot.is_ok() && kept.is_ok());
        assert_eq!(delete_learned_terms(&conn).unwrap(), 1);
        assert_eq!(count_terms(&conn, "manual"), 1);
    }

    #[test]
    fn migration_is_idempotent_and_creates_every_table() {
        let conn = open_in_memory().unwrap();
        migrate(&conn).unwrap();
        let tables: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name IN ('settings','providers','dictations',
                 'dictionary_terms','snippets','styles','app_rules','meetings','segments','thoughts','summaries','notes','events')",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(tables, 13);
    }

    #[test]
    fn dictations_round_trip_newest_first_with_raw_kept_separately() {
        let conn = open_in_memory().unwrap();
        insert_dictation(&conn, &sample("a", "2026-11-12T10:00:00Z", "first one")).unwrap();
        insert_dictation(
            &conn,
            &sample("b", "2026-11-12T11:00:00Z", "second one here"),
        )
        .unwrap();
        let all = list_dictations(&conn, 10).unwrap();
        assert_eq!(
            all.iter().map(|d| d.id.as_str()).collect::<Vec<_>>(),
            ["b", "a"]
        );
        assert_eq!(
            all[0],
            sample("b", "2026-11-12T11:00:00Z", "second one here")
        );
        assert_ne!(all[0].raw_text, all[0].final_text);
        assert_eq!(last_text(&conn).as_deref(), Some("second one here"));
        assert_eq!(words_since(&conn, "2026-11-12T10:30:00Z"), 3);
        assert_eq!(
            delete_dictation(&conn, "b").unwrap().as_deref(),
            Some("b.wav")
        );
        assert_eq!(last_text(&conn).as_deref(), Some("first one"));
    }

    #[test]
    fn dictionary_snippets_consent_and_events() {
        let conn = open_in_memory().unwrap();
        upsert_term(&conn, "EBITDA", &[], "pack", true).unwrap();
        upsert_term(&conn, "EBITDA", &["Ebit Da".into()], "learned", true).unwrap();
        upsert_term(&conn, "Supabase", &["super base".into()], "manual", true).unwrap();
        assert!(upsert_term(&conn, "  ", &[], "manual", true).is_err());
        let terms = list_terms(&conn).unwrap();
        assert_eq!(terms.len(), 2);
        assert_eq!(terms[0].term, "Supabase", "manual entries come first");
        let ebitda = terms.iter().find(|t| t.term == "EBITDA").unwrap();
        assert_eq!(ebitda.spoken_variants, vec!["ebit da"]);
        assert_eq!(
            ebitda.source, "learned",
            "a learned fix upgrades a pack entry"
        );
        delete_term(&conn, &ebitda.id).unwrap();
        assert_eq!(list_terms(&conn).unwrap().len(), 1);

        assert!(
            upsert_snippet(&conn, "address", "x").is_err(),
            "one-word triggers are refused"
        );
        upsert_snippet(&conn, "Insert My Address", "12 Station Road").unwrap();
        upsert_snippet(&conn, "insert my address", "14 Station Road").unwrap();
        let s = list_snippets(&conn).unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!(
            (s[0].trigger.as_str(), s[0].body.as_str()),
            ("insert my address", "14 Station Road")
        );

        assert!(!has_consent(&conn, "groq"));
        record_consent(&conn, "groq", "https://x", "m", "note").unwrap();
        assert!(has_consent(&conn, "groq"));
        revoke_consent(&conn, "groq").unwrap();
        assert!(!has_consent(&conn, "groq"));

        log_event(&conn, "upload", "groq: 120 bytes");
        log_event(&conn, "other", "x");
        let ev = list_events(&conn, "upload", 10).unwrap();
        assert_eq!(ev.len(), 1);
        assert_eq!(ev[0].detail, "groq: 120 bytes");
    }

    #[test]
    fn meeting_library_lifecycle() {
        use app_core::summary::{Segment, MIC, SYSTEM};
        let conn = open_in_memory().unwrap();
        let segs = vec![
            Segment {
                id: "s1".into(),
                start_ms: 0,
                end_ms: 4000,
                speaker: MIC.into(),
                text: "Sales went up 12 percent.".into(),
            },
            Segment {
                id: "s2".into(),
                start_ms: 4200,
                end_ms: 9000,
                speaker: SYSTEM.into(),
                text: "Priya will send the report.".into(),
            },
        ];
        save_meeting(
            &conn,
            "m1",
            "Weekly review",
            "2026-11-01T10:00:00Z",
            9000,
            Some("C:/data/meetings/m1/mic.wav"),
            Some("C:/data/meetings/m1/system.wav"),
            &segs,
            &[(3000, "ask about margin".into())],
            Some("{\"overview\":{\"title\":\"Weekly review\"}}"),
            "groq",
        )
        .unwrap();
        save_meeting(
            &conn,
            "m2",
            "Hiring sync",
            "2026-11-15T10:00:00Z",
            1000,
            None,
            None,
            &[],
            &[],
            None,
            "",
        )
        .unwrap();
        insert_interrupted(
            &conn,
            "m3",
            "2026-11-16T09:00:00Z",
            60000,
            Some("x.wav"),
            None,
        )
        .unwrap();

        let all = list_meetings(&conn, "").unwrap();
        assert_eq!(
            all.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            ["m3", "m2", "m1"]
        );
        assert_eq!(all[0].status, "interrupted");
        assert!(all[2].has_audio && all[2].has_summary && all[2].segments == 2);
        assert_eq!(
            list_meetings(&conn, "priya").unwrap()[0].id,
            "m1",
            "searches the transcript"
        );
        assert_eq!(
            list_meetings(&conn, "hiring").unwrap()[0].id,
            "m2",
            "searches titles"
        );

        rename_meeting(&conn, "m1", "Weekly finance review").unwrap();
        edit_segment(&conn, "m1", "s2", "Priya will send the margin report.").unwrap();
        let m = get_meeting(&conn, "m1").unwrap();
        assert_eq!(m.row.title, "Weekly finance review");
        assert_eq!(
            m.segments[1].original, "Priya will send the report.",
            "original kept"
        );
        assert_eq!(
            m.segments[1].edited.as_deref(),
            Some("Priya will send the margin report.")
        );
        assert_eq!(m.thoughts, [(3000, "ask about margin".to_string())]);
        assert!(list_meetings(&conn, "margin report")
            .unwrap()
            .iter()
            .any(|x| x.id == "m1"));

        save_summary_version(&conn, "m1", "{\"v\":2}", "groq").unwrap();
        assert_eq!(
            get_meeting(&conn, "m1").unwrap().summary_json.as_deref(),
            Some("{\"v\":2}")
        );

        let due = meeting_audio_before(&conn, "2026-11-10T00:00:00Z");
        assert_eq!(due.len(), 1);
        assert_eq!(due[0].1.len(), 2);
        clear_meeting_audio(&conn, "m1").unwrap();
        assert!(!get_meeting(&conn, "m1").unwrap().row.has_audio);
        assert_eq!(
            get_meeting(&conn, "m1").unwrap().segments.len(),
            2,
            "transcript stays"
        );

        assert_eq!(delete_meeting(&conn, "m3").unwrap(), ["x.wav"]);
        assert!(delete_meeting(&conn, "m1").unwrap().is_empty());
        for table in ["segments", "thoughts", "summaries"] {
            let n: i64 = conn
                .query_row(
                    &format!("SELECT COUNT(*) FROM {table} WHERE meeting_id='m1'"),
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(n, 0, "{table} rows removed with the meeting");
        }
        assert_eq!(all_meeting_ids(&conn), ["m2"]);
        assert!(get_meeting(&conn, "m1").is_err());
    }

    #[test]
    fn hand_fixes_keep_the_original() {
        let conn = open_in_memory().unwrap();
        insert_dictation(
            &conn,
            &sample("a", "2026-11-12T10:00:00Z", "the ebit da margin"),
        )
        .unwrap();
        assert_eq!(last_dictation_id(&conn).as_deref(), Some("a"));
        update_final_text(&conn, "a", "the EBITDA margin").unwrap();
        let d = get_dictation(&conn, "a").unwrap();
        assert_eq!(d.final_text, "the EBITDA margin");
        assert_eq!(d.raw_text, "um the ebit da margin");
        assert!(update_final_text(&conn, "missing", "x").is_err());
    }

    #[test]
    fn settings_and_audio_expiry() {
        let conn = open_in_memory().unwrap();
        assert_eq!(get_setting(&conn, "model"), None);
        set_setting(&conn, "model", "base").unwrap();
        set_setting(&conn, "model", "turbo").unwrap();
        assert_eq!(get_setting(&conn, "model").as_deref(), Some("turbo"));

        insert_dictation(&conn, &sample("old", "2026-11-01T10:00:00Z", "old")).unwrap();
        insert_dictation(&conn, &sample("new", "2026-11-12T10:00:00Z", "new")).unwrap();
        let due = dictation_audio_before(&conn, "2026-11-05T00:00:00Z");
        assert_eq!(due, [("old".to_string(), "old.wav".to_string())]);
        clear_dictation_audio(&conn, "old").unwrap();
        assert!(dictation_audio_before(&conn, "2026-11-05T00:00:00Z").is_empty());
        assert_eq!(dictations_before(&conn, "2026-11-05T00:00:00Z"), ["old"]);
        let all = list_dictations(&conn, 10).unwrap();
        assert_eq!(all.iter().find(|d| d.id == "old").unwrap().audio_path, None);
        assert!(all
            .iter()
            .find(|d| d.id == "new")
            .unwrap()
            .audio_path
            .is_some());
    }
}

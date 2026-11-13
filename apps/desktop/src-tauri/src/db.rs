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
];

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
            raw_text, clean_text, final_text, inserted, insert_method, duration_ms, audio_path, word_count, error)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?10,?11,?12,?13,?14,?15,?16)",
        params![
            d.id, d.created_at, d.app_process, d.app_title, d.app_category, d.language, d.engine, d.level, d.raw_text,
            d.final_text, d.inserted, d.insert_method, d.duration_ms, d.audio_path, d.word_count, d.error
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
    })
}

const DICTATION_COLUMNS: &str =
    "id, created_at, app_process, app_title, app_category, language, engine, level,
    raw_text, final_text, inserted, insert_method, duration_ms, audio_path, word_count, error";

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

/// Words dictated since `since` (an ISO timestamp). A local statistic only.
pub fn words_since(conn: &Connection, since: &str) -> i64 {
    conn.query_row(
        "SELECT COALESCE(SUM(word_count), 0) FROM dictations WHERE created_at >= ?1",
        [since],
        |r| r.get(0),
    )
    .unwrap_or(0)
}

/// Audio files of dictations older than `before`; their paths are cleared in the database.
pub fn expire_audio(conn: &Connection, before: &str) -> Vec<String> {
    let paths: Vec<String> = conn
        .prepare(
            "SELECT audio_path FROM dictations WHERE audio_path IS NOT NULL AND created_at < ?1",
        )
        .and_then(|mut s| s.query_map([before], |r| r.get(0))?.collect())
        .unwrap_or_default();
    let _ = conn.execute(
        "UPDATE dictations SET audio_path = NULL WHERE audio_path IS NOT NULL AND created_at < ?1",
        [before],
    );
    paths
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
        }
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
    fn settings_and_audio_expiry() {
        let conn = open_in_memory().unwrap();
        assert_eq!(get_setting(&conn, "model"), None);
        set_setting(&conn, "model", "base").unwrap();
        set_setting(&conn, "model", "turbo").unwrap();
        assert_eq!(get_setting(&conn, "model").as_deref(), Some("turbo"));

        insert_dictation(&conn, &sample("old", "2026-11-01T10:00:00Z", "old")).unwrap();
        insert_dictation(&conn, &sample("new", "2026-11-12T10:00:00Z", "new")).unwrap();
        assert_eq!(expire_audio(&conn, "2026-11-05T00:00:00Z"), ["old.wav"]);
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

//! Usage statistics for the Home and Insights pages, computed only from the local history.
//! Nothing here is estimated except "time saved", which states its assumption.

use std::collections::BTreeMap;

use chrono::{Duration, NaiveDate};
use serde::Serialize;

use crate::db::UsageRow;

/// Typing speed used for "time saved" (a common average for typing on a keyboard).
pub const TYPING_WPM: f64 = 40.0;
/// Days shown in the activity calendar (17 full weeks).
pub const CALENDAR_DAYS: i64 = 119;

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct DayCount {
    pub date: String,
    pub words: i64,
    pub dictations: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct AppUse {
    pub name: String,
    pub words: i64,
    /// Share of all dictated words, 0..1.
    pub share: f64,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Insights {
    pub total_words: i64,
    pub total_dictations: i64,
    pub commands: i64,
    pub words_today: i64,
    pub words_this_week: i64,
    /// Words per minute of speaking, over dictations of at least one second.
    pub speaking_wpm: Option<f64>,
    /// Minutes saved compared with typing at `TYPING_WPM` (never negative).
    pub minutes_saved: f64,
    pub typing_wpm: f64,
    pub current_streak: i64,
    pub longest_streak: i64,
    pub active_days: i64,
    pub days: Vec<DayCount>,
    pub apps: Vec<AppUse>,
    pub fixes: i64,
    pub learned_words: i64,
    pub your_words: i64,
    pub first_day: Option<String>,
}

/// "WINWORD.EXE" -> "Word". Unknown programs keep their own name without ".exe".
pub fn app_name(process: &str) -> String {
    let p = process.trim();
    let base = p
        .strip_suffix(".exe")
        .or_else(|| p.strip_suffix(".EXE"))
        .unwrap_or(p);
    let known = match base.to_lowercase().as_str() {
        "" => "Other",
        "winword" => "Word",
        "excel" => "Excel",
        "powerpnt" => "PowerPoint",
        "outlook" | "olk" => "Outlook",
        "onenote" => "OneNote",
        "chrome" => "Chrome",
        "msedge" => "Edge",
        "firefox" => "Firefox",
        "brave" => "Brave",
        "whatsapp" | "whatsapp.root" => "WhatsApp",
        "slack" => "Slack",
        "ms-teams" | "teams" => "Teams",
        "zoom" => "Zoom",
        "code" => "VS Code",
        "cursor" => "Cursor",
        "notepad" => "Notepad",
        "windowsterminal" | "cmd" | "powershell" | "pwsh" => "Terminal",
        "explorer" => "File Explorer",
        "telegram" => "Telegram",
        "notion" => "Notion",
        "obsidian" => "Obsidian",
        "claude" => "Claude",
        "chatgpt" => "ChatGPT",
        "app-desktop" => app_core::brand::APP_NAME,
        _ => "",
    };
    if !known.is_empty() {
        return known.to_string();
    }
    let mut c = base.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_else(|| "Other".into())
}

/// Longest run of consecutive days in `days` (sorted, unique).
fn longest_run(days: &[NaiveDate]) -> i64 {
    let (mut best, mut run) = (0i64, 0i64);
    let mut prev: Option<NaiveDate> = None;
    for d in days {
        run = match prev {
            Some(p) if *d - p == Duration::days(1) => run + 1,
            _ => 1,
        };
        best = best.max(run);
        prev = Some(*d);
    }
    best
}

/// Days in a row up to today. A day without dictation yet today does not break the streak
/// until the day is over: it counts back from yesterday in that case.
fn current_run(days: &[NaiveDate], today: NaiveDate) -> i64 {
    let set: std::collections::BTreeSet<NaiveDate> = days.iter().copied().collect();
    let mut d = if set.contains(&today) {
        today
    } else {
        today - Duration::days(1)
    };
    let mut n = 0;
    while set.contains(&d) {
        n += 1;
        d -= Duration::days(1);
    }
    n
}

/// `local_date` turns a stored UTC timestamp into the user's calendar day.
pub fn compute(
    rows: &[UsageRow],
    today: NaiveDate,
    local_date: &dyn Fn(&str) -> Option<NaiveDate>,
    learned_words: i64,
    your_words: i64,
) -> Insights {
    let mut per_day: BTreeMap<NaiveDate, (i64, i64)> = BTreeMap::new();
    let mut per_app: BTreeMap<String, i64> = BTreeMap::new();
    let (mut total_words, mut dictations, mut commands, mut fixes) = (0i64, 0i64, 0i64, 0i64);
    let (mut speech_words, mut speech_ms) = (0i64, 0i64);
    let week_start = today - Duration::days(6);
    let (mut today_words, mut week_words) = (0i64, 0i64);

    for r in rows {
        if r.level == "COMMAND" {
            commands += 1;
            continue;
        }
        dictations += 1;
        total_words += r.words;
        if r.fixed {
            fixes += 1;
        }
        if r.duration_ms >= 1000 && r.words > 0 {
            speech_words += r.words;
            speech_ms += r.duration_ms;
        }
        *per_app.entry(app_name(&r.app_process)).or_default() += r.words;
        if let Some(day) = local_date(&r.created_at) {
            let e = per_day.entry(day).or_default();
            e.0 += r.words;
            e.1 += 1;
            if day == today {
                today_words += r.words;
            }
            if day >= week_start && day <= today {
                week_words += r.words;
            }
        }
    }

    let active: Vec<NaiveDate> = per_day.keys().copied().collect();
    let first = today - Duration::days(CALENDAR_DAYS - 1);
    let days = (0..CALENDAR_DAYS)
        .map(|i| {
            let d = first + Duration::days(i);
            let (words, n) = per_day.get(&d).copied().unwrap_or((0, 0));
            DayCount {
                date: d.format("%Y-%m-%d").to_string(),
                words,
                dictations: n,
            }
        })
        .collect();

    let mut apps: Vec<AppUse> = per_app
        .into_iter()
        .filter(|(_, w)| *w > 0)
        .map(|(name, words)| AppUse {
            name,
            words,
            share: if total_words > 0 {
                words as f64 / total_words as f64
            } else {
                0.0
            },
        })
        .collect();
    apps.sort_by(|a, b| b.words.cmp(&a.words).then(a.name.cmp(&b.name)));
    apps.truncate(6);

    let speaking_minutes = speech_ms as f64 / 60_000.0;
    let speaking_wpm = (speaking_minutes > 0.0).then(|| speech_words as f64 / speaking_minutes);
    let all_minutes: f64 = rows
        .iter()
        .filter(|r| r.level != "COMMAND")
        .map(|r| r.duration_ms as f64 / 60_000.0)
        .sum();
    let minutes_saved = (total_words as f64 / TYPING_WPM - all_minutes).max(0.0);

    Insights {
        total_words,
        total_dictations: dictations,
        commands,
        words_today: today_words,
        words_this_week: week_words,
        speaking_wpm,
        minutes_saved,
        typing_wpm: TYPING_WPM,
        current_streak: current_run(&active, today),
        longest_streak: longest_run(&active),
        active_days: active.len() as i64,
        days,
        apps,
        fixes,
        learned_words,
        your_words,
        first_day: active.first().map(|d| d.format("%Y-%m-%d").to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(day: &str, words: i64, ms: i64, app: &str) -> UsageRow {
        UsageRow {
            created_at: day.into(),
            words,
            duration_ms: ms,
            app_process: app.into(),
            level: "LIGHT".into(),
            fixed: false,
        }
    }

    fn date(s: &str) -> NaiveDate {
        NaiveDate::parse_from_str(s, "%Y-%m-%d").unwrap()
    }

    fn local(s: &str) -> Option<NaiveDate> {
        NaiveDate::parse_from_str(&s[..10], "%Y-%m-%d").ok()
    }

    #[test]
    fn counts_streaks_speed_and_apps_from_history_only() {
        let rows = vec![
            row("2026-11-10", 30, 12_000, "WINWORD.EXE"),
            row("2026-11-12", 20, 6_000, "chrome.exe"),
            row("2026-11-13", 40, 12_000, "chrome.exe"),
            row("2026-11-14", 10, 3_000, "WhatsApp.exe"),
            UsageRow {
                level: "COMMAND".into(),
                ..row("2026-11-14", 3, 2_000, "WINWORD.EXE")
            },
            UsageRow {
                fixed: true,
                ..row("2026-11-14", 0, 500, "notepad.exe")
            },
        ];
        let i = compute(&rows, date("2026-11-14"), &local, 2, 5);
        assert_eq!(i.total_words, 100);
        assert_eq!(i.total_dictations, 5);
        assert_eq!(i.commands, 1);
        assert_eq!(i.fixes, 1);
        assert_eq!(i.current_streak, 3); // 12, 13, 14
        assert_eq!(i.longest_streak, 3);
        assert_eq!(i.words_today, 10);
        assert_eq!(i.words_this_week, 100);
        // 100 words over 33 s of speech (the 0.5 s, 0-word clip is left out).
        assert!((i.speaking_wpm.unwrap() - 100.0 / (33.0 / 60.0)).abs() < 0.01);
        assert_eq!(i.apps[0].name, "Chrome");
        assert_eq!(i.apps[0].words, 60);
        assert_eq!(i.days.len() as i64, CALENDAR_DAYS);
        assert_eq!(i.days.last().unwrap().date, "2026-11-14");
        assert_eq!(i.first_day.as_deref(), Some("2026-11-10"));
    }

    #[test]
    fn streak_survives_until_the_day_is_over() {
        let rows = vec![
            row("2026-11-12", 5, 2_000, ""),
            row("2026-11-13", 5, 2_000, ""),
        ];
        assert_eq!(
            compute(&rows, date("2026-11-14"), &local, 0, 0).current_streak,
            2
        );
        assert_eq!(
            compute(&rows, date("2026-11-15"), &local, 0, 0).current_streak,
            0
        );
    }

    #[test]
    fn empty_history_gives_zeros_not_made_up_numbers() {
        let i = compute(&[], date("2026-11-14"), &local, 0, 0);
        assert_eq!(i.total_words, 0);
        assert_eq!(i.speaking_wpm, None);
        assert_eq!(i.minutes_saved, 0.0);
        assert!(i.apps.is_empty());
    }

    #[test]
    fn program_names_are_friendly() {
        assert_eq!(app_name("WINWORD.EXE"), "Word");
        assert_eq!(app_name("ms-teams.exe"), "Teams");
        assert_eq!(app_name("tally.exe"), "Tally");
        assert_eq!(app_name(""), "Other");
    }
}

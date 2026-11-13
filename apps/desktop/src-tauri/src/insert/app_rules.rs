//! Per-app behaviour: which key pastes, and what kind of app it is.

use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum PasteKey {
    CtrlV,
    ShiftInsert,
    CtrlShiftV,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Category {
    Email,
    Chat,
    Docs,
    Code,
    AiChat,
    Terminal,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AppRule {
    pub category: Category,
    pub paste_key: PasteKey,
}

/// Built-in rules. `process` is the executable name ("Code.exe"); `title` is the window title.
pub fn rule_for(process: &str, title: &str) -> AppRule {
    let p = process.to_lowercase();
    let p = p.trim_end_matches(".exe");
    let t = title.to_lowercase();
    let rule = |category, paste_key| AppRule {
        category,
        paste_key,
    };

    match p {
        "code" | "code - insiders" | "cursor" | "windsurf" | "vscodium" => {
            rule(Category::Code, PasteKey::ShiftInsert)
        }
        "windowsterminal" | "wt" | "mintty" | "alacritty" | "wezterm-gui" | "wezterm" => {
            rule(Category::Terminal, PasteKey::CtrlShiftV)
        }
        "outlook" | "olk" | "thunderbird" => rule(Category::Email, PasteKey::CtrlV),
        "whatsapp" | "slack" | "teams" | "ms-teams" | "telegram" | "discord" => {
            rule(Category::Chat, PasteKey::CtrlV)
        }
        "winword" | "notepad" | "wordpad" | "excel" | "powerpnt" | "onenote" => {
            rule(Category::Docs, PasteKey::CtrlV)
        }
        "chrome" | "msedge" | "firefox" | "brave" | "opera" | "vivaldi" | "arc" => {
            // Browsers: the tab title is the only hint available without reading the page.
            let category = if t.contains("gmail") || t.contains("outlook") || t.contains(" mail") {
                Category::Email
            } else if t.contains("whatsapp")
                || t.contains("slack")
                || t.contains("telegram")
                || t.contains("messenger")
            {
                Category::Chat
            } else if t.contains("chatgpt")
                || t.contains("claude")
                || t.contains("gemini")
                || t.contains("copilot")
            {
                Category::AiChat
            } else if t.contains("google docs") || t.contains("notion") || t.contains("word") {
                Category::Docs
            } else {
                Category::Other
            };
            rule(category, PasteKey::CtrlV)
        }
        _ => rule(Category::Other, PasteKey::CtrlV),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn editors_use_shift_insert_and_terminals_ctrl_shift_v() {
        assert_eq!(
            rule_for("Code.exe", "main.rs").paste_key,
            PasteKey::ShiftInsert
        );
        assert_eq!(rule_for("Cursor.exe", "").paste_key, PasteKey::ShiftInsert);
        assert_eq!(
            rule_for("WindowsTerminal.exe", "PowerShell").paste_key,
            PasteKey::CtrlShiftV
        );
        assert_eq!(
            rule_for("notepad.exe", "Untitled").paste_key,
            PasteKey::CtrlV
        );
    }

    #[test]
    fn browser_tabs_are_categorised_by_title() {
        assert_eq!(
            rule_for(
                "chrome.exe",
                "Inbox (3) - someone@gmail.com - Gmail - Google Chrome"
            )
            .category,
            Category::Email
        );
        assert_eq!(
            rule_for("chrome.exe", "WhatsApp - Google Chrome").category,
            Category::Chat
        );
        assert_eq!(
            rule_for("msedge.exe", "Claude - Microsoft Edge").category,
            Category::AiChat
        );
        assert_eq!(rule_for("chrome.exe", "News").category, Category::Other);
    }

    #[test]
    fn unknown_apps_get_safe_defaults() {
        assert_eq!(
            rule_for("SomeApp.exe", "x"),
            AppRule {
                category: Category::Other,
                paste_key: PasteKey::CtrlV
            }
        );
    }
}

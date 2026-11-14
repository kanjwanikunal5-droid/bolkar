//! "Is there a newer version?" Asks GitHub's public releases list for this repository.
//!
//! Only when the user presses "Check for updates", or once a day if they turned automatic
//! checks on (off by default). The request carries no personal data: a generic user agent
//! and the public repository address. Nothing is downloaded or installed automatically; the
//! user gets a link to the release page.

use std::time::Duration;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub current: String,
    /// Latest published version, without a leading "v". None when nothing is published yet.
    pub latest: Option<String>,
    pub newer: bool,
    /// Release page to open (only ever a github.com address of this repository).
    pub url: Option<String>,
    pub message: String,
}

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
}

/// "v1.2.10" -> [1, 2, 10]. Anything that is not a plain version gives None.
pub fn parse_version(v: &str) -> Option<Vec<u64>> {
    let core = v.trim().trim_start_matches(['v', 'V']);
    let core = core.split(['-', '+']).next()?;
    let parts: Option<Vec<u64>> = core.split('.').map(|p| p.parse().ok()).collect();
    parts.filter(|p| !p.is_empty() && p.len() <= 4)
}

pub fn is_newer(latest: &str, current: &str) -> bool {
    match (parse_version(latest), parse_version(current)) {
        (Some(mut a), Some(mut b)) => {
            let n = a.len().max(b.len());
            a.resize(n, 0);
            b.resize(n, 0);
            a > b
        }
        _ => false,
    }
}

pub fn check(repository: &str, current: &str) -> UpdateInfo {
    let mut info = UpdateInfo {
        current: current.to_string(),
        latest: None,
        newer: false,
        url: None,
        message: String::new(),
    };
    if repository.is_empty() {
        info.message = "No release page is configured for this build.".into();
        return info;
    }
    let url = format!("https://api.github.com/repos/{repository}/releases/latest");
    let client = match reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("desktop-update-check")
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            info.message = format!("Could not check: {e}");
            return info;
        }
    };
    let response = match client
        .get(url)
        .header("Accept", "application/vnd.github+json")
        .send()
    {
        Ok(r) => r,
        Err(e) => {
            info.message = format!("Could not reach GitHub: {e}");
            return info;
        }
    };
    if response.status().as_u16() == 404 {
        info.message = "No version has been published yet. You have the latest build.".into();
        return info;
    }
    if !response.status().is_success() {
        info.message = format!("GitHub answered {}. Try again later.", response.status());
        return info;
    }
    let parsed = response
        .text()
        .map_err(|e| e.to_string())
        .and_then(|t| serde_json::from_str::<Release>(&t).map_err(|e| e.to_string()));
    match parsed {
        Ok(r) => {
            let expected = format!("https://github.com/{repository}/releases/");
            let latest = r.tag_name.trim_start_matches(['v', 'V']).to_string();
            info.newer = is_newer(&latest, current);
            info.url = r.html_url.starts_with(&expected).then_some(r.html_url);
            info.message = if info.newer {
                format!("Version {latest} is available (you have {current}).")
            } else {
                format!("You have the latest version ({current}).")
            };
            info.latest = Some(latest);
        }
        Err(e) => info.message = format!("Unexpected answer from GitHub: {e}"),
    }
    info
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_by_number_not_text() {
        assert!(is_newer("v0.1.10", "0.1.9"));
        assert!(is_newer("1.0.0", "0.9.9"));
        assert!(!is_newer("v0.1.0", "0.1.0"));
        assert!(!is_newer("0.1", "0.1.0"));
        assert!(!is_newer("0.0.9", "0.1.0"));
        assert!(!is_newer("nightly", "0.1.0"));
        assert_eq!(parse_version("v2.3.4-beta.1"), Some(vec![2, 3, 4]));
    }

    /// Talks to GitHub: run by hand with `cargo test -p app-desktop --lib -- --ignored`.
    #[test]
    #[ignore]
    fn live_check_against_github() {
        let info = check(app_core::brand::REPOSITORY, "0.0.1");
        println!("{info:?}");
        assert!(!info.message.is_empty());
        assert!(info.message.starts_with("No version") || info.latest.is_some());
    }
}

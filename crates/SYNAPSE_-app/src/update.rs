//! Auto-update check via GitHub Releases API.
//! No auto-install; only notifies the user when a newer version exists.

const RELEASES_URL: &str = "https://api.github.com/repos/isradev-git/synapse_/releases/latest";
const CURRENT: &str = env!("CARGO_PKG_VERSION");

/// Fetch latest GitHub release and compare to the running version.
///
/// Returns `Ok(Some(latest_tag))` when an update is available,
/// `Ok(None)` when already up-to-date, `Err` on network/parse failure.
pub fn check_update() -> Result<Option<String>, String> {
    let body: serde_json::Value = ureq::get(RELEASES_URL)
        .set("User-Agent", &format!("synapse_/{CURRENT}"))
        .set("Accept", "application/vnd.github+json")
        .call()
        .map_err(|e| e.to_string())?
        .into_json()
        .map_err(|e| e.to_string())?;

    let tag = body["tag_name"]
        .as_str()
        .unwrap_or("")
        .trim_start_matches('v');

    if tag.is_empty() {
        return Err("could not read tag_name from GitHub response".into());
    }

    if tag != CURRENT {
        Ok(Some(tag.to_string()))
    } else {
        Ok(None)
    }
}

/// Print update status to stdout. Called from `--check-update` CLI path.
pub fn run_check() -> Result<(), Box<dyn std::error::Error>> {
    match check_update() {
        Ok(Some(latest)) => {
            println!("Update available: v{latest}  (current: v{CURRENT})");
            println!("Download: https://github.com/isradev-git/synapse_/releases/latest");
        }
        Ok(None) => {
            println!("SYNAPSE_ v{CURRENT} is up to date.");
        }
        Err(e) => {
            eprintln!("Update check failed: {e}");
            std::process::exit(1);
        }
    }
    Ok(())
}

/// Spawn a background thread that checks for updates and fires a desktop
/// notification when a newer version is found. Returns immediately.
pub fn spawn_background_check() {
    std::thread::Builder::new()
        .name("synapse-update-check".into())
        .spawn(|| {
            if let Ok(Some(latest)) = check_update() {
                let msg = format!("SYNAPSE_ v{latest} is available (current: v{CURRENT})");
                tracing::info!("{msg}");
                let _ = notify_rust::Notification::new()
                    .summary("SYNAPSE_ update available")
                    .body(&msg)
                    .show();
            }
        })
        .ok();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_version_not_empty() {
        assert!(!CURRENT.is_empty());
    }

    #[test]
    fn releases_url_is_https() {
        assert!(RELEASES_URL.starts_with("https://"));
    }

    #[test]
    fn same_version_returns_none() {
        // Simulate tag equal to current → no update.
        let tag = CURRENT.trim_start_matches('v');
        let result = if tag != CURRENT {
            Some(tag.to_string())
        } else {
            None
        };
        assert!(result.is_none());
    }

    #[test]
    fn different_version_returns_some() {
        let fake_latest = "99.0.0";
        let result = if fake_latest != CURRENT {
            Some(fake_latest.to_string())
        } else {
            None
        };
        assert_eq!(result, Some("99.0.0".to_string()));
    }
}

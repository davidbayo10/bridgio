use std::fs;
use std::path::PathBuf;

/// Returns `~/.local/share/bridgio/state` (or equivalent on the current OS).
fn state_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("bridgio").join("state"))
}

/// Persists the selected profile name and region string.
/// Silently ignores errors (best-effort).
pub fn save_state(profile: &str, region: &str) {
    let Some(path) = state_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(path, format!("{profile}\n{region}\n"));
}

/// Returns `(profile, region)` loaded from disk, or `None` if the file does
/// not exist or cannot be parsed.
pub fn load_state() -> Option<(String, String)> {
    let path = state_path()?;
    let contents = fs::read_to_string(path).ok()?;
    let mut lines = contents.lines();
    let profile = lines.next()?.trim().to_string();
    let region = lines.next()?.trim().to_string();
    if profile.is_empty() || region.is_empty() {
        return None;
    }
    Some((profile, region))
}

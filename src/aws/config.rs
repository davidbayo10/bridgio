use anyhow::Result;
use aws_config::{BehaviorVersion, SdkConfig};
use aws_types::region::Region;
use std::fs;
use std::path::PathBuf;

/// Returns the path to `~/.aws/config`.
fn config_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".aws").join("config"))
}

/// Returns the path to `~/.aws/credentials`.
fn credentials_path() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".aws").join("credentials"))
}

/// Parses `[profile name]` and `[name]` section headers from an AWS config file.
fn parse_profile_names(content: &str) -> Vec<String> {
    content
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if line.starts_with('[') && line.ends_with(']') {
                let inner = &line[1..line.len() - 1];
                // ~/.aws/config uses "[profile name]", credentials uses "[name]"
                let name = if let Some(stripped) = inner.strip_prefix("profile ") {
                    stripped
                } else {
                    inner
                };
                if !name.is_empty() {
                    return Some(name.to_string());
                }
            }
            None
        })
        .collect()
}

/// Returns a deduplicated, sorted list of all profiles found in
/// `~/.aws/config` and `~/.aws/credentials`.
pub fn list_profiles() -> Vec<String> {
    let mut profiles: Vec<String> = Vec::new();

    for path in [config_path(), credentials_path()].into_iter().flatten() {
        if let Ok(content) = fs::read_to_string(&path) {
            for name in parse_profile_names(&content) {
                if !profiles.contains(&name) {
                    profiles.push(name);
                }
            }
        }
    }

    // Always ensure "default" is present and first.
    if let Some(pos) = profiles.iter().position(|p| p == "default") {
        profiles.remove(pos);
    }
    profiles.insert(0, "default".to_string());

    profiles
}

/// Builds an `SdkConfig` for the given profile and region.
pub async fn load_sdk_config(profile: &str, region: &str) -> Result<SdkConfig> {
    let config = aws_config::defaults(BehaviorVersion::latest())
        .profile_name(profile)
        .region(Region::new(region.to_string()))
        .load()
        .await;
    Ok(config)
}

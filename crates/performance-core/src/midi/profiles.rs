// SPDX-License-Identifier: GPL-3.0-or-later
//! Named profile storage. Only validated names resolve beneath the configured directory.
use super::profile::Profile;
use std::{
    io::Read,
    path::{Path, PathBuf},
};
pub fn valid_name(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    let reserved = matches!(lower.as_str(), "con" | "prn" | "aux" | "nul")
        || (lower.len() == 4
            && (lower.starts_with("com") || lower.starts_with("lpt"))
            && matches!(lower.as_bytes()[3], b'1'..=b'9'));
    !name.is_empty()
        && !reserved
        && name.len() <= 48
        && name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || b"-_".contains(&c))
}
pub fn path(default: &Path, name: &str) -> Result<PathBuf, String> {
    if !valid_name(name) {
        return Err(
            "Profile name: 1-48 ASCII letters, digits, - or _; reserved OS names are unavailable"
                .into(),
        );
    }
    if name == "default" {
        return Ok(default.into());
    }
    Ok(default
        .parent()
        .ok_or("No profile directory")?
        .join("profiles")
        .join(format!("{name}.json")))
}
pub fn list(default: &Path) -> Result<Vec<String>, String> {
    let directory = default
        .parent()
        .ok_or("No profile directory")?
        .join("profiles");
    let mut names = vec!["default".into()];
    if directory.exists() {
        for entry in std::fs::read_dir(directory)
            .map_err(|e| e.to_string())?
            .take(128)
        {
            let entry = entry.map_err(|e| e.to_string())?;
            let p = entry.path();
            if p.extension().is_some_and(|e| e == "json")
                && let Some(name) = p.file_stem().and_then(|s| s.to_str())
                && valid_name(name)
                && name != "default"
            {
                names.push(name.into());
            }
        }
    }
    names.sort();
    names.dedup();
    Ok(names)
}
pub fn remember(default: &Path, name: &str) -> Result<(), String> {
    path(default, name)?;
    let parent = default.parent().ok_or("No profile directory")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let temp = parent.join("active-profile.tmp");
    std::fs::write(&temp, name).map_err(|e| e.to_string())?;
    std::fs::rename(temp, parent.join("active-profile.txt")).map_err(|e| e.to_string())
}
pub fn selected(default: &Path) -> String {
    let read = || -> Option<String> {
        let mut name = String::new();
        std::fs::File::open(default.parent()?.join("active-profile.txt"))
            .ok()?
            .take(50)
            .read_to_string(&mut name)
            .ok()?;
        valid_name(&name).then_some(name)
    };
    read().unwrap_or_else(|| "default".into())
}
pub fn save(default: &Path, name: &str, profile: &Profile) -> Result<(), String> {
    profile.save(&path(default, name)?)?;
    remember(default, name)
}

//! Persistent user settings, stored as plain JSON. No database, no account.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::util;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GroupMeta {
    pub name: String,
    pub emoji: String,
    pub color: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct ProjectMeta {
    pub pinned: bool,
    /// Editor id override for this project.
    pub editor: Option<String>,
    pub last_opened: Option<i64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CustomEditor {
    pub id: String,
    pub name: String,
    /// Shell command; `{path}` is replaced by the quoted project path.
    pub command: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub version: u32,
    pub onboarded: bool,
    /// Absolute path to the folder that holds group folders (default ~/Code).
    pub projects_root: String,
    /// Extra folders (outside the root) to scan for existing projects.
    pub scan_paths: Vec<String>,
    pub default_editor: String,
    pub groups: Vec<GroupMeta>,
    /// Per-project metadata keyed by absolute path.
    pub projects: BTreeMap<String, ProjectMeta>,
    pub custom_editors: Vec<CustomEditor>,
    /// Private GitHub repo used to park public / repo-less projects ("owner/name").
    pub vault_repo: Option<String>,
    /// Projects untouched for this many days are suggested for parking (0 = off).
    pub stale_days: u32,
    /// Defaults remembered by the New Project wizard.
    pub new_project_defaults: NewProjectDefaults,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct NewProjectDefaults {
    pub git_init: bool,
    /// "none" | "private" | "public"
    pub github: String,
    pub open_after: bool,
    pub template: String,
}

impl Default for NewProjectDefaults {
    fn default() -> Self {
        Self {
            git_init: true,
            github: "private".into(),
            open_after: true,
            template: "blank".into(),
        }
    }
}

pub fn starter_groups() -> Vec<GroupMeta> {
    [
        ("Startups", "🚀", "#ff9600"),
        ("Hackathons", "⚡", "#ffc800"),
        ("Experiments", "🧪", "#ce82ff"),
        ("School", "🎓", "#58cc02"),
        ("Websites", "🌐", "#1c9af0"),
        ("Archive", "📦", "#afafaf"),
    ]
    .into_iter()
    .map(|(n, e, c)| GroupMeta {
        name: n.into(),
        emoji: e.into(),
        color: c.into(),
    })
    .collect()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: 1,
            onboarded: false,
            projects_root: util::home().join("Code").to_string_lossy().into_owned(),
            scan_paths: vec![],
            default_editor: "vscode".into(),
            groups: starter_groups(),
            projects: BTreeMap::new(),
            custom_editors: vec![],
            vault_repo: None,
            stale_days: 60,
            new_project_defaults: NewProjectDefaults::default(),
        }
    }
}

impl Config {
    pub fn root(&self) -> PathBuf {
        util::expand_tilde(&self.projects_root)
    }

    #[allow(dead_code)]
    pub fn group_meta(&self, name: &str) -> Option<&GroupMeta> {
        self.groups.iter().find(|g| g.name == name)
    }

    pub fn meta_mut(&mut self, path: &str) -> &mut ProjectMeta {
        self.projects.entry(path.to_string()).or_default()
    }

    /// Move per-project metadata when a project's path changes.
    #[allow(dead_code)]
    pub fn rename_project_key(&mut self, from: &str, to: &str) {
        if let Some(meta) = self.projects.remove(from) {
            self.projects.insert(to.to_string(), meta);
        }
    }
}

pub fn config_path(data_dir: &Path) -> PathBuf {
    data_dir.join("config.json")
}

pub fn load(data_dir: &Path) -> Config {
    let path = config_path(data_dir);
    match std::fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_else(|e| {
            // Keep a copy of the unreadable file rather than silently losing it.
            let _ = std::fs::copy(&path, data_dir.join("config.broken.json"));
            eprintln!("pelago: config.json unreadable ({e}); using defaults");
            Config::default()
        }),
        Err(_) => Config::default(),
    }
}

/// Write JSON atomically (write temp file, then rename).
pub fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

pub fn save(data_dir: &Path, config: &Config) -> Result<(), String> {
    write_json(&config_path(data_dir), config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_partial_json() {
        let dir = tempfile::tempdir().unwrap();
        let mut c = Config {
            onboarded: true,
            ..Config::default()
        };
        c.meta_mut("/x").pinned = true;
        save(dir.path(), &c).unwrap();
        assert_eq!(load(dir.path()), c);

        // Older / partial files still load with defaults filled in.
        std::fs::write(config_path(dir.path()), r#"{"onboarded":true}"#).unwrap();
        let loaded = load(dir.path());
        assert!(loaded.onboarded);
        assert_eq!(loaded.groups.len(), 6);
    }

    #[test]
    fn broken_file_falls_back() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(config_path(dir.path()), "not json").unwrap();
        assert!(!load(dir.path()).onboarded);
        assert!(dir.path().join("config.broken.json").exists());
    }
}

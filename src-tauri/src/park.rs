//! Park & resume. Phase 1 only needs the registry so parked projects show up in the list.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::commands::AppState;
use crate::config;
use crate::scan::{self, Project, ProjectList};
use crate::util;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ParkedProject {
    pub name: String,
    /// Absolute path the project lived at (and will be restored to).
    pub path: String,
    pub group: Option<String>,
    /// "branch" (pelago/wip in the project's own private repo) | "vault"
    pub mode: String,
    /// Repo holding the snapshot ("owner/name").
    pub snapshot_repo: String,
    /// Ref inside `snapshot_repo` that holds the snapshot commit.
    pub snapshot_ref: String,
    pub snapshot_commit: String,
    /// The project's own origin remote, if it had one.
    pub origin_url: Option<String>,
    pub branch: Option<String>,
    pub head_commit: Option<String>,
    pub stack: Vec<String>,
    pub size_bytes: u64,
    pub parked_at: i64,
    pub last_activity: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct Registry {
    pub version: u32,
    pub parked: Vec<ParkedProject>,
}

pub fn registry_path(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("registry.json")
}

pub fn load_registry(data_dir: &Path) -> Registry {
    std::fs::read_to_string(registry_path(data_dir))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or(Registry {
            version: 1,
            parked: vec![],
        })
}

pub fn save_registry(data_dir: &Path, reg: &Registry) -> Result<(), String> {
    config::write_json(&registry_path(data_dir), reg)
}

/// Add parked projects (not present on disk) to a scan result.
pub fn merge_parked(state: &AppState, mut list: ProjectList) -> ProjectList {
    let reg = load_registry(&state.data_dir);
    let cfg = state.config();
    for p in reg.parked {
        if Path::new(&p.path).exists() {
            continue; // restored outside Pelago; the scan already shows it
        }
        let meta = cfg.projects.get(&p.path).cloned().unwrap_or_default();
        list.projects.push(Project {
            id: p.path.clone(),
            name: p.name.clone(),
            display_path: util::display_path(Path::new(&p.path)),
            path: p.path.clone(),
            group: p.group.clone(),
            location: "root".into(),
            status: "parked".into(),
            git: None,
            stack: p.stack.clone(),
            empty: false,
            nested_repos: vec![],
            duplicates: vec![],
            last_activity: p.last_activity,
            pinned: meta.pinned,
            editor: meta.editor,
            last_opened: meta.last_opened,
            suggested_group: None,
            size_bytes: Some(p.size_bytes),
            parked_at: Some(p.parked_at),
        });
    }
    list.projects.sort_by_key(|a| a.name.to_lowercase());
    let root = cfg.root();
    list.groups = scan::build_groups(&cfg, &list.projects, &root);
    list
}

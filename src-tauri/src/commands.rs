//! Tauri command surface. Heavy work runs on a blocking thread so the UI never freezes.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::State;

use crate::config::{self, Config, CustomEditor, GroupMeta};
use crate::editors::{self, Editor};
use crate::github::{self, GhStatus};
use crate::scan::{self, ProjectList};
use crate::util;

pub struct AppState {
    pub data_dir: PathBuf,
    pub config: Mutex<Config>,
    pub editors: Mutex<Option<Vec<Editor>>>,
}

pub type Shared = Arc<AppState>;

impl AppState {
    pub fn new(data_dir: PathBuf) -> Self {
        let config = config::load(&data_dir);
        Self {
            data_dir,
            config: Mutex::new(config),
            editors: Mutex::new(None),
        }
    }

    pub fn config(&self) -> Config {
        self.config.lock().unwrap().clone()
    }

    /// Mutate config and persist it atomically.
    pub fn update<R>(&self, f: impl FnOnce(&mut Config) -> R) -> Result<R, String> {
        let mut guard = self.config.lock().unwrap();
        let mut next = guard.clone();
        let out = f(&mut next);
        config::save(&self.data_dir, &next)?;
        *guard = next;
        Ok(out)
    }

    pub fn editors(&self) -> Vec<Editor> {
        let mut cache = self.editors.lock().unwrap();
        if cache.is_none() {
            let custom = self.config().custom_editors;
            *cache = Some(editors::detect(&custom));
        }
        cache.clone().unwrap()
    }

    pub fn invalidate_editors(&self) {
        *self.editors.lock().unwrap() = None;
    }

    pub fn find_editor(&self, id: &str) -> Result<Editor, String> {
        self.editors()
            .into_iter()
            .find(|e| e.id == id)
            .ok_or_else(|| format!("Unknown editor `{id}`"))
    }
}

/// Run blocking work off the main thread.
pub async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f)
        .await
        .map_err(|e| e.to_string())?
}

// ---------- config / onboarding ----------

#[tauri::command]
pub fn get_config(state: State<'_, Shared>) -> Config {
    state.config()
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub projects_root: Option<String>,
    pub scan_paths: Option<Vec<String>>,
    pub default_editor: Option<String>,
    pub stale_days: Option<u32>,
    pub vault_repo: Option<Option<String>>,
    pub custom_editors: Option<Vec<CustomEditor>>,
    pub new_project_defaults: Option<config::NewProjectDefaults>,
    pub groups: Option<Vec<GroupMeta>>,
}

#[tauri::command]
pub fn save_settings(state: State<'_, Shared>, patch: SettingsPatch) -> Result<Config, String> {
    let custom_changed = patch.custom_editors.is_some();
    let cfg = state.update(|c| {
        if let Some(v) = patch.projects_root {
            c.projects_root = util::expand_tilde(&v).to_string_lossy().into_owned();
        }
        if let Some(v) = patch.scan_paths {
            c.scan_paths = v;
        }
        if let Some(v) = patch.default_editor {
            c.default_editor = v;
        }
        if let Some(v) = patch.stale_days {
            c.stale_days = v;
        }
        if let Some(v) = patch.vault_repo {
            c.vault_repo = v.filter(|s| !s.trim().is_empty());
        }
        if let Some(v) = patch.custom_editors {
            c.custom_editors = v;
        }
        if let Some(v) = patch.new_project_defaults {
            c.new_project_defaults = v;
        }
        if let Some(v) = patch.groups {
            c.groups = v;
        }
        c.clone()
    })?;
    if custom_changed {
        state.invalidate_editors();
    }
    Ok(cfg)
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Onboarding {
    pub projects_root: String,
    pub scan_paths: Vec<String>,
    pub default_editor: String,
    pub groups: Vec<GroupMeta>,
}

#[tauri::command]
pub fn complete_onboarding(state: State<'_, Shared>, input: Onboarding) -> Result<Config, String> {
    let root = util::expand_tilde(&input.projects_root);
    std::fs::create_dir_all(&root)
        .map_err(|e| format!("Couldn't create {}: {e}", root.display()))?;
    for g in &input.groups {
        let name = util::validate_name(&g.name)?;
        std::fs::create_dir_all(root.join(name)).map_err(|e| e.to_string())?;
    }
    state.update(|c| {
        c.onboarded = true;
        c.projects_root = root.to_string_lossy().into_owned();
        c.scan_paths = input.scan_paths;
        c.default_editor = input.default_editor;
        c.groups = input.groups;
        c.clone()
    })
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct FolderSuggestion {
    pub path: String,
    pub display_path: String,
    pub project_count: usize,
    pub examples: Vec<String>,
}

const HOME_SKIP: &[&str] = &[
    "Library",
    "Applications",
    "Movies",
    "Music",
    "Pictures",
    "Public",
    "Google Drive",
    "OneDrive",
    "Dropbox",
    "iCloud Drive (Archive)",
    "anaconda3",
    "miniconda3",
    "opt",
    "go",
];

/// Folders in the home directory that look like they hold several projects.
#[tauri::command]
pub async fn suggest_scan_folders() -> Result<Vec<FolderSuggestion>, String> {
    blocking(|| {
        let home = util::home();
        let mut out = vec![];
        for e in std::fs::read_dir(&home)
            .map_err(|e| e.to_string())?
            .filter_map(|e| e.ok())
        {
            let name = e.file_name().to_string_lossy().into_owned();
            let path = e.path();
            if !path.is_dir() || name.starts_with('.') || HOME_SKIP.contains(&name.as_str()) {
                continue;
            }
            if scan::looks_like_project(&path) && crate::git::is_repo_root(&path) {
                continue; // a single repo, not a folder of projects
            }
            let Ok(rd) = std::fs::read_dir(&path) else {
                continue;
            };
            let mut examples = vec![];
            let mut count = 0;
            for c in rd.filter_map(|c| c.ok()) {
                let cp = c.path();
                let cn = c.file_name().to_string_lossy().into_owned();
                if cp.is_dir()
                    && !cn.starts_with('.')
                    && cn != "node_modules"
                    && scan::looks_like_project(&cp)
                {
                    count += 1;
                    if examples.len() < 4 {
                        examples.push(cn);
                    }
                }
            }
            if count >= 2 {
                out.push(FolderSuggestion {
                    display_path: util::display_path(&path),
                    path: path.to_string_lossy().into_owned(),
                    project_count: count,
                    examples,
                });
            }
        }
        out.sort_by_key(|s| std::cmp::Reverse(s.project_count));
        Ok(out)
    })
    .await
}

// ---------- projects ----------

#[tauri::command]
pub async fn list_projects(state: State<'_, Shared>) -> Result<ProjectList, String> {
    let st = state.inner().clone();
    blocking(move || crate::park::merge_parked(&st, scan::scan(&st.config()))).await
}

#[tauri::command]
pub async fn project_size(path: String) -> Result<u64, String> {
    blocking(move || Ok(util::dir_size(Path::new(&path)))).await
}

#[tauri::command]
pub async fn open_project(
    state: State<'_, Shared>,
    path: String,
    editor: Option<String>,
) -> Result<(), String> {
    let st = state.inner().clone();
    blocking(move || {
        let cfg = st.config();
        let editor_id = editor
            .or_else(|| cfg.projects.get(&path).and_then(|m| m.editor.clone()))
            .unwrap_or(cfg.default_editor.clone());
        let ed = st.find_editor(&editor_id)?;
        editors::open(&ed, &cfg.custom_editors, Path::new(&path))?;
        st.update(|c| c.meta_mut(&path).last_opened = Some(util::now_secs()))?;
        Ok(())
    })
    .await
}

#[tauri::command]
pub fn reveal_in_finder(path: String) -> Result<(), String> {
    util::run("open", &["-R", &path], None).map(|_| ())
}

#[tauri::command]
pub fn open_url(url: String) -> Result<(), String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("Only web links can be opened".into());
    }
    util::run("open", &[&url], None).map(|_| ())
}

#[tauri::command]
pub fn set_pinned(state: State<'_, Shared>, path: String, pinned: bool) -> Result<(), String> {
    state.update(|c| c.meta_mut(&path).pinned = pinned)
}

#[tauri::command]
pub fn set_project_editor(
    state: State<'_, Shared>,
    path: String,
    editor: Option<String>,
) -> Result<(), String> {
    state.update(|c| c.meta_mut(&path).editor = editor)
}

// ---------- editors / github ----------

#[tauri::command]
pub async fn list_editors(
    state: State<'_, Shared>,
    refresh: Option<bool>,
) -> Result<Vec<Editor>, String> {
    let st = state.inner().clone();
    blocking(move || {
        if refresh.unwrap_or(false) {
            st.invalidate_editors();
        }
        Ok(st.editors())
    })
    .await
}

#[tauri::command]
pub async fn gh_status() -> Result<GhStatus, String> {
    blocking(|| Ok(github::status())).await
}

#[tauri::command]
pub fn gh_login() -> Result<(), String> {
    github::open_login_terminal()
}

// ---------- create / groups / moves (Phase 2) ----------

use crate::manage::{self, CreateRequest, CreateResult, MovePreview, MoveRecord};

#[tauri::command]
pub async fn create_project(
    state: State<'_, Shared>,
    req: CreateRequest,
    open_after: bool,
    editor: Option<String>,
) -> Result<CreateResult, String> {
    let st = state.inner().clone();
    blocking(move || {
        let cfg = st.config();
        let mut res = manage::create(&cfg, &req)?;
        // Remember the toggles as next time's defaults.
        st.update(|c| {
            c.new_project_defaults = config::NewProjectDefaults {
                git_init: req.git_init,
                github: req.github.clone(),
                open_after,
                template: req.template.clone(),
            };
            if let Some(ed) = editor.clone() {
                c.meta_mut(&res.path).editor = Some(ed);
            }
        })?;
        if open_after {
            let editor_id = editor.unwrap_or(cfg.default_editor.clone());
            match st.find_editor(&editor_id) {
                Ok(ed) => {
                    if let Err(e) = editors::open(&ed, &cfg.custom_editors, Path::new(&res.path)) {
                        res.warnings.push(e);
                    } else {
                        let p = res.path.clone();
                        st.update(|c| c.meta_mut(&p).last_opened = Some(util::now_secs()))?;
                    }
                }
                Err(e) => res.warnings.push(e),
            }
        }
        Ok(res)
    })
    .await
}

#[tauri::command]
pub async fn publish_project(
    path: String,
    private: bool,
    repo_name: Option<String>,
) -> Result<String, String> {
    blocking(move || manage::publish(Path::new(&path), private, repo_name.as_deref())).await
}

#[tauri::command]
pub fn create_group(state: State<'_, Shared>, group: GroupMeta) -> Result<Config, String> {
    let mut cfg = state.config();
    manage::create_group(&mut cfg, group)?;
    state.update(|c| {
        *c = cfg;
        c.clone()
    })
}

#[tauri::command]
pub fn update_group(
    state: State<'_, Shared>,
    old_name: String,
    group: GroupMeta,
) -> Result<Config, String> {
    let mut cfg = state.config();
    manage::update_group(&mut cfg, &old_name, group)?;
    state.update(|c| {
        *c = cfg;
        c.clone()
    })
}

#[tauri::command]
pub fn delete_group(state: State<'_, Shared>, name: String) -> Result<Config, String> {
    let mut cfg = state.config();
    manage::delete_group(&mut cfg, &name)?;
    state.update(|c| {
        *c = cfg;
        c.clone()
    })
}

#[tauri::command]
pub async fn preview_move(
    state: State<'_, Shared>,
    path: String,
    group: Option<String>,
    new_name: Option<String>,
) -> Result<MovePreview, String> {
    let st = state.inner().clone();
    blocking(move || {
        Ok(manage::preview_move(
            &st.config(),
            Path::new(&path),
            group.as_deref(),
            new_name.as_deref(),
        ))
    })
    .await
}

#[tauri::command]
pub async fn move_project(
    state: State<'_, Shared>,
    path: String,
    group: Option<String>,
    new_name: Option<String>,
) -> Result<MoveRecord, String> {
    let st = state.inner().clone();
    blocking(move || {
        let mut cfg = st.config();
        let rec = manage::do_move(
            &mut cfg,
            &st.data_dir,
            Path::new(&path),
            group.as_deref(),
            new_name.as_deref(),
        )?;
        st.update(|c| *c = cfg)?;
        Ok(rec)
    })
    .await
}

#[tauri::command]
pub async fn undo_move(state: State<'_, Shared>, id: String) -> Result<MoveRecord, String> {
    let st = state.inner().clone();
    blocking(move || {
        let mut cfg = st.config();
        let rec = manage::undo_move(&mut cfg, &st.data_dir, &id)?;
        st.update(|c| *c = cfg)?;
        Ok(rec)
    })
    .await
}

#[tauri::command]
pub fn list_moves(state: State<'_, Shared>) -> Vec<MoveRecord> {
    let mut moves = manage::load_log(&state.data_dir).moves;
    moves.reverse();
    moves.truncate(30);
    moves
}

#[tauri::command]
pub async fn remove_empty_repo(path: String) -> Result<(), String> {
    blocking(move || manage::remove_empty_repo(Path::new(&path))).await
}

#[tauri::command]
pub async fn preview_park(
    state: State<'_, Shared>,
    path: String,
) -> Result<crate::park::ParkPreview, String> {
    let st = state.inner().clone();
    blocking(move || crate::park::preview_project(&st, Path::new(&path))).await
}

#[tauri::command]
pub async fn park_project(
    state: State<'_, Shared>,
    path: String,
    passphrase: String,
) -> Result<crate::park::ParkedProject, String> {
    let st = state.inner().clone();
    let secret = age::secrecy::SecretString::from(passphrase);
    blocking(move || {
        use age::secrecy::ExposeSecret;
        crate::park::park(&st, Path::new(&path), secret.expose_secret())
    })
    .await
}

#[tauri::command]
pub async fn restore_project(
    state: State<'_, Shared>,
    path: String,
    passphrase: String,
) -> Result<(), String> {
    let st = state.inner().clone();
    let secret = age::secrecy::SecretString::from(passphrase);
    blocking(move || {
        use age::secrecy::ExposeSecret;
        crate::park::restore(&st, Path::new(&path), secret.expose_secret())
    })
    .await
}

#[cfg(test)]
mod state_tests {
    use super::*;

    #[test]
    fn failed_config_save_keeps_live_config_unchanged() {
        let temp = tempfile::tempdir().unwrap();
        let data = temp.path().join("not-a-directory");
        std::fs::write(&data, "occupied").unwrap();
        let state = AppState::new(data);
        let before = state.config();
        assert!(state.update(|c| c.stale_days = 999).is_err());
        assert_eq!(state.config(), before);
    }

    #[test]
    fn successful_config_save_updates_live_and_persisted_state() {
        let temp = tempfile::tempdir().unwrap();
        let state = AppState::new(temp.path().to_path_buf());
        state.update(|c| c.stale_days = 42).unwrap();
        assert_eq!(state.config().stale_days, 42);
        assert_eq!(config::load(temp.path()).stale_days, 42);
    }
}

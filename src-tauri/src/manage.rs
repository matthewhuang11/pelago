//! Creating projects, managing group folders, and moving projects safely.
//!
//! Every move is recorded in `moves.json` so it can be undone.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::config::{self, Config, GroupMeta};
use crate::git;
use crate::github;
use crate::util;

// ---------- templates ----------

pub struct Template {
    pub id: &'static str,
    pub files: &'static [(&'static str, &'static str)],
    pub gitignore: &'static str,
}

const GI_BASE: &str = ".DS_Store\n.env\n.env.*\n!.env.example\n";
const GI_NODE: &str = ".DS_Store\n.env\n.env.*\n!.env.example\nnode_modules/\ndist/\n*.log\n";
const GI_PY: &str = ".DS_Store\n.env\n.env.*\n!.env.example\n__pycache__/\n*.pyc\n.venv/\nvenv/\n";

pub const TEMPLATES: &[Template] = &[
    Template {
        id: "blank",
        files: &[("README.md", "# {name}\n")],
        gitignore: GI_BASE,
    },
    Template {
        id: "node-ts",
        files: &[
            (
                "README.md",
                "# {name}\n\n```bash\nnpm install\nnpm run dev\n```\n",
            ),
            (
                "package.json",
                "{\n  \"name\": \"{slug}\",\n  \"version\": \"0.1.0\",\n  \"private\": true,\n  \"type\": \"module\",\n  \"scripts\": {\n    \"dev\": \"tsx watch src/index.ts\",\n    \"build\": \"tsc\",\n    \"start\": \"node dist/index.js\"\n  },\n  \"devDependencies\": {\n    \"tsx\": \"^4\",\n    \"typescript\": \"^5\",\n    \"@types/node\": \"^22\"\n  }\n}\n",
            ),
            (
                "tsconfig.json",
                "{\n  \"compilerOptions\": {\n    \"target\": \"ES2022\",\n    \"module\": \"NodeNext\",\n    \"moduleResolution\": \"NodeNext\",\n    \"outDir\": \"dist\",\n    \"strict\": true,\n    \"skipLibCheck\": true\n  },\n  \"include\": [\"src\"]\n}\n",
            ),
            ("src/index.ts", "console.log(\"Hello from {name}!\");\n"),
        ],
        gitignore: GI_NODE,
    },
    Template {
        id: "python",
        files: &[
            (
                "README.md",
                "# {name}\n\n```bash\npython3 -m venv .venv && source .venv/bin/activate\npip install -r requirements.txt\npython main.py\n```\n",
            ),
            (
                "main.py",
                "def main() -> None:\n    print(\"Hello from {name}!\")\n\n\nif __name__ == \"__main__\":\n    main()\n",
            ),
            ("requirements.txt", ""),
        ],
        gitignore: GI_PY,
    },
    Template {
        id: "react-vite",
        files: &[
            (
                "README.md",
                "# {name}\n\n```bash\nnpm install\nnpm run dev\n```\n",
            ),
            (
                "package.json",
                "{\n  \"name\": \"{slug}\",\n  \"version\": \"0.1.0\",\n  \"private\": true,\n  \"type\": \"module\",\n  \"scripts\": {\n    \"dev\": \"vite\",\n    \"build\": \"tsc -b && vite build\",\n    \"preview\": \"vite preview\"\n  },\n  \"dependencies\": {\n    \"react\": \"^19\",\n    \"react-dom\": \"^19\"\n  },\n  \"devDependencies\": {\n    \"@types/react\": \"^19\",\n    \"@types/react-dom\": \"^19\",\n    \"@vitejs/plugin-react\": \"^5\",\n    \"typescript\": \"^5\",\n    \"vite\": \"^7\"\n  }\n}\n",
            ),
            (
                "index.html",
                "<!doctype html>\n<html lang=\"en\">\n  <head>\n    <meta charset=\"UTF-8\" />\n    <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\" />\n    <title>{name}</title>\n  </head>\n  <body>\n    <div id=\"root\"></div>\n    <script type=\"module\" src=\"/src/main.tsx\"></script>\n  </body>\n</html>\n",
            ),
            (
                "vite.config.ts",
                "import { defineConfig } from \"vite\";\nimport react from \"@vitejs/plugin-react\";\n\nexport default defineConfig({ plugins: [react()] });\n",
            ),
            (
                "tsconfig.json",
                "{\n  \"compilerOptions\": {\n    \"target\": \"ES2022\",\n    \"lib\": [\"ES2022\", \"DOM\"],\n    \"module\": \"ESNext\",\n    \"moduleResolution\": \"bundler\",\n    \"jsx\": \"react-jsx\",\n    \"strict\": true,\n    \"noEmit\": true,\n    \"skipLibCheck\": true\n  },\n  \"include\": [\"src\"]\n}\n",
            ),
            (
                "src/main.tsx",
                "import { StrictMode } from \"react\";\nimport { createRoot } from \"react-dom/client\";\nimport App from \"./App\";\n\ncreateRoot(document.getElementById(\"root\")!).render(\n  <StrictMode>\n    <App />\n  </StrictMode>,\n);\n",
            ),
            (
                "src/App.tsx",
                "export default function App() {\n  return <h1>{name}</h1>;\n}\n",
            ),
        ],
        gitignore: GI_NODE,
    },
];

pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in name.trim().chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    out.trim_end_matches('-').to_string()
}

fn write_template(dir: &Path, template: &Template, name: &str) -> Result<(), String> {
    let slug = slugify(name);
    for (rel, content) in template.files {
        let p = dir.join(rel);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let text = content.replace("{name}", name).replace("{slug}", &slug);
        std::fs::write(&p, text).map_err(|e| format!("writing {rel}: {e}"))?;
    }
    let gi = dir.join(".gitignore");
    if !gi.exists() {
        std::fs::write(gi, template.gitignore).map_err(|e| e.to_string())?;
    }
    Ok(())
}

// ---------- create ----------

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CreateRequest {
    pub name: String,
    pub group: Option<String>,
    pub template: String,
    pub git_init: bool,
    /// "none" | "private" | "public"
    pub github: String,
    /// GitHub repo name (defaults to a slug of `name`).
    pub repo_name: Option<String>,
    pub description: Option<String>,
}

#[derive(Serialize, Debug, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct CreateResult {
    pub path: String,
    pub github_slug: Option<String>,
    pub steps: Vec<String>,
    pub warnings: Vec<String>,
}

pub fn target_dir(cfg: &Config, group: Option<&str>, name: &str) -> Result<PathBuf, String> {
    let name = util::validate_name(name)?;
    let mut dir = cfg.root();
    if let Some(g) = group.filter(|g| !g.is_empty()) {
        dir = dir.join(util::validate_name(g)?);
    }
    Ok(dir.join(name))
}

fn commit_all(dir: &Path, message: &str) -> Result<(), String> {
    git::git(dir, &["add", "-A"])?;
    // Fall back to a generic identity only if the user hasn't configured one.
    let has_identity = git::git(dir, &["config", "user.email"]).is_ok();
    if has_identity {
        git::git(dir, &["commit", "-q", "-m", message])?;
    } else {
        git::git(
            dir,
            &[
                "-c",
                "user.name=Pelago",
                "-c",
                "user.email=pelago@localhost",
                "commit",
                "-q",
                "-m",
                message,
            ],
        )?;
    }
    Ok(())
}

pub fn create(cfg: &Config, req: &CreateRequest) -> Result<CreateResult, String> {
    let dir = target_dir(cfg, req.group.as_deref(), &req.name)?;
    if dir.exists() {
        return Err(format!("{} already exists", util::display_path(&dir)));
    }
    let template = TEMPLATES
        .iter()
        .find(|t| t.id == req.template)
        .ok_or_else(|| format!("Unknown template `{}`", req.template))?;
    let wants_github = req.github == "private" || req.github == "public";
    if wants_github {
        // Fail early, before anything touches disk.
        github::require_user()?;
    }

    let mut res = CreateResult {
        path: dir.to_string_lossy().into_owned(),
        ..Default::default()
    };
    std::fs::create_dir_all(&dir).map_err(|e| format!("Couldn't create folder: {e}"))?;
    let name = req.name.trim();
    let result = (|| -> Result<(), String> {
        write_template(&dir, template, name)?;
        res.steps
            .push(format!("Created {}", util::display_path(&dir)));

        if req.git_init || wants_github {
            git::git(&dir, &["init", "-q", "-b", "main"])?;
            commit_all(&dir, "Initial commit")?;
            res.steps.push("Initialized git repo".into());
        }
        if wants_github {
            let repo = req
                .repo_name
                .clone()
                .filter(|s| !s.trim().is_empty())
                .unwrap_or_else(|| slugify(name));
            let slug = github::create_from_local(
                &dir,
                &repo,
                req.github == "private",
                true,
                req.description.as_deref(),
            )?;
            res.steps
                .push(format!("Created {} GitHub repo {slug}", req.github));
            res.github_slug = Some(slug);
        }
        Ok(())
    })();

    if let Err(e) = result {
        // Roll back the folder we just made so a retry starts clean,
        // unless a GitHub repo was already created (then keep local copy).
        if res.github_slug.is_none() {
            let _ = std::fs::remove_dir_all(&dir);
        }
        return Err(e);
    }
    Ok(res)
}

/// Create a GitHub repo for an existing local project.
pub fn publish(dir: &Path, private: bool, repo_name: Option<&str>) -> Result<String, String> {
    github::require_user()?;
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !git::is_repo_root(dir) {
        git::git(dir, &["init", "-q", "-b", "main"])?;
    }
    if git::git(dir, &["rev-parse", "--verify", "-q", "HEAD"]).is_err() {
        if !dir.join(".gitignore").exists() {
            std::fs::write(
                dir.join(".gitignore"),
                GI_NODE.to_string() + "__pycache__/\n.venv/\n",
            )
            .map_err(|e| e.to_string())?;
        }
        commit_all(dir, "Initial commit")?;
    }
    if git::git(dir, &["remote", "get-url", "origin"]).is_ok() {
        return Err("This project already has an `origin` remote".into());
    }
    let repo = repo_name
        .map(String::from)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| slugify(&name));
    github::create_from_local(dir, &repo, private, true, None)
}

// ---------- groups ----------

pub fn create_group(cfg: &mut Config, meta: GroupMeta) -> Result<(), String> {
    let name = util::validate_name(&meta.name)?;
    if cfg
        .groups
        .iter()
        .any(|g| g.name.eq_ignore_ascii_case(&name))
    {
        return Err(format!("A group called {name} already exists"));
    }
    std::fs::create_dir_all(cfg.root().join(&name)).map_err(|e| e.to_string())?;
    cfg.groups.push(GroupMeta { name, ..meta });
    Ok(())
}

pub fn update_group(cfg: &mut Config, old_name: &str, meta: GroupMeta) -> Result<(), String> {
    let new_name = util::validate_name(&meta.name)?;
    let root = cfg.root();
    if new_name != old_name {
        let from = root.join(old_name);
        let to = root.join(&new_name);
        let case_only = new_name.eq_ignore_ascii_case(old_name);
        if to.exists() && !case_only {
            return Err(format!("{} already exists", util::display_path(&to)));
        }
        if from.exists() {
            if case_only {
                // macOS is case-insensitive: hop through a temp name.
                let tmp = root.join(format!(".pelago-rename-{}", util::now_secs()));
                std::fs::rename(&from, &tmp).map_err(|e| e.to_string())?;
                std::fs::rename(&tmp, &to).map_err(|e| e.to_string())?;
            } else {
                std::fs::rename(&from, &to).map_err(|e| e.to_string())?;
            }
        } else {
            std::fs::create_dir_all(&to).map_err(|e| e.to_string())?;
        }
        // Re-key project metadata that lived under the old folder.
        let from_prefix = format!("{}/", from.to_string_lossy());
        let to_prefix = format!("{}/", to.to_string_lossy());
        let keys: Vec<String> = cfg
            .projects
            .keys()
            .filter(|k| k.starts_with(&from_prefix))
            .cloned()
            .collect();
        for k in keys {
            let nk = k.replacen(&from_prefix, &to_prefix, 1);
            cfg.rename_project_key(&k, &nk);
        }
    }
    let entry = GroupMeta {
        name: new_name,
        ..meta
    };
    match cfg.groups.iter_mut().find(|g| g.name == old_name) {
        Some(g) => *g = entry,
        None => cfg.groups.push(entry),
    }
    Ok(())
}

/// Remove a group. Only allowed when its folder is empty (besides .DS_Store).
pub fn delete_group(cfg: &mut Config, name: &str) -> Result<(), String> {
    let dir = cfg.root().join(name);
    if dir.exists() {
        let leftover: Vec<String> = std::fs::read_dir(&dir)
            .map_err(|e| e.to_string())?
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .filter(|n| n != ".DS_Store")
            .collect();
        if !leftover.is_empty() {
            return Err(format!(
                "{name} still has {} item(s) in it. Move them out first.",
                leftover.len()
            ));
        }
        let _ = std::fs::remove_file(dir.join(".DS_Store"));
        std::fs::remove_dir(&dir).map_err(|e| e.to_string())?;
    }
    cfg.groups.retain(|g| g.name != name);
    Ok(())
}

// ---------- moves ----------

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct MoveRecord {
    pub id: String,
    pub name: String,
    pub from: String,
    pub to: String,
    pub at: i64,
    pub undone: bool,
}

#[derive(Serialize, Deserialize, Default, Debug)]
#[serde(default)]
pub struct MoveLog {
    pub moves: Vec<MoveRecord>,
}

fn log_path(data_dir: &Path) -> PathBuf {
    data_dir.join("moves.json")
}

pub fn load_log(data_dir: &Path) -> MoveLog {
    std::fs::read_to_string(log_path(data_dir))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save_log(data_dir: &Path, log: &MoveLog) -> Result<(), String> {
    config::write_json(&log_path(data_dir), log)
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct MovePreview {
    pub from: String,
    pub to: String,
    pub to_display: String,
    pub ok: bool,
    pub blockers: Vec<String>,
    pub warnings: Vec<String>,
}

fn is_cloud_synced(p: &Path) -> bool {
    let s = p.to_string_lossy();
    s.contains("/Library/CloudStorage/")
        || s.contains("/Mobile Documents/")
        || s.contains("/Google Drive/")
        || s.contains("/OneDrive/")
        || s.contains("/Dropbox/")
}

/// Things that might break when a folder moves.
fn move_warnings(from: &Path) -> Vec<String> {
    let mut w = vec![];
    for venv in [".venv", "venv", "env"] {
        let cfg = from.join(venv).join("pyvenv.cfg");
        if cfg.exists() {
            w.push(format!("Python virtualenv `{venv}/` has hard-coded paths and will need to be recreated (python3 -m venv {venv})."));
        }
    }
    if from.join(".git").is_file() {
        w.push(
            "This is a git worktree or submodule; its link to the main repo uses absolute paths."
                .into(),
        );
    }
    if let Ok(rd) = std::fs::read_dir(from) {
        let nested: Vec<String> = rd
            .filter_map(|e| e.ok())
            .filter(|e| e.path().join(".git").exists())
            .map(|e| e.file_name().to_string_lossy().into_owned())
            .collect();
        if !nested.is_empty() {
            w.push(format!(
                "Contains nested repos ({}); they move along with it.",
                nested.join(", ")
            ));
        }
    }
    w.push("VS Code's recent-files list and open tabs are tied to the old path.".into());
    w
}

pub fn preview_move(
    cfg: &Config,
    from: &Path,
    group: Option<&str>,
    new_name: Option<&str>,
) -> MovePreview {
    let name = new_name
        .map(String::from)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            from.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default()
        });
    let mut blockers = vec![];
    let to = match target_dir(cfg, group, &name) {
        Ok(t) => t,
        Err(e) => {
            blockers.push(e);
            cfg.root().join(&name)
        }
    };
    if !from.is_dir() {
        blockers.push(format!("{} doesn't exist", util::display_path(from)));
    }
    if to == from {
        blockers.push("It's already there".into());
    } else if to.exists() {
        blockers.push(format!("{} already exists", util::display_path(&to)));
    }
    if to.starts_with(from) {
        blockers.push("Can't move a folder inside itself".into());
    }
    let mut warnings = if from.is_dir() {
        move_warnings(from)
    } else {
        vec![]
    };
    if is_cloud_synced(&to) {
        warnings.push("The destination is inside a cloud-synced folder; syncing code (and node_modules) there can be slow.".into());
    }
    MovePreview {
        from: from.to_string_lossy().into_owned(),
        to_display: util::display_path(&to),
        to: to.to_string_lossy().into_owned(),
        ok: blockers.is_empty(),
        blockers,
        warnings,
    }
}

/// Rename, falling back to copy+delete across volumes.
fn move_dir(from: &Path, to: &Path) -> Result<(), String> {
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    match std::fs::rename(from, to) {
        Ok(()) => Ok(()),
        Err(e) if e.raw_os_error() == Some(18) => {
            // EXDEV: different volume. `ditto` preserves metadata, then verify and remove.
            util::run(
                "ditto",
                &[&from.to_string_lossy(), &to.to_string_lossy()],
                None,
            )?;
            let a = util::dir_size(from);
            let b = util::dir_size(to);
            if b + 4096 < a {
                let _ = std::fs::remove_dir_all(to);
                return Err("Copy verification failed; nothing was moved".into());
            }
            std::fs::remove_dir_all(from).map_err(|e| e.to_string())
        }
        Err(e) => Err(format!("Couldn't move: {e}")),
    }
}

pub fn do_move(
    cfg: &mut Config,
    data_dir: &Path,
    from: &Path,
    group: Option<&str>,
    new_name: Option<&str>,
) -> Result<MoveRecord, String> {
    let preview = preview_move(cfg, from, group, new_name);
    if !preview.ok {
        return Err(preview.blockers.join("; "));
    }
    let to = PathBuf::from(&preview.to);
    move_dir(from, &to)?;
    cfg.rename_project_key(&preview.from, &preview.to);
    let rec = MoveRecord {
        id: format!(
            "m{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ),
        name: to
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
        from: preview.from,
        to: preview.to,
        at: util::now_secs(),
        undone: false,
    };
    let mut log = load_log(data_dir);
    log.moves.push(rec.clone());
    if log.moves.len() > 200 {
        let drop = log.moves.len() - 200;
        log.moves.drain(..drop);
    }
    save_log(data_dir, &log)?;
    Ok(rec)
}

pub fn undo_move(cfg: &mut Config, data_dir: &Path, id: &str) -> Result<MoveRecord, String> {
    let mut log = load_log(data_dir);
    let rec = log
        .moves
        .iter_mut()
        .find(|m| m.id == id)
        .ok_or("That move isn't in the log anymore")?;
    if rec.undone {
        return Err("Already undone".into());
    }
    let (from, to) = (PathBuf::from(&rec.to), PathBuf::from(&rec.from));
    if !from.exists() {
        return Err(format!(
            "{} is gone, so it can't be moved back",
            util::display_path(&from)
        ));
    }
    if to.exists() {
        return Err(format!(
            "{} exists again, so it can't be moved back",
            util::display_path(&to)
        ));
    }
    move_dir(&from, &to)?;
    cfg.rename_project_key(&rec.to, &rec.from);
    rec.undone = true;
    let out = rec.clone();
    save_log(data_dir, &log)?;
    Ok(out)
}

/// Remove a `.git` folder only if the repo has zero commits and no stashes / refs.
/// Used for empty "wrapper" repos like a stray `git init` in a parent folder.
/// The `.git` folder is moved to the Trash, so it's recoverable.
pub fn remove_empty_repo(dir: &Path) -> Result<(), String> {
    remove_empty_repo_to(dir, &util::home().join(".Trash"))
}

fn remove_empty_repo_to(dir: &Path, trash_dir: &Path) -> Result<(), String> {
    let git_dir = dir.join(".git");
    if !git_dir.is_dir() {
        return Err("Not a git repo".into());
    }
    if git::git(dir, &["rev-parse", "--verify", "-q", "HEAD"]).is_ok() {
        return Err("This repo has commits; Pelago won't delete it".into());
    }
    let refs = git::git(dir, &["for-each-ref", "--count=1"]).unwrap_or_default();
    if !refs.trim().is_empty() {
        return Err("This repo has refs; Pelago won't delete it".into());
    }
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let trash = trash_dir.join(format!("{name}-empty-git-{}", util::now_secs()));
    std::fs::create_dir_all(trash_dir).map_err(|e| e.to_string())?;
    std::fs::rename(&git_dir, &trash).map_err(|e| format!("Couldn't move .git to Trash: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn cfg(root: &Path) -> Config {
        Config {
            projects_root: root.to_string_lossy().into_owned(),
            ..Config::default()
        }
    }

    #[test]
    fn slugs() {
        assert_eq!(slugify("My Cool App!"), "my-cool-app");
        assert_eq!(slugify("  --x__y  "), "x-y");
    }

    #[test]
    fn create_local_templates() {
        let tmp = tempfile::tempdir().unwrap();
        let c = cfg(tmp.path());
        for t in TEMPLATES {
            let req = CreateRequest {
                name: format!("App {}", t.id),
                group: Some("Experiments".into()),
                template: t.id.into(),
                git_init: true,
                github: "none".into(),
                repo_name: None,
                description: None,
            };
            let res = create(&c, &req).unwrap();
            let dir = PathBuf::from(&res.path);
            assert!(dir.join("README.md").exists());
            assert!(dir.join(".gitignore").exists());
            assert!(
                git::inspect(&dir).unwrap().has_commits,
                "{} should have a commit",
                t.id
            );
            assert_eq!(git::inspect(&dir).unwrap().dirty, 0);
        }
        // Duplicate name is rejected and doesn't clobber.
        let dup = CreateRequest {
            name: "App blank".into(),
            group: Some("Experiments".into()),
            template: "blank".into(),
            git_init: false,
            github: "none".into(),
            repo_name: None,
            description: None,
        };
        assert!(create(&c, &dup).is_err());
        // No git when toggled off.
        let nogit = CreateRequest {
            name: "plain".into(),
            git_init: false,
            ..dup
        };
        let res = create(&c, &nogit).unwrap();
        assert!(!Path::new(&res.path).join(".git").exists());
    }

    #[test]
    fn group_lifecycle() {
        let tmp = tempfile::tempdir().unwrap();
        let mut c = cfg(tmp.path());
        c.groups.clear();
        create_group(
            &mut c,
            GroupMeta {
                name: "Games".into(),
                emoji: "🎮".into(),
                color: "#000".into(),
            },
        )
        .unwrap();
        assert!(tmp.path().join("Games").is_dir());
        assert!(
            create_group(
                &mut c,
                GroupMeta {
                    name: "games".into(),
                    emoji: "x".into(),
                    color: "x".into()
                }
            )
            .is_err()
        );

        fs::create_dir_all(tmp.path().join("Games/pong")).unwrap();
        let pong = tmp.path().join("Games/pong").to_string_lossy().into_owned();
        c.meta_mut(&pong).pinned = true;
        update_group(
            &mut c,
            "Games",
            GroupMeta {
                name: "Gaming".into(),
                emoji: "🕹️".into(),
                color: "#111".into(),
            },
        )
        .unwrap();
        assert!(tmp.path().join("Gaming/pong").is_dir());
        assert!(!tmp.path().join("Games").exists());
        let new_key = tmp
            .path()
            .join("Gaming/pong")
            .to_string_lossy()
            .into_owned();
        assert!(c.projects.get(&new_key).unwrap().pinned);

        // Case-only rename works on case-insensitive filesystems.
        update_group(
            &mut c,
            "Gaming",
            GroupMeta {
                name: "GAMING".into(),
                emoji: "🕹️".into(),
                color: "#111".into(),
            },
        )
        .unwrap();
        assert!(c.groups.iter().any(|g| g.name == "GAMING"));

        assert!(
            delete_group(&mut c, "GAMING").is_err(),
            "non-empty group can't be deleted"
        );
        fs::remove_dir_all(tmp.path().join("GAMING/pong")).unwrap();
        fs::write(tmp.path().join("GAMING/.DS_Store"), "").unwrap();
        delete_group(&mut c, "GAMING").unwrap();
        assert!(c.groups.is_empty());
    }

    #[test]
    fn move_and_undo() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Code");
        let data = tmp.path().join("data");
        let mut c = cfg(&root);
        let src = tmp.path().join("old/thing");
        fs::create_dir_all(src.join(".venv")).unwrap();
        fs::write(src.join(".venv/pyvenv.cfg"), "home = /x").unwrap();
        fs::write(src.join("main.py"), "print(1)").unwrap();
        c.meta_mut(&src.to_string_lossy()).pinned = true;

        let pv = preview_move(&c, &src, Some("Experiments"), None);
        assert!(pv.ok, "{:?}", pv.blockers);
        assert!(pv.warnings.iter().any(|w| w.contains("virtualenv")));

        let rec = do_move(&mut c, &data, &src, Some("Experiments"), None).unwrap();
        let dest = root.join("Experiments/thing");
        assert!(dest.join("main.py").exists());
        assert!(!src.exists());
        assert!(
            c.projects
                .get(&dest.to_string_lossy().into_owned())
                .unwrap()
                .pinned
        );

        // Conflicts are blocked.
        fs::create_dir_all(&src).unwrap();
        assert!(!preview_move(&c, &src, Some("Experiments"), None).ok);
        fs::remove_dir(&src).unwrap();

        undo_move(&mut c, &data, &rec.id).unwrap();
        assert!(src.join("main.py").exists());
        assert!(!dest.exists());
        assert!(
            c.projects
                .get(&src.to_string_lossy().into_owned())
                .unwrap()
                .pinned
        );
        assert!(undo_move(&mut c, &data, &rec.id).is_err());
        assert_eq!(load_log(&data).moves.len(), 1);
    }

    #[test]
    fn empty_repo_removal_is_guarded() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join("wrapper");
        fs::create_dir_all(&p).unwrap();
        git::git(&p, &["init", "-q"]).unwrap();
        fs::write(p.join("f"), "x").unwrap();
        git::git(&p, &["add", "."]).unwrap();
        git::git(
            &p,
            &[
                "-c",
                "user.name=t",
                "-c",
                "user.email=t@t",
                "commit",
                "-qm",
                "x",
            ],
        )
        .unwrap();
        assert!(
            remove_empty_repo_to(&p, &tmp.path().join("Trash")).is_err(),
            "repo with commits must be kept"
        );

        let q = tmp.path().join("empty");
        fs::create_dir_all(&q).unwrap();
        git::git(&q, &["init", "-q"]).unwrap();
        remove_empty_repo_to(&q, &tmp.path().join("Trash")).unwrap();
        assert!(!q.join(".git").exists());
        assert_eq!(fs::read_dir(tmp.path().join("Trash")).unwrap().count(), 1);
    }
}

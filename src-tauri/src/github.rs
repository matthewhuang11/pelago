//! GitHub access through the user's own `gh` CLI. Pelago never stores tokens.
#![allow(dead_code)] // helpers used by the create/park flows

use serde::Serialize;
use std::path::Path;

use crate::util;

#[derive(Serialize, Clone, Debug, Default)]
#[serde(rename_all = "camelCase")]
pub struct GhStatus {
    pub installed: bool,
    pub logged_in: bool,
    pub user: Option<String>,
    pub git_installed: bool,
    pub brew_installed: bool,
}

pub fn status() -> GhStatus {
    let installed = util::which("gh").is_some();
    let mut s = GhStatus {
        installed,
        git_installed: util::which("git").is_some(),
        brew_installed: util::which("brew").is_some(),
        ..Default::default()
    };
    if installed && let Ok(login) = util::run("gh", &["api", "user", "--jq", ".login"], None) {
        let login = login.trim().to_string();
        if !login.is_empty() {
            s.logged_in = true;
            s.user = Some(login);
        }
    }
    s
}

pub fn require_user() -> Result<String, String> {
    let s = status();
    if !s.installed {
        return Err("GitHub CLI (gh) isn't installed. Open Settings → GitHub to set it up.".into());
    }
    s.user
        .ok_or_else(|| "You're not signed in to GitHub. Open Settings → GitHub to sign in.".into())
}

/// Returns Some(true) if private, Some(false) if public, None if unknown / no access.
pub fn repo_is_private(slug: &str) -> Option<bool> {
    util::run(
        "gh",
        &[
            "repo",
            "view",
            slug,
            "--json",
            "isPrivate",
            "--jq",
            ".isPrivate",
        ],
        None,
    )
    .ok()
    .map(|s| s.trim() == "true")
}

pub fn repo_exists(slug: &str) -> bool {
    util::run("gh", &["repo", "view", slug, "--json", "name"], None).is_ok()
}

/// Create a GitHub repo from an existing local folder and push it.
pub fn create_from_local(
    dir: &Path,
    name: &str,
    private: bool,
    push: bool,
) -> Result<String, String> {
    let dir_s = dir.to_string_lossy();
    let mut args = vec![
        "repo",
        "create",
        name,
        if private { "--private" } else { "--public" },
        "--source",
        &dir_s,
        "--remote",
        "origin",
    ];
    if push {
        args.push("--push");
    }
    util::run("gh", &args, Some(dir))?;
    let user = require_user()?;
    Ok(format!("{user}/{name}"))
}

/// Create an empty private repo (used for the vault).
pub fn create_empty_private(name: &str, description: &str) -> Result<(), String> {
    util::run(
        "gh",
        &[
            "repo",
            "create",
            name,
            "--private",
            "--description",
            description,
        ],
        None,
    )
    .map(|_| ())
}

/// Clone URL that works with whatever protocol the user configured in `gh`.
pub fn clone_url(slug: &str) -> String {
    let proto = util::run(
        "gh",
        &["config", "get", "git_protocol", "-h", "github.com"],
        None,
    )
    .unwrap_or_default();
    if proto.trim() == "ssh" {
        format!("git@github.com:{slug}.git")
    } else {
        format!("https://github.com/{slug}.git")
    }
}

/// Make sure plain `git` can authenticate HTTPS pushes using gh's login.
pub fn setup_git_credentials() {
    let _ = util::run("gh", &["auth", "setup-git"], None);
}

/// Open Terminal running `gh auth login` (it's interactive, so it needs a TTY).
pub fn open_login_terminal() -> Result<(), String> {
    let cmd = if util::which("gh").is_some() {
        "gh auth login --web --git-protocol https && gh auth setup-git && echo && echo '✅ Done! You can close this window and go back to Pelago.'"
    } else {
        "brew install gh && gh auth login --web --git-protocol https && gh auth setup-git && echo && echo '✅ Done! You can close this window and go back to Pelago.'"
    };
    let script = format!(
        "tell application \"Terminal\"\n activate\n do script \"{}\"\nend tell",
        cmd.replace('\\', "\\\\").replace('"', "\\\"")
    );
    util::run("osascript", &["-e", &script], None).map(|_| ())
}

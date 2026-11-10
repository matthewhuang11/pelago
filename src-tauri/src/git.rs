//! Read-only git inspection. Every call shells out to the user's own `git`.

use serde::Serialize;
use std::path::Path;

use crate::util;

#[derive(Serialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct GitInfo {
    pub branch: Option<String>,
    pub has_commits: bool,
    pub remote_url: Option<String>,
    /// "owner/repo" if the origin remote points at GitHub.
    pub github_slug: Option<String>,
    /// Number of changed + untracked entries.
    pub dirty: u32,
    pub ahead: u32,
    pub behind: u32,
    pub has_upstream: bool,
    pub last_commit: Option<i64>,
}

pub fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    util::run("git", args, Some(dir))
}

pub fn is_repo_root(dir: &Path) -> bool {
    dir.join(".git").exists()
}

/// Parse the `## ...` header of `git status --porcelain=v1 -b`.
/// Returns (branch, has_commits, has_upstream, ahead, behind).
pub fn parse_branch_header(line: &str) -> (Option<String>, bool, bool, u32, u32) {
    let rest = line.trim_start_matches("## ").trim();
    if let Some(b) = rest.strip_prefix("No commits yet on ") {
        return (Some(b.trim().to_string()), false, false, 0, 0);
    }
    if let Some(b) = rest.strip_prefix("Initial commit on ") {
        return (Some(b.trim().to_string()), false, false, 0, 0);
    }
    if rest.starts_with("HEAD (no branch)") {
        return (None, true, false, 0, 0);
    }
    let (names, tracking) = match rest.find(" [") {
        Some(i) => (&rest[..i], Some(&rest[i + 2..rest.len().saturating_sub(1)])),
        None => (rest, None),
    };
    let (branch, upstream) = match names.split_once("...") {
        Some((b, u)) => (b.to_string(), Some(u)),
        None => (names.to_string(), None),
    };
    let (mut ahead, mut behind) = (0, 0);
    if let Some(t) = tracking {
        for part in t.split(", ") {
            if let Some(n) = part.strip_prefix("ahead ") {
                ahead = n.parse().unwrap_or(0);
            } else if let Some(n) = part.strip_prefix("behind ") {
                behind = n.parse().unwrap_or(0);
            }
        }
    }
    let gone = tracking.is_some_and(|t| t.contains("gone"));
    (
        Some(branch),
        true,
        upstream.is_some() && !gone,
        ahead,
        behind,
    )
}

/// Extract "owner/repo" from any common GitHub remote URL form.
pub fn github_slug(url: &str) -> Option<String> {
    let url = url.trim();
    let rest = url
        .strip_prefix("git@github.com:")
        .or_else(|| url.strip_prefix("ssh://git@github.com/"))
        .or_else(|| url.strip_prefix("https://github.com/"))
        .or_else(|| url.strip_prefix("http://github.com/"))
        .or_else(|| url.strip_prefix("git://github.com/"))
        .or_else(|| {
            // https://user@github.com/owner/repo
            let after_scheme = url.strip_prefix("https://")?;
            let (_, path) = after_scheme.split_once("@github.com/")?;
            Some(path)
        })?;
    let rest = rest.trim_end_matches('/').trim_end_matches(".git");
    let mut parts = rest.split('/');
    let owner = parts.next()?;
    let repo = parts.next()?;
    if owner.is_empty() || repo.is_empty() || parts.next().is_some() {
        return None;
    }
    Some(format!("{owner}/{repo}"))
}

/// Normalized key used to spot duplicate checkouts of the same repo.
pub fn remote_key(url: &str) -> String {
    github_slug(url)
        .map(|s| format!("github:{}", s.to_lowercase()))
        .unwrap_or_else(|| url.trim().trim_end_matches(".git").to_lowercase())
}

pub fn inspect(dir: &Path) -> Option<GitInfo> {
    if !is_repo_root(dir) {
        return None;
    }
    let mut info = GitInfo::default();
    if let Ok(status) = git(
        dir,
        &["status", "--porcelain=v1", "-b", "--untracked-files=normal"],
    ) {
        let mut lines = status.lines();
        if let Some(header) = lines.next() {
            let (branch, has_commits, has_upstream, ahead, behind) = parse_branch_header(header);
            info.branch = branch;
            info.has_commits = has_commits;
            info.has_upstream = has_upstream;
            info.ahead = ahead;
            info.behind = behind;
        }
        info.dirty = lines.filter(|l| !l.trim().is_empty()).count() as u32;
    }
    info.remote_url = git(dir, &["config", "--get", "remote.origin.url"])
        .ok()
        .filter(|s| !s.is_empty());
    if info.remote_url.is_none() {
        // Fall back to the first remote, whatever it's called.
        if let Ok(remotes) = git(dir, &["remote"])
            && let Some(first) = remotes.lines().next()
        {
            info.remote_url = git(dir, &["remote", "get-url", first]).ok();
        }
    }
    info.github_slug = info.remote_url.as_deref().and_then(github_slug);
    if info.has_commits {
        info.last_commit = git(dir, &["log", "-1", "--format=%ct"])
            .ok()
            .and_then(|s| s.trim().parse().ok());
    }
    Some(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn headers() {
        assert_eq!(
            parse_branch_header("## main"),
            (Some("main".into()), true, false, 0, 0)
        );
        assert_eq!(
            parse_branch_header("## main...origin/main [ahead 2, behind 3]"),
            (Some("main".into()), true, true, 2, 3)
        );
        assert_eq!(
            parse_branch_header("## dev...origin/dev"),
            (Some("dev".into()), true, true, 0, 0)
        );
        assert_eq!(
            parse_branch_header("## No commits yet on main"),
            (Some("main".into()), false, false, 0, 0)
        );
        assert_eq!(
            parse_branch_header("## HEAD (no branch)"),
            (None, true, false, 0, 0)
        );
        assert_eq!(
            parse_branch_header("## x...origin/x [gone]"),
            (Some("x".into()), true, false, 0, 0)
        );
    }

    #[test]
    fn slugs() {
        assert_eq!(
            github_slug("git@github.com:me/app.git").as_deref(),
            Some("me/app")
        );
        assert_eq!(
            github_slug("https://github.com/Me/App.git").as_deref(),
            Some("Me/App")
        );
        assert_eq!(
            github_slug("https://github.com/me/app/").as_deref(),
            Some("me/app")
        );
        assert_eq!(
            github_slug("https://tok@github.com/me/app").as_deref(),
            Some("me/app")
        );
        assert_eq!(
            github_slug("ssh://git@github.com/me/app.git").as_deref(),
            Some("me/app")
        );
        assert_eq!(github_slug("https://gitlab.com/me/app"), None);
        assert_eq!(
            remote_key("git@github.com:Me/App.git"),
            remote_key("https://github.com/me/app")
        );
    }

    #[test]
    fn inspect_real_repo() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path();
        git(p, &["init", "-q", "-b", "main"]).unwrap();
        let info = inspect(p).unwrap();
        assert!(!info.has_commits);
        assert_eq!(info.branch.as_deref(), Some("main"));

        std::fs::write(p.join("a.txt"), "hi").unwrap();
        git(p, &["add", "."]).unwrap();
        git(
            p,
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
        std::fs::write(p.join("b.txt"), "new").unwrap();
        git(
            p,
            &["remote", "add", "origin", "git@github.com:me/thing.git"],
        )
        .unwrap();
        let info = inspect(p).unwrap();
        assert!(info.has_commits);
        assert_eq!(info.dirty, 1);
        assert_eq!(info.github_slug.as_deref(), Some("me/thing"));
        assert!(info.last_commit.is_some());
    }
}

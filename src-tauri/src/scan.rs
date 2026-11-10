//! Project discovery.
//!
//! Inside the projects root, the layout is `<root>/<Group>/<project>`: every
//! top-level folder is a group (unless it is itself a project, in which case
//! it is shown as ungrouped). Outside the root, each configured scan path is
//! searched up to two levels deep for things that look like code.

use rayon::prelude::*;
use serde::Serialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::config::Config;
use crate::git::{self, GitInfo};
use crate::util;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: String,
    pub display_path: String,
    /// Group folder name when the project lives in `<root>/<group>/`.
    pub group: Option<String>,
    /// "root" (inside the projects root) | "external" (found via a scan path)
    pub location: String,
    /// "local" | "parked"
    pub status: String,
    pub git: Option<GitInfo>,
    pub stack: Vec<String>,
    pub empty: bool,
    /// Paths (relative) of git repos nested inside this project.
    pub nested_repos: Vec<String>,
    /// Paths of other checkouts pointing at the same remote.
    pub duplicates: Vec<String>,
    pub last_activity: i64,
    pub pinned: bool,
    pub editor: Option<String>,
    pub last_opened: Option<i64>,
    pub suggested_group: Option<String>,
    /// Filled for parked projects (bytes freed), otherwise computed lazily.
    pub size_bytes: Option<u64>,
    pub parked_at: Option<i64>,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct GroupInfo {
    pub name: String,
    pub emoji: String,
    pub color: String,
    pub exists: bool,
    pub count: usize,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Warning {
    /// "emptyOuterRepo" | "missingScanPath" | "missingRoot"
    pub kind: String,
    pub path: String,
    pub message: String,
}

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ProjectList {
    pub projects: Vec<Project>,
    pub groups: Vec<GroupInfo>,
    pub warnings: Vec<Warning>,
    pub root: String,
}

const MARKER_FILES: &[&str] = &[
    "package.json",
    "Cargo.toml",
    "pyproject.toml",
    "requirements.txt",
    "setup.py",
    "Pipfile",
    "go.mod",
    "pom.xml",
    "build.gradle",
    "build.gradle.kts",
    "Package.swift",
    "Gemfile",
    "composer.json",
    "CMakeLists.txt",
    "Makefile",
    "deno.json",
    "pubspec.yaml",
    "index.html",
    "environment.yml",
    "mix.exs",
    "stack.yaml",
    "project.clj",
    "Dockerfile",
];

const CODE_EXTENSIONS: &[&str] = &[
    "py",
    "ipynb",
    "js",
    "jsx",
    "ts",
    "tsx",
    "mjs",
    "rs",
    "go",
    "java",
    "kt",
    "swift",
    "c",
    "h",
    "cc",
    "cpp",
    "hpp",
    "cs",
    "rb",
    "php",
    "html",
    "css",
    "scss",
    "vue",
    "svelte",
    "lua",
    "r",
    "jl",
    "scala",
    "dart",
    "ex",
    "exs",
    "hs",
    "ml",
    "sh",
    "sql",
    "m",
    "mm",
    "toe",
    "sln",
    "xcodeproj",
    "xcworkspace",
    "csproj",
];

const SKIP_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "target",
    "dist",
    "build",
    ".venv",
    "venv",
    "__pycache__",
    ".next",
];

fn ext_of(name: &str) -> Option<String> {
    Path::new(name)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
}

fn entries(dir: &Path) -> Vec<(String, PathBuf, bool)> {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return vec![];
    };
    rd.filter_map(|e| e.ok())
        .map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
            (name, e.path(), is_dir)
        })
        .collect()
}

/// Does this folder look like a programming project?
pub fn looks_like_project(dir: &Path) -> bool {
    if git::is_repo_root(dir) {
        return true;
    }
    entries(dir).iter().any(|(name, _, _)| {
        MARKER_FILES.contains(&name.as_str())
            || ext_of(name).is_some_and(|e| CODE_EXTENSIONS.contains(&e.as_str()))
    })
}

fn is_empty_dir(dir: &Path) -> bool {
    entries(dir).iter().all(|(name, _, _)| name == ".DS_Store")
}

/// Best-effort stack detection from top-level files.
pub fn detect_stack(dir: &Path) -> Vec<String> {
    let names: Vec<String> = entries(dir).into_iter().map(|(n, _, _)| n).collect();
    let has = |f: &str| names.iter().any(|n| n == f);
    let has_ext = |e: &str| names.iter().any(|n| ext_of(n).as_deref() == Some(e));
    let mut stack = vec![];
    if has("package.json") {
        let pkg = std::fs::read_to_string(dir.join("package.json")).unwrap_or_default();
        let dep = |d: &str| pkg.contains(&format!("\"{d}\""));
        if dep("next") {
            stack.push("Next.js");
        } else if dep("react") {
            stack.push("React");
        } else if dep("vue") {
            stack.push("Vue");
        } else if dep("svelte") {
            stack.push("Svelte");
        }
        if dep("@tauri-apps/api") {
            stack.push("Tauri");
        }
        if dep("electron") {
            stack.push("Electron");
        }
        if has("tsconfig.json") || dep("typescript") {
            stack.push("TypeScript");
        } else {
            stack.push("JavaScript");
        }
    }
    if has("Cargo.toml") {
        stack.push("Rust");
    }
    if has("pyproject.toml")
        || has("requirements.txt")
        || has("setup.py")
        || has("Pipfile")
        || has_ext("py")
    {
        stack.push("Python");
    }
    if has_ext("ipynb") {
        stack.push("Jupyter");
    }
    if has("go.mod") {
        stack.push("Go");
    }
    if has("Package.swift") || has_ext("xcodeproj") || has_ext("swift") {
        stack.push("Swift");
    }
    if has("pom.xml") || has("build.gradle") || has("build.gradle.kts") || has_ext("java") {
        stack.push("Java");
    }
    if has_ext("kt") {
        stack.push("Kotlin");
    }
    if has("CMakeLists.txt") || has_ext("cpp") || has_ext("cc") {
        stack.push("C++");
    } else if has_ext("c") {
        stack.push("C");
    }
    if has("Gemfile") {
        stack.push("Ruby");
    }
    if has("composer.json") || has_ext("php") {
        stack.push("PHP");
    }
    if has("pubspec.yaml") {
        stack.push("Flutter");
    }
    if has_ext("toe") {
        stack.push("TouchDesigner");
    }
    if stack.is_empty() && (has("index.html") || has_ext("html")) {
        stack.push("HTML");
    }
    stack.into_iter().map(String::from).collect()
}

/// Find git repos nested below `dir` (depth 1 and 2), skipping heavy folders.
fn nested_repos(dir: &Path) -> Vec<String> {
    let mut found = vec![];
    for (name, path, is_dir) in entries(dir) {
        if !is_dir || SKIP_DIRS.contains(&name.as_str()) || name.starts_with('.') {
            continue;
        }
        if git::is_repo_root(&path) {
            found.push(name.clone());
            continue;
        }
        for (sub, subpath, sub_dir) in entries(&path) {
            if sub_dir && !SKIP_DIRS.contains(&sub.as_str()) && git::is_repo_root(&subpath) {
                found.push(format!("{name}/{sub}"));
            }
        }
    }
    found.sort();
    found
}

/// Rough guess at which starter group an external project belongs in.
pub fn suggest_group(name: &str, stack: &[String], groups: &[String]) -> Option<String> {
    let n = name.to_lowercase();
    let pick = |g: &str| groups.iter().find(|x| x.eq_ignore_ascii_case(g)).cloned();
    if n.contains("hackathon") || n.contains("hack-") || n.contains("devpost") {
        return pick("Hackathons");
    }
    if n.contains("website")
        || n.contains("landing")
        || n.ends_with("site")
        || n.contains("portfolio")
    {
        return pick("Websites");
    }
    if n.contains("cs1")
        || n.contains("cs2")
        || n.contains("homework")
        || n.contains("class")
        || n.contains("lab")
    {
        return pick("School");
    }
    if n.contains("test")
        || n.contains("sandbox")
        || n.contains("playground")
        || n.contains("demo")
        || n.ends_with("-old")
    {
        return pick("Experiments");
    }
    if stack.iter().any(|s| s == "HTML") {
        return pick("Websites");
    }
    None
}

struct Candidate {
    path: PathBuf,
    group: Option<String>,
    location: &'static str,
}

fn collect_candidates(config: &Config, warnings: &mut Vec<Warning>) -> Vec<Candidate> {
    let mut out = vec![];
    let root = config.root();
    if root.is_dir() {
        for (name, path, is_dir) in entries(&root) {
            if !is_dir || name.starts_with('.') {
                continue;
            }
            if looks_like_project(&path) {
                out.push(Candidate {
                    path,
                    group: None,
                    location: "root",
                });
                continue;
            }
            for (sub, subpath, sub_dir) in entries(&path) {
                if sub_dir && !sub.starts_with('.') {
                    out.push(Candidate {
                        path: subpath,
                        group: Some(name.clone()),
                        location: "root",
                    });
                }
            }
        }
    } else {
        warnings.push(Warning {
            kind: "missingRoot".into(),
            path: root.to_string_lossy().into_owned(),
            message: format!(
                "Projects folder {} doesn't exist yet",
                util::display_path(&root)
            ),
        });
    }

    for scan in &config.scan_paths {
        let scan_path = util::expand_tilde(scan);
        if !scan_path.is_dir() {
            warnings.push(Warning {
                kind: "missingScanPath".into(),
                path: scan.clone(),
                message: format!("{} no longer exists", util::display_path(&scan_path)),
            });
            continue;
        }
        if scan_path.starts_with(&root) {
            continue;
        }
        if git::is_repo_root(&scan_path) {
            let has_commits =
                git::git(&scan_path, &["rev-parse", "--verify", "-q", "HEAD"]).is_ok();
            if !has_commits {
                warnings.push(Warning {
                    kind: "emptyOuterRepo".into(),
                    path: scan_path.to_string_lossy().into_owned(),
                    message: format!(
                        "{} is an empty git repo (0 commits) wrapped around other projects. It's safe to remove.",
                        util::display_path(&scan_path)
                    ),
                });
            }
        }
        for (name, path, is_dir) in entries(&scan_path) {
            if !is_dir || name.starts_with('.') || SKIP_DIRS.contains(&name.as_str()) {
                continue;
            }
            if looks_like_project(&path) || is_empty_dir(&path) {
                out.push(Candidate {
                    path,
                    group: None,
                    location: "external",
                });
            } else {
                // A plain folder of projects: look one level deeper.
                for (sub, subpath, sub_dir) in entries(&path) {
                    if sub_dir && !sub.starts_with('.') && looks_like_project(&subpath) {
                        out.push(Candidate {
                            path: subpath,
                            group: None,
                            location: "external",
                        });
                    }
                }
            }
        }
    }
    // Dedup identical paths (overlapping scan paths).
    out.sort_by(|a, b| a.path.cmp(&b.path));
    out.dedup_by(|a, b| a.path == b.path);
    out
}

pub fn scan(config: &Config) -> ProjectList {
    let mut warnings = vec![];
    let candidates = collect_candidates(config, &mut warnings);
    let group_names: Vec<String> = config.groups.iter().map(|g| g.name.clone()).collect();

    let mut projects: Vec<Project> = candidates
        .par_iter()
        .map(|c| {
            let path_str = c.path.to_string_lossy().into_owned();
            let name = c
                .path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            let git = git::inspect(&c.path);
            let stack = detect_stack(&c.path);
            let empty = is_empty_dir(&c.path);
            let meta = config.projects.get(&path_str).cloned().unwrap_or_default();
            let last_activity = git
                .as_ref()
                .and_then(|g| g.last_commit)
                .unwrap_or(0)
                .max(util::mtime_secs(&c.path));
            let suggested_group = if c.location == "external" {
                suggest_group(&name, &stack, &group_names)
            } else {
                None
            };
            Project {
                id: path_str.clone(),
                display_path: util::display_path(&c.path),
                path: path_str,
                group: c.group.clone(),
                location: c.location.into(),
                status: "local".into(),
                nested_repos: nested_repos(&c.path),
                duplicates: vec![],
                git,
                stack,
                empty,
                last_activity,
                pinned: meta.pinned,
                editor: meta.editor,
                last_opened: meta.last_opened,
                suggested_group,
                size_bytes: None,
                parked_at: None,
                name,
            }
        })
        .collect();

    // Duplicate detection by normalized remote.
    let mut by_remote: HashMap<String, Vec<String>> = HashMap::new();
    for p in &projects {
        if let Some(url) = p.git.as_ref().and_then(|g| g.remote_url.as_deref()) {
            by_remote
                .entry(git::remote_key(url))
                .or_default()
                .push(p.path.clone());
        }
    }
    for p in &mut projects {
        if let Some(url) = p.git.as_ref().and_then(|g| g.remote_url.as_deref())
            && let Some(paths) = by_remote.get(&git::remote_key(url))
        {
            p.duplicates = paths.iter().filter(|x| **x != p.path).cloned().collect();
        }
    }

    projects.sort_by_key(|a| a.name.to_lowercase());
    let root = config.root();
    let groups = build_groups(config, &projects, &root);
    ProjectList {
        projects,
        groups,
        warnings,
        root: root.to_string_lossy().into_owned(),
    }
}

pub fn build_groups(config: &Config, projects: &[Project], root: &Path) -> Vec<GroupInfo> {
    let mut groups: Vec<GroupInfo> = config
        .groups
        .iter()
        .map(|g| GroupInfo {
            name: g.name.clone(),
            emoji: g.emoji.clone(),
            color: g.color.clone(),
            exists: root.join(&g.name).is_dir(),
            count: 0,
        })
        .collect();
    // Folders on disk that aren't in config yet still show up as groups.
    if let Ok(rd) = std::fs::read_dir(root) {
        for e in rd.filter_map(|e| e.ok()) {
            let name = e.file_name().to_string_lossy().into_owned();
            let path = e.path();
            if path.is_dir()
                && !name.starts_with('.')
                && !looks_like_project(&path)
                && !groups.iter().any(|g| g.name == name)
            {
                groups.push(GroupInfo {
                    name,
                    emoji: "📁".into(),
                    color: "#1c9af0".into(),
                    exists: true,
                    count: 0,
                });
            }
        }
    }
    for g in &mut groups {
        g.count = projects
            .iter()
            .filter(|p| p.group.as_deref() == Some(&g.name))
            .count();
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn cfg(root: &Path, scan: &[&Path]) -> Config {
        Config {
            projects_root: root.to_string_lossy().into_owned(),
            scan_paths: scan
                .iter()
                .map(|p| p.to_string_lossy().into_owned())
                .collect(),
            ..Config::default()
        }
    }

    #[test]
    fn scans_root_groups_and_external() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Code");
        fs::create_dir_all(root.join("Startups/rocket")).unwrap();
        fs::write(
            root.join("Startups/rocket/package.json"),
            r#"{"dependencies":{"react":"1"}}"#,
        )
        .unwrap();
        fs::create_dir_all(root.join("Experiments")).unwrap();
        fs::create_dir_all(root.join("loose-app")).unwrap();
        fs::write(root.join("loose-app/main.py"), "").unwrap();

        let ext = tmp.path().join("Old Projects");
        fs::create_dir_all(ext.join("my-website")).unwrap();
        fs::write(ext.join("my-website/index.html"), "").unwrap();
        fs::create_dir_all(ext.join("empty-one")).unwrap();
        fs::create_dir_all(ext.join("notes")).unwrap();
        fs::write(ext.join("notes/todo.txt"), "").unwrap();
        fs::create_dir_all(ext.join("bundle/inner")).unwrap();
        fs::write(ext.join("bundle/inner/lib.rs"), "").unwrap();

        let list = scan(&cfg(&root, &[&ext]));
        let names: Vec<_> = list.projects.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["empty-one", "inner", "loose-app", "my-website", "rocket"]
        );

        let rocket = list.projects.iter().find(|p| p.name == "rocket").unwrap();
        assert_eq!(rocket.group.as_deref(), Some("Startups"));
        assert_eq!(rocket.stack, vec!["React", "JavaScript"]);
        let site = list
            .projects
            .iter()
            .find(|p| p.name == "my-website")
            .unwrap();
        assert_eq!(site.location, "external");
        assert_eq!(site.suggested_group.as_deref(), Some("Websites"));
        assert!(
            list.projects
                .iter()
                .find(|p| p.name == "empty-one")
                .unwrap()
                .empty
        );
        assert!(
            list.projects
                .iter()
                .find(|p| p.name == "loose-app")
                .unwrap()
                .group
                .is_none()
        );

        let startups = list.groups.iter().find(|g| g.name == "Startups").unwrap();
        assert!(startups.exists);
        assert_eq!(startups.count, 1);
        assert!(
            !list
                .groups
                .iter()
                .find(|g| g.name == "School")
                .unwrap()
                .exists
        );
    }

    #[test]
    fn flags_duplicates_nested_and_empty_outer_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().join("Code");
        let ext = tmp.path().join("Better");
        for name in ["a", "a2"] {
            let p = ext.join(name);
            fs::create_dir_all(&p).unwrap();
            git::git(&p, &["init", "-q"]).unwrap();
        }
        git::git(
            &ext.join("a"),
            &["remote", "add", "origin", "git@github.com:me/a.git"],
        )
        .unwrap();
        git::git(
            &ext.join("a2"),
            &["remote", "add", "origin", "https://github.com/me/a"],
        )
        .unwrap();
        git::git(&ext, &["init", "-q"]).unwrap();
        let mono = ext.join("mono");
        fs::create_dir_all(mono.join("svc")).unwrap();
        git::git(&mono, &["init", "-q"]).unwrap();
        git::git(&mono.join("svc"), &["init", "-q"]).unwrap();

        let list = scan(&cfg(&root, &[&ext]));
        let a = list.projects.iter().find(|p| p.name == "a").unwrap();
        assert_eq!(a.duplicates.len(), 1);
        assert!(a.duplicates[0].ends_with("a2"));
        let mono = list.projects.iter().find(|p| p.name == "mono").unwrap();
        assert_eq!(mono.nested_repos, vec!["svc"]);
        assert!(list.warnings.iter().any(|w| w.kind == "emptyOuterRepo"));
        assert!(list.warnings.iter().any(|w| w.kind == "missingRoot"));
    }

    #[test]
    fn group_suggestions() {
        let g: Vec<String> = crate::config::starter_groups()
            .into_iter()
            .map(|g| g.name)
            .collect();
        assert_eq!(
            suggest_group("naive-hackathon-nov-2026", &[], &g).as_deref(),
            Some("Hackathons")
        );
        assert_eq!(
            suggest_group("warden-landing", &[], &g).as_deref(),
            Some("Websites")
        );
        assert_eq!(
            suggest_group("warden-old", &[], &g).as_deref(),
            Some("Experiments")
        );
        assert_eq!(suggest_group("warden", &[], &g), None);
    }
}

#[cfg(test)]
mod real_tests {
    /// Manual smoke test against real folders:
    /// PELAGO_SCAN="~/A:~/B" cargo test real_scan -- --ignored --nocapture
    #[test]
    #[ignore]
    fn real_scan() {
        let paths = std::env::var("PELAGO_SCAN").unwrap_or_default();
        let cfg = crate::config::Config {
            projects_root: "~/Code".into(),
            scan_paths: paths
                .split(':')
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect(),
            ..Default::default()
        };
        let t = std::time::Instant::now();
        let list = super::scan(&cfg);
        for p in &list.projects {
            println!(
                "{:<40} stack={:?} git={} dirty={} dup={} nested={:?} suggest={:?} empty={}",
                p.display_path,
                p.stack,
                p.git
                    .as_ref()
                    .and_then(|g| g.github_slug.clone())
                    .unwrap_or("-".into()),
                p.git.as_ref().map(|g| g.dirty).unwrap_or(0),
                p.duplicates.len(),
                p.nested_repos,
                p.suggested_group,
                p.empty
            );
        }
        for w in &list.warnings {
            println!("WARN {}: {}", w.kind, w.message);
        }
        println!("{} projects in {:?}", list.projects.len(), t.elapsed());
    }
}

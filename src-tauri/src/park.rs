//! Encrypted whole-tree parking with verified private-vault transport and fail-closed recovery.

use serde::{Deserialize, Serialize};
use std::path::Path;

use crate::commands::AppState;
use crate::scan::{self, Project, ProjectList};
use crate::util;
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::Read;
use std::path::PathBuf;
use std::sync::Mutex;

static OPERATION: Mutex<()> = Mutex::new(());
const CHUNK_SIZE: u64 = 32 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChunkManifest {
    version: u32,
    chunk_size: u64,
    ciphertext_size: u64,
    chunks: u32,
}

fn chunk_name(index: u32) -> String {
    format!("snapshot.{index:06}.age")
}

fn split_snapshot(dir: &Path, chunk_size: u64) -> Result<(), String> {
    let source = dir.join("snapshot.age");
    let ciphertext_size = fs::metadata(&source).map_err(err)?.len();
    let chunks = u32::try_from(ciphertext_size.div_ceil(chunk_size)).map_err(err)?;
    if chunks == 0 || chunks > 10_000 {
        return Err("Snapshot exceeds the 10,000 chunk transport limit".into());
    }
    let mut input = File::open(&source).map_err(err)?;
    for index in 0..chunks {
        let mut output = File::create(dir.join(chunk_name(index))).map_err(err)?;
        std::io::copy(&mut (&mut input).take(chunk_size), &mut output).map_err(err)?;
        output.sync_all().map_err(err)?;
    }
    let manifest = ChunkManifest {
        version: 1,
        chunk_size,
        ciphertext_size,
        chunks,
    };
    let file = File::create(dir.join("manifest.json")).map_err(err)?;
    serde_json::to_writer(&file, &manifest).map_err(err)?;
    file.sync_all().map_err(err)?;
    fs::remove_file(source).map_err(err)
}

struct RegistryLock(PathBuf);
impl RegistryLock {
    fn acquire(data: &Path) -> Result<Self, String> {
        fs::create_dir_all(data).map_err(err)?;
        let path = data.join("parking.lock");
        fs::OpenOptions::new().write(true).create_new(true).open(&path)
            .map_err(|e| format!("Parking is locked by another operation or interrupted session ({}): {e}. Only remove this lock after confirming no parking operation is running.", path.display()))?;
        Ok(Self(path))
    }
}
impl Drop for RegistryLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct ParkPreview {
    pub path: String,
    pub size_bytes: u64,
    pub file_count: u64,
    pub warnings: Vec<String>,
}

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

fn strong(pass: &str) -> Result<(), String> {
    let classes = [
        pass.chars().any(char::is_uppercase),
        pass.chars().any(char::is_lowercase),
        pass.chars().any(char::is_numeric),
        pass.chars().any(|c| !c.is_alphanumeric()),
    ];
    if pass.chars().count() < 16
        || (classes.into_iter().filter(|v| *v).count() < 3 && pass.split_whitespace().count() < 6)
    {
        return Err("Use a unique passphrase of at least 16 characters with three character classes, or six words. Losing it makes recovery impossible.".into());
    }
    Ok(())
}

// Reject unsupported objects rather than omit data. No ignore rules are consulted.
fn inventory(root: &Path) -> Result<(String, u64, u64), String> {
    fn walk(
        root: &Path,
        p: &Path,
        hash: &mut Sha256,
        size: &mut u64,
        count: &mut u64,
    ) -> Result<(), String> {
        use std::os::unix::ffi::OsStrExt;
        use std::os::unix::fs::MetadataExt;
        let m = fs::symlink_metadata(p).map_err(err)?;
        if m.file_type().is_symlink() {
            return Err(format!("Symlinks are not supported: {}", p.display()));
        }
        if !m.is_dir() && !m.is_file() {
            return Err(format!("Unsupported special file: {}", p.display()));
        }
        if m.is_file() && m.nlink() > 1 {
            return Err(format!(
                "Hard-linked files are not supported: {}",
                p.display()
            ));
        }
        if p.file_name().is_some_and(|n| n == ".git") && m.is_file() {
            return Err("Linked Git worktrees and submodule .git files are not supported".into());
        }
        if p.ends_with(".git/worktrees") || p.ends_with(".git/objects/info/alternates") {
            return Err(
                "Git worktrees and external object stores must be detached before parking".into(),
            );
        }
        let rel = p.strip_prefix(root).map_err(err)?;
        let bytes = rel.as_os_str().as_bytes();
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
        hash.update(m.mode().to_le_bytes());
        if m.is_file() {
            hash.update(m.len().to_le_bytes());
            let mut f = File::open(p).map_err(err)?;
            let mut buf = [0u8; 65536];
            loop {
                let n = f.read(&mut buf).map_err(err)?;
                if n == 0 {
                    break;
                }
                hash.update(&buf[..n]);
            }
            *size = size.checked_add(m.len()).ok_or("Directory size overflow")?;
            *count += 1;
        } else {
            let mut children = fs::read_dir(p)
                .map_err(err)?
                .map(|e| e.map(|v| v.path()))
                .collect::<Result<Vec<_>, _>>()
                .map_err(err)?;
            children.sort();
            for c in children {
                walk(root, &c, hash, size, count)?;
            }
        }
        Ok(())
    }
    let mut h = Sha256::new();
    let mut size = 0;
    let mut count = 0;
    walk(root, root, &mut h, &mut size, &mut count)?;
    Ok((format!("{:x}", h.finalize()), size, count))
}

pub fn preview(path: &Path) -> Result<ParkPreview, String> {
    if !path.is_absolute()
        || fs::symlink_metadata(path)
            .map_err(err)?
            .file_type()
            .is_symlink()
    {
        return Err("Choose an absolute, non-symlink project directory".into());
    }
    let canonical = fs::canonicalize(path).map_err(err)?;
    if canonical != path || !path.is_dir() || path.parent().is_none() || canonical == util::home() {
        return Err("Choose a canonical project directory, not a filesystem root or home".into());
    }
    let (_, size_bytes, file_count) = inventory(path)?;
    // tar does not preserve macOS resource forks/ACLs/xattrs. Refuse instead of losing them.
    #[cfg(target_os = "macos")]
    {
        let attrs = util::run("xattr", &["-r", &path.to_string_lossy()], None)?;
        if !attrs.trim().is_empty() {
            return Err("Extended attributes/resource forks are present. Parking cannot preserve them, so nothing was removed.".into());
        }
        let acl = util::run("ls", &["-laeR", &path.to_string_lossy()], None)?;
        if acl.lines().any(|l| {
            l.trim_start()
                .chars()
                .next()
                .is_some_and(|c| c.is_ascii_digit())
                && l.contains(": ")
        }) {
            return Err("ACLs are not supported by parking".into());
        }
    }
    Ok(ParkPreview { path: path.to_string_lossy().into_owned(), size_bytes, file_count,
        warnings: vec!["Quit editors, dev servers and all processes writing this project. Unsaved buffers are not included.".into(),
            "All files, including .git, ignored files and secrets, are encrypted. Keep your passphrase somewhere safe, it is never saved.".into(),
            "Symlinks, hard links, special files, linked worktrees and macOS extended attributes/ACLs are blocked. Ownership and creation times are not preserved. Snapshots use 32 MiB encrypted chunks and require substantial temporary disk space during remote verification.".into()] })
}

fn discovered_project(state: &AppState, path: &Path) -> Result<Project, String> {
    let canonical = fs::canonicalize(path).map_err(err)?;
    if canonical != path {
        return Err("Project path must be canonical and cannot contain symlinks".into());
    }
    scan::scan(&state.config()).projects.into_iter().find(|p| {
        fs::canonicalize(&p.path).is_ok_and(|candidate| candidate == canonical)
    }).ok_or_else(|| "Parking is only available for a discovered project, not a root/group/arbitrary directory".into())
}

pub fn preview_project(state: &AppState, path: &Path) -> Result<ParkPreview, String> {
    discovered_project(state, path)?;
    preview(path)
}

fn encrypt(root: &Path, output: &Path, pass: &str) -> Result<(), String> {
    let file = File::create(output).map_err(err)?;
    let enc =
        age::Encryptor::with_user_passphrase(age::secrecy::SecretString::from(pass.to_owned()));
    let writer = enc.wrap_output(file).map_err(err)?;
    let mut tar = tar::Builder::new(writer);
    tar.follow_symlinks(false);
    tar.append_dir_all("tree", root).map_err(err)?;
    let writer = tar.into_inner().map_err(err)?;
    let file = writer.finish().map_err(err)?;
    file.sync_all().map_err(err)
}

fn decrypt(input: &Path, staging: &Path, pass: &str) -> Result<PathBuf, String> {
    // Consume through authenticated EOF BEFORE parsing/extracting any plaintext.
    let d = age::Decryptor::new(File::open(input).map_err(err)?).map_err(err)?;
    let identity = age::scrypt::Identity::new(age::secrecy::SecretString::from(pass.to_owned()));
    let mut r = d
        .decrypt(std::iter::once(&identity as &dyn age::Identity))
        .map_err(err)?;
    let mut plain = tempfile::tempfile().map_err(err)?;
    std::io::copy(&mut r, &mut plain).map_err(err)?;
    use std::io::{Seek, SeekFrom};
    plain.seek(SeekFrom::Start(0)).map_err(err)?;
    let mut archive = tar::Archive::new(plain);
    archive.set_preserve_permissions(true);
    for entry in archive.entries().map_err(err)? {
        let mut entry = entry.map_err(err)?;
        let p = entry.path().map_err(err)?.into_owned();
        if !p.starts_with("tree")
            || p.components()
                .any(|c| !matches!(c, std::path::Component::Normal(_)))
            || !(entry.header().entry_type().is_file() || entry.header().entry_type().is_dir())
        {
            return Err("Unsafe or unsupported archive entry".into());
        }
        if !entry.unpack_in(staging).map_err(err)? {
            return Err("Archive entry escaped staging".into());
        }
    }
    let tree = staging.join("tree");
    inventory(&tree)?;
    Ok(tree)
}

fn git(dir: &Path, args: &[&str]) -> Result<String, String> {
    let output = std::process::Command::new("git")
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "core.symlinks=false",
            "-c",
            "protocol.file.allow=always",
        ])
        .args(args)
        .current_dir(dir)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .map_err(err)?;
    if !output.status.success() {
        return Err(format!(
            "Git transport failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn verify_vault_response(slug: &str, response: &str) -> Result<(), String> {
    let v: serde_json::Value = serde_json::from_str(response).map_err(err)?;
    if v["private"] != true
        || v["visibility"] != "private"
        || v["full_name"]
            .as_str()
            .is_none_or(|s| !s.eq_ignore_ascii_case(slug))
    {
        return Err(
            "Vault privacy could not be verified. Only a private GitHub repository is permitted."
                .into(),
        );
    }
    Ok(())
}

fn vault(slug: &str) -> Result<String, String> {
    let parts: Vec<_> = slug.split('/').collect();
    if parts.len() != 2
        || parts.iter().any(|p| {
            p.is_empty()
                || p.starts_with('-')
                || !p
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
        })
    {
        return Err("Configure the vault as owner/repository".into());
    }
    let response = util::run(
        "gh",
        &["api", "--hostname", "github.com", &format!("repos/{slug}")],
        None,
    )?;
    verify_vault_response(slug, &response)?;
    util::run(
        "gh",
        &["auth", "setup-git", "--hostname", "github.com"],
        None,
    )?;
    Ok(format!("https://github.com/{slug}.git"))
}

fn init(dir: &Path) -> Result<(), String> {
    git(dir, &["init", "--quiet"])?;
    Ok(())
}

fn upload(dir: &Path, url: &str, reference: &str) -> Result<String, String> {
    let manifest: ChunkManifest =
        serde_json::from_reader(File::open(dir.join("manifest.json")).map_err(err)?)
            .map_err(err)?;
    // Push bounded batches so multi-GB snapshots stay below GitHub's per-push pack limit.
    let names = (0..manifest.chunks).map(chunk_name).collect::<Vec<_>>();
    for batch in names.chunks(16) {
        let mut args = vec!["add", "--force", "--"];
        args.extend(batch.iter().map(String::as_str));
        git(dir, &args)?;
        snapshot_commit(dir)?;
        git(dir, &["push", url, &format!("HEAD:{reference}")])?;
    }
    git(dir, &["add", "--force", "--", "manifest.json"])?;
    snapshot_commit(dir)?;
    let commit = git(dir, &["rev-parse", "HEAD"])?;
    git(dir, &["push", url, &format!("HEAD:{reference}")])?;
    Ok(commit)
}

fn snapshot_commit(dir: &Path) -> Result<(), String> {
    git(
        dir,
        &[
            "-c",
            "user.name=Pelago",
            "-c",
            "user.email=pelago@localhost",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--quiet",
            "-m",
            "Encrypted snapshot",
        ],
    )?;
    Ok(())
}

fn download(dir: &Path, url: &str, reference: &str, commit: &str) -> Result<PathBuf, String> {
    if !reference.starts_with("refs/heads/pelago/")
        || ![40, 64].contains(&commit.len())
        || !commit.chars().all(|c| c.is_ascii_hexdigit())
    {
        return Err("Invalid snapshot reference or commit in registry".into());
    }
    git(dir, &["check-ref-format", reference])?;
    init(dir)?;
    git(dir, &["fetch", "--depth=1", url, reference])?;
    if git(dir, &["rev-parse", "FETCH_HEAD"])? != commit {
        return Err("Remote snapshot commit does not match registry".into());
    }
    git(dir, &["checkout", "FETCH_HEAD", "--", "manifest.json"])?;
    let metadata = fs::metadata(dir.join("manifest.json")).map_err(err)?;
    if metadata.len() > 4096 {
        return Err("Invalid oversized chunk manifest".into());
    }
    let manifest: ChunkManifest =
        serde_json::from_reader(File::open(dir.join("manifest.json")).map_err(err)?)
            .map_err(err)?;
    if manifest.version != 1
        || manifest.chunk_size == 0
        || manifest.chunk_size > CHUNK_SIZE
        || manifest.chunks == 0
        || manifest.chunks > 10_000
        || manifest.ciphertext_size.div_ceil(manifest.chunk_size) != u64::from(manifest.chunks)
    {
        return Err("Invalid encrypted chunk manifest".into());
    }
    let names = (0..manifest.chunks).map(chunk_name).collect::<Vec<_>>();
    for batch in names.chunks(128) {
        let mut args = vec!["checkout", "FETCH_HEAD", "--"];
        args.extend(batch.iter().map(String::as_str));
        git(dir, &args)?;
    }
    let output = dir.join("snapshot.age");
    let mut combined = File::create(&output).map_err(err)?;
    let mut total = 0u64;
    for (index, name) in names.iter().enumerate() {
        let file = dir.join(name);
        let length = fs::metadata(&file).map_err(err)?.len();
        let expected = if index + 1 == names.len() {
            manifest.ciphertext_size - total
        } else {
            manifest.chunk_size
        };
        if length != expected {
            return Err("Encrypted snapshot chunk has an invalid length".into());
        }
        total += std::io::copy(&mut File::open(file).map_err(err)?, &mut combined).map_err(err)?;
    }
    combined.sync_all().map_err(err)?;
    Ok(output)
}

pub fn park(state: &AppState, path: &Path, pass: &str) -> Result<ParkedProject, String> {
    let _lock = OPERATION.lock().map_err(err)?;
    let _disk_lock = RegistryLock::acquire(&state.data_dir)?;
    strong(pass)?;
    let p = preview_project(state, path)?;
    if fs::canonicalize(&state.data_dir)
        .map_err(err)?
        .starts_with(path)
    {
        return Err("Project contains Pelago's data directory".into());
    }
    let mut reg = load_registry(&state.data_dir)?;
    if reg.parked.iter().any(|r| r.path == p.path) {
        return Err("Project already has a parked snapshot".into());
    }
    let slug = state
        .config()
        .vault_repo
        .ok_or("Configure a private GitHub vault in Settings first")?;
    let url = vault(&slug)?;
    park_to(state, path, pass, &slug, &url, &mut reg)
}

fn park_to(
    state: &AppState,
    path: &Path,
    pass: &str,
    slug: &str,
    url: &str,
    reg: &mut Registry,
) -> Result<ParkedProject, String> {
    let source = discovered_project(state, path)?;
    let before = inventory(path)?;
    let tmp = tempfile::tempdir().map_err(err)?;
    if tmp.path().starts_with(path) {
        return Err("Snapshot staging is inside project".into());
    }
    init(tmp.path())?;
    encrypt(path, &tmp.path().join("snapshot.age"), pass)?;
    split_snapshot(tmp.path(), CHUNK_SIZE)?;
    let reference = format!(
        "refs/heads/pelago/{}",
        tmp.path()
            .file_name()
            .ok_or("Missing staging name")?
            .to_string_lossy()
            .trim_start_matches('.')
    );
    let commit = upload(tmp.path(), url, &reference)?;
    let fetched = tempfile::tempdir().map_err(err)?;
    let ciphertext = download(fetched.path(), url, &reference, &commit)?;
    let unpack = tempfile::tempdir().map_err(err)?;
    let tree = decrypt(&ciphertext, unpack.path(), pass)?;
    if inventory(&tree)? != before || inventory(path)? != before {
        return Err("Snapshot validation failed or project changed. Nothing was removed.".into());
    }
    if url.starts_with("https://github.com/") {
        vault(slug)?;
    }
    let record = ParkedProject {
        name: path
            .file_name()
            .ok_or("Missing project name")?
            .to_string_lossy()
            .into_owned(),
        path: path.to_string_lossy().into_owned(),
        group: source.group.clone(),
        mode: "vault".into(),
        snapshot_repo: slug.into(),
        snapshot_ref: reference,
        snapshot_commit: commit,
        origin_url: source.git.as_ref().and_then(|g| g.remote_url.clone()),
        branch: source.git.as_ref().and_then(|g| g.branch.clone()),
        head_commit: if path.join(".git").exists() {
            git(path, &["rev-parse", "HEAD"]).ok()
        } else {
            None
        },
        snapshot_digest: Some(before.0.clone()),
        stack: source.stack.clone(),
        size_bytes: before.1,
        parked_at: util::now_secs(),
        last_activity: source.last_activity,
    };
    reg.parked.push(record.clone());
    save_registry(&state.data_dir, reg)?; // Durably record recovery location BEFORE removing anything.
    // Rename into a same-filesystem private quarantine, then validate again. A failure retains all data.
    let quarantine = tempfile::Builder::new()
        .prefix(".pelago-parking-")
        .tempdir_in(path.parent().ok_or("Missing parent")?)
        .map_err(err)?;
    let quarantine = quarantine.keep();
    let moved = quarantine.join("tree");
    util::rename_noreplace(path, &moved).map_err(err)?;
    if inventory(&moved).map_err(|e| {
        format!(
            "Cannot validate quarantined project at {}. Data and registered snapshot retained: {e}",
            moved.display()
        )
    })? != before
    {
        // Never overwrite a concurrently recreated original path.
        if !path.try_exists().map_err(err)? {
            let _ = util::rename_noreplace(&moved, path);
        }
        return Err(format!(
            "Project changed during parking. Data retained at {}. Snapshot remains registered.",
            moved.display()
        ));
    }
    // Recheck unsupported metadata after the move too, so late xattrs/ACLs cannot be silently lost.
    preview(&moved).map_err(|e| {
        format!(
            "Project retained at {} because final metadata validation failed: {e}",
            moved.display()
        )
    })?;
    fs::remove_dir_all(&moved).map_err(|e| {
        format!(
            "Snapshot verified, but local cleanup failed at {}: {e}",
            moved.display()
        )
    })?;
    fs::remove_dir(&quarantine).map_err(err)?;
    Ok(record)
}

pub fn restore(state: &AppState, path: &Path, pass: &str) -> Result<(), String> {
    let _lock = OPERATION.lock().map_err(err)?;
    let _disk_lock = RegistryLock::acquire(&state.data_dir)?;
    let mut reg = load_registry(&state.data_dir)?;
    let index = reg
        .parked
        .iter()
        .position(|r| Path::new(&r.path) == path)
        .ok_or("No parked snapshot at this path")?;
    let record = reg.parked[index].clone();
    let url = vault(&record.snapshot_repo)?;
    restore_from(state, path, pass, &url, &record, &mut reg, index)
}

fn restore_from(
    state: &AppState,
    path: &Path,
    pass: &str,
    url: &str,
    record: &ParkedProject,
    reg: &mut Registry,
    index: usize,
) -> Result<(), String> {
    if !path.is_absolute() || fs::symlink_metadata(path).is_ok() {
        return Err(
            "Restore destination already exists or is not absolute. Nothing will be overwritten."
                .into(),
        );
    }
    let parent = path.parent().ok_or("Missing destination parent")?;
    if fs::canonicalize(parent).map_err(err)? != parent {
        return Err("Restore parent must exist and cannot contain symlinks".into());
    }
    let fetched = tempfile::tempdir().map_err(err)?;
    let ciphertext = download(
        fetched.path(),
        url,
        &record.snapshot_ref,
        &record.snapshot_commit,
    )?;
    let staging = tempfile::Builder::new()
        .prefix(".pelago-restore-")
        .tempdir_in(parent)
        .map_err(err)?;
    let tree = decrypt(&ciphertext, staging.path(), pass)?;
    if Some(inventory(&tree)?.0) != record.snapshot_digest {
        return Err("Restored tree integrity does not match registry".into());
    }
    let staging = staging.keep();
    util::rename_noreplace(&tree, path).map_err(|e| format!("Restore destination exists or cannot be moved. No overwrite performed. Staging retained at {}: {e}", staging.display()))?;
    if Some(inventory(path)?.0) != record.snapshot_digest {
        return Err("Restore validation failed. Snapshot and registry retained.".into());
    }
    reg.parked.remove(index);
    save_registry(&state.data_dir, reg)?;
    fs::remove_dir_all(staging).map_err(err)?;
    Ok(())
}

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
    /// SHA-256 of the complete path/content/mode inventory, not a Git commit.
    #[serde(default)]
    pub snapshot_digest: Option<String>,
    pub stack: Vec<String>,
    pub size_bytes: u64,
    pub parked_at: i64,
    pub last_activity: i64,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Registry {
    pub version: u32,
    pub parked: Vec<ParkedProject>,
}

pub fn registry_path(data_dir: &Path) -> std::path::PathBuf {
    data_dir.join("registry.json")
}

pub fn load_registry(data_dir: &Path) -> Result<Registry, String> {
    match std::fs::read_to_string(registry_path(data_dir)) {
        Ok(text) => {
            let reg: Registry = serde_json::from_str(&text)
                .map_err(|e| format!("Invalid parking registry: {e}"))?;
            if reg.version != 1 {
                return Err("Unsupported parking registry version".into());
            }
            Ok(reg)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Registry {
            version: 1,
            parked: vec![],
        }),
        Err(e) => Err(format!("Cannot read parking registry: {e}")),
    }
}

pub fn save_registry(data_dir: &Path, reg: &Registry) -> Result<(), String> {
    use std::io::Write;
    fs::create_dir_all(data_dir).map_err(err)?;
    let mut file = tempfile::NamedTempFile::new_in(data_dir).map_err(err)?;
    serde_json::to_writer_pretty(&mut file, reg).map_err(err)?;
    file.flush().map_err(err)?;
    file.as_file().sync_all().map_err(err)?;
    file.persist(registry_path(data_dir)).map_err(err)?;
    File::open(data_dir).map_err(err)?.sync_all().map_err(err)
}

/// Add parked projects (not present on disk) to a scan result.
pub fn merge_parked(state: &AppState, mut list: ProjectList) -> Result<ProjectList, String> {
    let reg = load_registry(&state.data_dir)?;
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
    Ok(list)
}

#[cfg(test)]
mod tests {
    use super::*;
    const PASS: &str = "correct horse battery staple private vault";

    #[test]
    fn private_vault_proof_fails_closed() {
        assert!(
            verify_vault_response(
                "owner/vault",
                r#"{"private":true,"visibility":"private","full_name":"owner/vault"}"#
            )
            .is_ok()
        );
        for response in [
            "not json",
            "{}",
            r#"{"private":false,"visibility":"public","full_name":"owner/vault"}"#,
            r#"{"private":true,"visibility":"internal","full_name":"owner/vault"}"#,
            r#"{"private":true,"visibility":"private","full_name":"other/vault"}"#,
        ] {
            assert!(verify_vault_response("owner/vault", response).is_err());
        }
    }

    #[test]
    fn encrypted_chunks_roundtrip_across_multiple_push_batches() {
        let (_temp, project, _state) = fixture();
        let staging = tempfile::tempdir().unwrap();
        init(staging.path()).unwrap();
        encrypt(&project, &staging.path().join("snapshot.age"), PASS).unwrap();
        split_snapshot(staging.path(), 1024).unwrap();
        let manifest: ChunkManifest =
            serde_json::from_reader(File::open(staging.path().join("manifest.json")).unwrap())
                .unwrap();
        assert!(manifest.chunks > 16);
        assert!(!staging.path().join("snapshot.age").exists());
        let bare = project.parent().unwrap().join("chunks.git");
        fs::create_dir(&bare).unwrap();
        git(&bare, &["init", "--bare", "--quiet"]).unwrap();
        let reference = "refs/heads/pelago/chunk-test";
        let commit = upload(staging.path(), bare.to_str().unwrap(), reference).unwrap();
        let fetched = tempfile::tempdir().unwrap();
        let ciphertext =
            download(fetched.path(), bare.to_str().unwrap(), reference, &commit).unwrap();
        let unpack = tempfile::tempdir().unwrap();
        let tree = decrypt(&ciphertext, unpack.path(), PASS).unwrap();
        assert_eq!(inventory(&tree).unwrap(), inventory(&project).unwrap());
        // A missing remote chunk must never yield a partial successful snapshot.
        fs::remove_file(staging.path().join(chunk_name(0))).unwrap();
        git(staging.path(), &["add", "--update"]).unwrap();
        snapshot_commit(staging.path()).unwrap();
        git(
            staging.path(),
            &["push", bare.to_str().unwrap(), &format!("HEAD:{reference}")],
        )
        .unwrap();
        let broken = git(staging.path(), &["rev-parse", "HEAD"]).unwrap();
        let next = tempfile::tempdir().unwrap();
        assert!(download(next.path(), bare.to_str().unwrap(), reference, &broken).is_err());
    }

    #[test]
    fn arbitrary_root_group_and_alias_paths_cannot_be_parked() {
        let (_temp, project, state) = fixture();
        assert!(preview_project(&state, &project).is_ok());
        assert!(preview_project(&state, project.parent().unwrap()).is_err());
        let arbitrary = project.parent().unwrap().join("empty-group");
        fs::create_dir(&arbitrary).unwrap();
        assert!(preview_project(&state, &arbitrary).is_err());
        let alias = project.parent().unwrap().join("alias");
        std::os::unix::fs::symlink(&project, &alias).unwrap();
        assert!(preview_project(&state, &alias).is_err());
    }

    fn fixture() -> (tempfile::TempDir, PathBuf, AppState) {
        let temp = tempfile::tempdir().unwrap();
        let base = fs::canonicalize(temp.path()).unwrap();
        let project = base.join("project");
        fs::create_dir(&project).unwrap();
        init(&project).unwrap();
        fs::write(project.join(".gitignore"), ".env\nignored/\n").unwrap();
        fs::write(project.join(".env"), "SECRET=never-in-plaintext-remote").unwrap();
        fs::create_dir_all(project.join("ignored/empty")).unwrap();
        fs::write(project.join("ignored/data"), [0u8, 1, 2, 255]).unwrap();
        fs::write(project.join("untracked.txt"), "not committed").unwrap();
        let state = AppState::new(base.join("appdata"));
        {
            let mut config = state.config.lock().unwrap();
            config.projects_root = base.to_string_lossy().into_owned();
            config.scan_paths.clear();
            config.groups.clear();
        }
        (temp, project, state)
    }

    #[test]
    fn local_bare_roundtrip_includes_git_ignored_and_untracked() {
        let (_temp, project, state) = fixture();
        let original = inventory(&project).unwrap();
        let bare = project.parent().unwrap().join("remote.git");
        fs::create_dir(&bare).unwrap();
        git(&bare, &["init", "--bare", "--quiet"]).unwrap();
        let url = bare.to_str().unwrap();
        let mut reg = Registry {
            version: 1,
            parked: vec![],
        };
        let rec = park_to(&state, &project, PASS, "local/test", url, &mut reg).unwrap();
        assert!(!project.exists());
        assert_eq!(load_registry(&state.data_dir).unwrap().parked.len(), 1);
        assert!(restore_from(&state, &project, "incorrect", url, &rec, &mut reg, 0).is_err());
        assert!(!project.exists());
        restore_from(&state, &project, PASS, url, &rec, &mut reg, 0).unwrap();
        assert_eq!(inventory(&project).unwrap(), original);
        assert!(project.join(".git/HEAD").is_file());
        assert!(project.join("ignored/empty").is_dir());
        assert_eq!(
            fs::read_to_string(project.join(".env")).unwrap(),
            "SECRET=never-in-plaintext-remote"
        );
        assert!(load_registry(&state.data_dir).unwrap().parked.is_empty());
        assert!(
            restore_from(&state, &project, PASS, url, &rec, &mut reg, 0)
                .unwrap_err()
                .contains("already exists")
        );
    }

    #[test]
    fn corrupted_registry_never_becomes_empty_registry() {
        let dir = tempfile::tempdir().unwrap();
        assert!(load_registry(dir.path()).unwrap().parked.is_empty());
        for content in [
            "garbage",
            "{}",
            "{\"version\":1}",
            "{\"version\":2,\"parked\":[]}",
        ] {
            fs::write(registry_path(dir.path()), content).unwrap();
            assert!(load_registry(dir.path()).is_err(), "{content}");
        }
        fs::remove_file(registry_path(dir.path())).unwrap();
        fs::create_dir(registry_path(dir.path())).unwrap();
        assert!(load_registry(dir.path()).is_err());
    }

    #[test]
    fn unsupported_links_and_worktrees_are_rejected() {
        let (_temp, project, _state) = fixture();
        std::os::unix::fs::symlink("/etc/passwd", project.join("escape")).unwrap();
        assert!(inventory(&project).unwrap_err().contains("Symlinks"));
        fs::remove_file(project.join("escape")).unwrap();
        fs::hard_link(project.join(".env"), project.join("hardlink")).unwrap();
        assert!(inventory(&project).unwrap_err().contains("Hard-linked"));
        fs::remove_file(project.join("hardlink")).unwrap();
        fs::create_dir(project.join(".git/worktrees")).unwrap();
        assert!(inventory(&project).unwrap_err().contains("worktrees"));
    }

    #[test]
    fn encryption_tampering_is_rejected_before_extraction() {
        let (_temp, project, _state) = fixture();
        let encrypted = project.parent().unwrap().join("cipher.age");
        encrypt(&project, &encrypted, PASS).unwrap();
        let mut bytes = fs::read(&encrypted).unwrap();
        let last = bytes.len() - 1;
        bytes[last] ^= 0x80;
        fs::write(&encrypted, bytes).unwrap();
        let out = tempfile::tempdir().unwrap();
        assert!(decrypt(&encrypted, out.path(), PASS).is_err());
        assert_eq!(fs::read_dir(out.path()).unwrap().count(), 0);
    }

    #[test]
    fn failed_upload_keeps_source_and_registry_unmodified() {
        let (_temp, project, state) = fixture();
        let before = inventory(&project).unwrap();
        let mut reg = Registry {
            version: 1,
            parked: vec![],
        };
        let missing = project.parent().unwrap().join("missing.git");
        assert!(
            park_to(
                &state,
                &project,
                PASS,
                "local/test",
                missing.to_str().unwrap(),
                &mut reg
            )
            .is_err()
        );
        assert_eq!(inventory(&project).unwrap(), before);
        assert!(load_registry(&state.data_dir).unwrap().parked.is_empty());
    }

    #[test]
    fn strong_passphrase_and_exclusive_lock() {
        assert!(strong("password").is_err());
        assert!(strong("aaaaaaaaaaaaaaaaaaaa").is_err());
        assert!(strong(PASS).is_ok());
        assert!(strong("StrongUnique!9284Value").is_ok());
        let temp = tempfile::tempdir().unwrap();
        let lock = RegistryLock::acquire(temp.path()).unwrap();
        assert!(RegistryLock::acquire(temp.path()).is_err());
        drop(lock);
        assert!(RegistryLock::acquire(temp.path()).is_ok());
    }

    #[test]
    fn registry_write_failure_never_removes_project() {
        let (_temp, project, state) = fixture();
        let before = inventory(&project).unwrap();
        fs::write(&state.data_dir, "not a directory").unwrap();
        let bare = project.parent().unwrap().join("remote.git");
        fs::create_dir(&bare).unwrap();
        git(&bare, &["init", "--bare", "--quiet"]).unwrap();
        let mut reg = Registry {
            version: 1,
            parked: vec![],
        };
        assert!(
            park_to(
                &state,
                &project,
                PASS,
                "local/test",
                bare.to_str().unwrap(),
                &mut reg
            )
            .is_err()
        );
        assert_eq!(inventory(&project).unwrap(), before);
    }

    #[test]
    fn remote_commit_mismatch_does_not_restore() {
        let (_temp, project, state) = fixture();
        let bare = project.parent().unwrap().join("remote.git");
        fs::create_dir(&bare).unwrap();
        git(&bare, &["init", "--bare", "--quiet"]).unwrap();
        let mut reg = Registry {
            version: 1,
            parked: vec![],
        };
        let mut rec = park_to(
            &state,
            &project,
            PASS,
            "local/test",
            bare.to_str().unwrap(),
            &mut reg,
        )
        .unwrap();
        rec.snapshot_commit = "0".repeat(40);
        assert!(
            restore_from(
                &state,
                &project,
                PASS,
                bare.to_str().unwrap(),
                &rec,
                &mut reg,
                0
            )
            .unwrap_err()
            .contains("commit does not match")
        );
        assert!(!project.exists());
        assert_eq!(load_registry(&state.data_dir).unwrap().parked.len(), 1);
    }
}

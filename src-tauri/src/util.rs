//! Small process / path helpers shared by every module.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// GUI apps launched from Finder get a minimal PATH. Make sure the usual
/// Homebrew / system locations are present so `git`, `gh`, `du` etc. resolve.
pub fn init_path() {
    let current = std::env::var("PATH").unwrap_or_default();
    let mut parts: Vec<String> = current
        .split(':')
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect();
    let mut extras = vec![
        "/opt/homebrew/bin".to_string(),
        "/usr/local/bin".to_string(),
        "/usr/bin".to_string(),
        "/bin".to_string(),
        "/usr/sbin".to_string(),
        "/sbin".to_string(),
    ];
    if let Some(home) = dirs::home_dir() {
        extras.push(home.join(".cargo/bin").to_string_lossy().into_owned());
        extras.push(home.join(".local/bin").to_string_lossy().into_owned());
    }
    for extra in extras {
        if !parts.contains(&extra) {
            parts.push(extra);
        }
    }
    // SAFETY: called once from `main` before any other threads exist.
    unsafe { std::env::set_var("PATH", parts.join(":")) };
}

/// Run a program and return trimmed stdout, or a readable error with stderr.
pub fn run(program: &str, args: &[&str], cwd: Option<&Path>) -> Result<String, String> {
    let mut cmd = Command::new(program);
    cmd.args(args);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    if program == "git" {
        cmd.env("GIT_TERMINAL_PROMPT", "0").env("LC_ALL", "C");
    }
    let out = cmd
        .output()
        .map_err(|e| format!("could not run `{program}`: {e}"))?;
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).trim_end().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&out.stdout).trim().to_string();
        let msg = if stderr.is_empty() { stdout } else { stderr };
        Err(format!("`{program} {}` failed: {msg}", args.join(" ")))
    }
}

/// True if `program` is on PATH.
pub fn which(program: &str) -> Option<PathBuf> {
    let path = std::env::var("PATH").ok()?;
    path.split(':')
        .map(|dir| Path::new(dir).join(program))
        .find(|candidate| candidate.is_file())
}

pub fn home() -> PathBuf {
    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
}

/// Expand a leading `~` to the home directory.
pub fn expand_tilde(p: &str) -> PathBuf {
    if p == "~" {
        home()
    } else if let Some(rest) = p.strip_prefix("~/") {
        home().join(rest)
    } else {
        PathBuf::from(p)
    }
}

/// Replace the home directory prefix with `~` for display.
pub fn display_path(p: &Path) -> String {
    let home = home();
    match p.strip_prefix(&home) {
        Ok(rest) if rest.as_os_str().is_empty() => "~".into(),
        Ok(rest) => format!("~/{}", rest.to_string_lossy()),
        Err(_) => p.to_string_lossy().into_owned(),
    }
}

pub fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

pub fn mtime_secs(p: &Path) -> i64 {
    std::fs::metadata(p)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Disk usage in bytes (via `du -sk`, which is far faster than walking in Rust).
pub fn dir_size(p: &Path) -> u64 {
    let Some(s) = p.to_str() else { return 0 };
    run("du", &["-sk", s], None)
        .ok()
        .and_then(|out| out.split_whitespace().next()?.parse::<u64>().ok())
        .map(|kb| kb * 1024)
        .unwrap_or(0)
}

/// Quote a string for safe use inside `sh -c`.
pub fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// Atomic rename which never replaces an existing destination, even an empty
/// directory or dangling symlink. Unsupported platforms fail closed.
pub fn rename_noreplace(from: &Path, to: &Path) -> std::io::Result<()> {
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        let from = CString::new(from.as_os_str().as_bytes())?;
        let to = CString::new(to.as_os_str().as_bytes())?;
        #[cfg(target_os = "macos")]
        unsafe extern "C" {
            fn renamex_np(
                from: *const std::ffi::c_char,
                to: *const std::ffi::c_char,
                flags: u32,
            ) -> std::ffi::c_int;
        }
        #[cfg(target_os = "linux")]
        unsafe extern "C" {
            fn renameat2(
                from_fd: std::ffi::c_int,
                from: *const std::ffi::c_char,
                to_fd: std::ffi::c_int,
                to: *const std::ffi::c_char,
                flags: u32,
            ) -> std::ffi::c_int;
        }
        // SAFETY: both C strings remain valid through this call. The exclusive
        // flags prevent overwriting any destination. There is no unsafe fallback.
        #[cfg(target_os = "macos")]
        let result = unsafe { renamex_np(from.as_ptr(), to.as_ptr(), 0x4) };
        #[cfg(target_os = "linux")]
        let result = unsafe { renameat2(-100, from.as_ptr(), -100, to.as_ptr(), 1) };
        if result == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = (from, to);
        Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "Exclusive rename is unavailable on this platform",
        ))
    }
}

/// Validate a single path component chosen by the user (project / group name).
pub fn validate_absolute_path(path: &Path) -> Result<(), String> {
    if !path.is_absolute()
        || path
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir))
    {
        return Err("Use an absolute path without parent traversal (..)".into());
    }
    Ok(())
}

/// Validate a single path component chosen by the user (project / group name).
pub fn validate_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Name can't be empty".into());
    }
    if name.len() > 100 {
        return Err("Name is too long".into());
    }
    if name.starts_with('.') {
        return Err("Name can't start with a dot".into());
    }
    if !name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '_' | '.'))
    {
        return Err(
            "Names may contain only ASCII letters, numbers, spaces, hyphens, underscores and dots"
                .into(),
        );
    }
    Ok(name.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exclusive_rename_preserves_existing_destinations() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source");
        let destination = tmp.path().join("destination");
        std::fs::create_dir(&source).unwrap();
        std::fs::write(source.join("keep"), "safe").unwrap();
        std::fs::create_dir(&destination).unwrap();
        assert!(rename_noreplace(&source, &destination).is_err());
        assert!(source.join("keep").exists());
        let fresh = tmp.path().join("fresh");
        rename_noreplace(&source, &fresh).unwrap();
        assert!(fresh.join("keep").exists());
    }

    #[cfg(unix)]
    #[test]
    fn exclusive_rename_preserves_dangling_symlink() {
        let tmp = tempfile::tempdir().unwrap();
        let source = tmp.path().join("source");
        let destination = tmp.path().join("destination");
        std::fs::create_dir(&source).unwrap();
        std::os::unix::fs::symlink(tmp.path().join("missing"), &destination).unwrap();
        assert!(rename_noreplace(&source, &destination).is_err());
        assert!(source.is_dir());
        assert!(
            std::fs::symlink_metadata(destination)
                .unwrap()
                .file_type()
                .is_symlink()
        );
    }

    #[test]
    fn tilde_roundtrip() {
        let p = expand_tilde("~/Code/x");
        assert!(p.is_absolute());
        assert_eq!(display_path(&p), "~/Code/x");
        assert_eq!(display_path(&home()), "~");
    }

    #[test]
    fn quoting() {
        assert_eq!(shell_quote("a b"), "'a b'");
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
    }

    #[test]
    fn names() {
        assert!(validate_name("  my app ").is_ok());
        assert!(validate_name("").is_err());
        assert!(validate_name("a/b").is_err());
        assert!(validate_name(".hidden").is_err());
    }
}

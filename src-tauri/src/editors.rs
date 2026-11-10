//! Editor / terminal detection and launching.
//!
//! Apps are found by bundle id through Spotlight, so renamed or relocated
//! apps (e.g. `Visual Studio Code-3.app`) still resolve. Launching uses
//! `open -a <app> <path>`, which needs no per-editor CLI shim.

use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::config::CustomEditor;
use crate::util;

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Editor {
    pub id: String,
    pub name: String,
    pub kind: String, // "editor" | "terminal" | "custom"
    pub installed: bool,
    pub app_path: Option<String>,
}

struct Known {
    id: &'static str,
    name: &'static str,
    kind: &'static str,
    bundle_ids: &'static [&'static str],
    app_names: &'static [&'static str],
}

const KNOWN: &[Known] = &[
    Known {
        id: "vscode",
        name: "VS Code",
        kind: "editor",
        bundle_ids: &["com.microsoft.VSCode"],
        app_names: &["Visual Studio Code.app"],
    },
    Known {
        id: "vscode-insiders",
        name: "VS Code Insiders",
        kind: "editor",
        bundle_ids: &["com.microsoft.VSCodeInsiders"],
        app_names: &["Visual Studio Code - Insiders.app"],
    },
    Known {
        id: "cursor",
        name: "Cursor",
        kind: "editor",
        bundle_ids: &["com.todesktop.230313mzl4w4u92"],
        app_names: &["Cursor.app"],
    },
    Known {
        id: "windsurf",
        name: "Windsurf",
        kind: "editor",
        bundle_ids: &["com.exafunction.windsurf"],
        app_names: &["Windsurf.app"],
    },
    Known {
        id: "zed",
        name: "Zed",
        kind: "editor",
        bundle_ids: &["dev.zed.Zed"],
        app_names: &["Zed.app"],
    },
    Known {
        id: "sublime",
        name: "Sublime Text",
        kind: "editor",
        bundle_ids: &["com.sublimetext.4", "com.sublimetext.3"],
        app_names: &["Sublime Text.app"],
    },
    Known {
        id: "xcode",
        name: "Xcode",
        kind: "editor",
        bundle_ids: &["com.apple.dt.Xcode"],
        app_names: &["Xcode.app"],
    },
    Known {
        id: "intellij",
        name: "IntelliJ IDEA",
        kind: "editor",
        bundle_ids: &["com.jetbrains.intellij", "com.jetbrains.intellij.ce"],
        app_names: &["IntelliJ IDEA.app", "IntelliJ IDEA CE.app"],
    },
    Known {
        id: "pycharm",
        name: "PyCharm",
        kind: "editor",
        bundle_ids: &["com.jetbrains.pycharm", "com.jetbrains.pycharm.ce"],
        app_names: &["PyCharm.app", "PyCharm CE.app"],
    },
    Known {
        id: "webstorm",
        name: "WebStorm",
        kind: "editor",
        bundle_ids: &["com.jetbrains.WebStorm"],
        app_names: &["WebStorm.app"],
    },
    Known {
        id: "clion",
        name: "CLion",
        kind: "editor",
        bundle_ids: &["com.jetbrains.CLion"],
        app_names: &["CLion.app"],
    },
    Known {
        id: "goland",
        name: "GoLand",
        kind: "editor",
        bundle_ids: &["com.jetbrains.goland"],
        app_names: &["GoLand.app"],
    },
    Known {
        id: "rustrover",
        name: "RustRover",
        kind: "editor",
        bundle_ids: &["com.jetbrains.rustrover"],
        app_names: &["RustRover.app"],
    },
    Known {
        id: "android-studio",
        name: "Android Studio",
        kind: "editor",
        bundle_ids: &["com.google.android.studio"],
        app_names: &["Android Studio.app"],
    },
    Known {
        id: "terminal",
        name: "Terminal",
        kind: "terminal",
        bundle_ids: &["com.apple.Terminal"],
        app_names: &["Utilities/Terminal.app"],
    },
    Known {
        id: "iterm",
        name: "iTerm",
        kind: "terminal",
        bundle_ids: &["com.googlecode.iterm2"],
        app_names: &["iTerm.app"],
    },
    Known {
        id: "warp",
        name: "Warp",
        kind: "terminal",
        bundle_ids: &["dev.warp.Warp-Stable"],
        app_names: &["Warp.app"],
    },
    Known {
        id: "ghostty",
        name: "Ghostty",
        kind: "terminal",
        bundle_ids: &["com.mitchellh.ghostty"],
        app_names: &["Ghostty.app"],
    },
    Known {
        id: "finder",
        name: "Finder",
        kind: "terminal",
        bundle_ids: &["com.apple.finder"],
        app_names: &[],
    },
];

fn locate(k: &Known) -> Option<PathBuf> {
    if k.id == "finder" {
        return Some(PathBuf::from("/System/Library/CoreServices/Finder.app"));
    }
    for bid in k.bundle_ids {
        if let Ok(out) = util::run(
            "mdfind",
            &[&format!("kMDItemCFBundleIdentifier == '{bid}'")],
            None,
        ) && let Some(first) = out
            .lines()
            .find(|l| l.ends_with(".app") && !l.contains("/Trash/"))
        {
            return Some(PathBuf::from(first));
        }
    }
    let mut roots = vec![
        PathBuf::from("/Applications"),
        PathBuf::from("/System/Applications"),
        util::home().join("Applications"),
    ];
    roots.push(util::home().join("Applications/JetBrains Toolbox"));
    for root in roots {
        for name in k.app_names {
            let p = root.join(name);
            if p.exists() {
                return Some(p);
            }
        }
    }
    None
}

pub fn detect(custom: &[CustomEditor]) -> Vec<Editor> {
    use rayon::prelude::*;
    let mut list: Vec<Editor> = KNOWN
        .par_iter()
        .map(|k| {
            let app = locate(k);
            Editor {
                id: k.id.into(),
                name: k.name.into(),
                kind: k.kind.into(),
                installed: app.is_some(),
                app_path: app.map(|p| p.to_string_lossy().into_owned()),
            }
        })
        .collect();
    list.extend(custom.iter().map(|c| Editor {
        id: c.id.clone(),
        name: c.name.clone(),
        kind: "custom".into(),
        installed: true,
        app_path: None,
    }));
    list
}

/// Build the command that opens `path` in the given editor.
pub fn launch_command(
    editor: &Editor,
    custom: &[CustomEditor],
    path: &Path,
) -> Result<Command, String> {
    let path_str = path.to_string_lossy();
    if editor.kind == "custom" {
        let c = custom
            .iter()
            .find(|c| c.id == editor.id)
            .ok_or("custom editor not found")?;
        let script = if c.command.contains("{path}") {
            c.command.replace("{path}", &util::shell_quote(&path_str))
        } else {
            format!("{} {}", c.command, util::shell_quote(&path_str))
        };
        let mut cmd = Command::new("/bin/sh");
        cmd.arg("-c").arg(script);
        return Ok(cmd);
    }
    let app = editor
        .app_path
        .as_ref()
        .ok_or_else(|| format!("{} doesn't seem to be installed", editor.name))?;
    let mut cmd = Command::new("open");
    cmd.arg("-a").arg(app).arg(path.as_os_str());
    Ok(cmd)
}

pub fn open(editor: &Editor, custom: &[CustomEditor], path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err(format!(
            "{} doesn't exist on disk",
            util::display_path(path)
        ));
    }
    let mut cmd = launch_command(editor, custom, path)?;
    let out = cmd.output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(format!(
            "Couldn't open in {}: {}",
            editor.name,
            String::from_utf8_lossy(&out.stderr).trim()
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn custom_command_quotes_path() {
        let custom = vec![CustomEditor {
            id: "custom-nvim".into(),
            name: "Neovim".into(),
            command: "echo {path}".into(),
        }];
        let ed = Editor {
            id: "custom-nvim".into(),
            name: "Neovim".into(),
            kind: "custom".into(),
            installed: true,
            app_path: None,
        };
        let cmd = launch_command(&ed, &custom, Path::new("/tmp/it's here")).unwrap();
        let args: Vec<_> = cmd
            .get_args()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        assert_eq!(args[1], r"echo '/tmp/it'\''s here'");
    }

    #[test]
    fn known_ids_unique() {
        let mut ids: Vec<_> = KNOWN.iter().map(|k| k.id).collect();
        ids.sort();
        ids.dedup();
        assert_eq!(ids.len(), KNOWN.len());
    }
}

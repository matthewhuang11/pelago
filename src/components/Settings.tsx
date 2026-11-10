import { useEffect, useState } from "react";
import { api, errorText, pickFolder } from "../api";
import type { Config, Editor, GhStatus } from "../types";
import { useToast } from "./Toast";
import { Spinner } from "./ui";

export default function Settings({
  config,
  editors,
  onChanged,
  onRefreshEditors,
}: {
  config: Config;
  editors: Editor[];
  onChanged: (c: Config) => void;
  onRefreshEditors: () => void;
}) {
  const toast = useToast();
  const [gh, setGh] = useState<GhStatus | null>(null);
  const [checking, setChecking] = useState(false);
  const [custom, setCustom] = useState({ name: "", command: "" });
  const [stale, setStale] = useState(String(config.staleDays));

  const checkGh = async () => {
    setChecking(true);
    try {
      setGh(await api.ghStatus());
    } finally {
      setChecking(false);
    }
  };
  useEffect(() => {
    checkGh();
  }, []);

  const save = async (patch: Parameters<typeof api.saveSettings>[0], msg = "Saved") => {
    try {
      onChanged(await api.saveSettings(patch));
      toast.show(msg, "success");
    } catch (e) {
      toast.error(e);
    }
  };

  const installed = editors.filter((e) => e.installed);

  return (
    <div className="main-inner" style={{ maxWidth: 760 }}>
      <div className="page-head">
        <span className="emoji">⚙️</span>
        <h1>Settings</h1>
      </div>

      <div className="section">
        <h3 style={{ marginBottom: 12 }}>📂 Folders</h3>
        <div className="card col" style={{ gap: 16 }}>
          <div className="field">
            <label>Projects folder</label>
            <div className="row">
              <div className="input mono ellipsis" style={{ background: "#fff" }}>
                {config.projectsRoot}
              </div>
              <button
                className="btn ghost"
                onClick={async () => {
                  const p = await pickFolder("Choose your projects folder", config.projectsRoot);
                  if (p) save({ projectsRoot: p }, "Projects folder updated");
                }}
              >
                Change
              </button>
            </div>
            <div className="hint">Groups are the folders directly inside this one. Changing it doesn't move anything.</div>
          </div>
          <div className="field">
            <label>Also scan these folders</label>
            {config.scanPaths.length === 0 && <div className="hint">None. Only the projects folder is shown.</div>}
            {config.scanPaths.map((p) => (
              <div key={p} className="row">
                <div className="input mono ellipsis grow" style={{ background: "#fff" }}>
                  {p}
                </div>
                <button
                  className="icon-btn"
                  title="Stop scanning"
                  onClick={() => save({ scanPaths: config.scanPaths.filter((x) => x !== p) }, "Removed")}
                >
                  ✕
                </button>
              </div>
            ))}
            <div>
              <button
                className="btn ghost small"
                onClick={async () => {
                  const p = await pickFolder("Choose a folder that contains projects");
                  if (p && !config.scanPaths.includes(p)) save({ scanPaths: [...config.scanPaths, p] }, "Folder added");
                }}
              >
                + Add folder
              </button>
            </div>
            <div className="hint">Projects found here appear under Unsorted until you move them into a group.</div>
          </div>
        </div>
      </div>

      <div className="section">
        <div className="row" style={{ marginBottom: 12 }}>
          <h3>🧑‍💻 Editors</h3>
          <span className="spacer" />
          <button className="btn ghost small" onClick={onRefreshEditors}>
            Re-detect
          </button>
        </div>
        <div className="card col" style={{ gap: 16 }}>
          <div className="field">
            <label>Default editor</label>
            <select
              className="input"
              value={config.defaultEditor}
              onChange={(e) => save({ defaultEditor: e.target.value }, "Default editor updated")}
            >
              {installed.map((e) => (
                <option key={e.id} value={e.id}>
                  {e.name}
                </option>
              ))}
            </select>
            <div className="hint">
              Detected: {installed.filter((e) => e.kind !== "custom").map((e) => e.name).join(", ") || "none"}
            </div>
          </div>
          <div className="field">
            <label>Custom editors</label>
            {config.customEditors.map((c) => (
              <div key={c.id} className="row">
                <b style={{ width: 120 }} className="ellipsis">
                  {c.name}
                </b>
                <code className="grow ellipsis">{c.command}</code>
                <button
                  className="icon-btn"
                  onClick={() =>
                    save({ customEditors: config.customEditors.filter((x) => x.id !== c.id) }, "Removed").then(onRefreshEditors)
                  }
                >
                  ✕
                </button>
              </div>
            ))}
            <div className="row">
              <input
                className="input"
                style={{ width: 160 }}
                placeholder="Name (e.g. Neovim)"
                value={custom.name}
                onChange={(e) => setCustom({ ...custom, name: e.target.value })}
              />
              <input
                className="input mono grow"
                placeholder={`open -a iTerm {path}`}
                value={custom.command}
                onChange={(e) => setCustom({ ...custom, command: e.target.value })}
              />
              <button
                className="btn ghost"
                disabled={!custom.name.trim() || !custom.command.trim()}
                onClick={async () => {
                  const id = "custom-" + custom.name.trim().toLowerCase().replace(/[^a-z0-9]+/g, "-");
                  await save(
                    {
                      customEditors: [
                        ...config.customEditors.filter((x) => x.id !== id),
                        { id, name: custom.name.trim(), command: custom.command.trim() },
                      ],
                    },
                    "Custom editor added",
                  );
                  setCustom({ name: "", command: "" });
                  onRefreshEditors();
                }}
              >
                Add
              </button>
            </div>
            <div className="hint">
              Runs in a shell. <code>{"{path}"}</code> is replaced with the quoted project path (appended if omitted).
            </div>
          </div>
        </div>
      </div>

      <div className="section">
        <h3 style={{ marginBottom: 12 }}>🐙 GitHub</h3>
        <div className="card col" style={{ gap: 14 }}>
          {gh === null ? (
            <Spinner dark />
          ) : gh.loggedIn ? (
            <div className="row">
              <span style={{ fontSize: 22 }}>✅</span>
              <div className="grow">
                <b>Signed in as @{gh.user}</b>
                <div className="muted" style={{ fontSize: 13 }}>
                  via GitHub CLI. Run <code>gh auth logout</code> in a terminal to sign out.
                </div>
              </div>
              <button className="btn ghost small" onClick={checkGh} disabled={checking}>
                Refresh
              </button>
            </div>
          ) : (
            <div className="row">
              <span style={{ fontSize: 22 }}>🔌</span>
              <div className="grow">
                <b>{gh.installed ? "Not signed in" : "GitHub CLI not installed"}</b>
                <div className="muted" style={{ fontSize: 13 }}>
                  Needed to create repos and park projects.
                </div>
              </div>
              <button className="btn small" onClick={() => api.ghLogin().catch((e) => toast.error(errorText(e)))}>
                {gh.installed ? "Sign in" : "Install & sign in"}
              </button>
              <button className="btn ghost small" onClick={checkGh} disabled={checking}>
                Check
              </button>
            </div>
          )}
          <div className="field">
            <label>Private vault repo</label>
            <input
              className="input mono"
              placeholder={gh?.user ? `${gh.user}/pelago-vault` : "you/pelago-vault"}
              defaultValue={config.vaultRepo ?? ""}
              onBlur={(e) => {
                const v = e.target.value.trim();
                if (v !== (config.vaultRepo ?? "")) save({ vaultRepo: v || null }, "Vault updated");
              }}
            />
            <div className="hint">
              Used when parking public or repo-less projects so work-in-progress never goes public. Created automatically
              (private) the first time it's needed.
            </div>
          </div>
        </div>
      </div>

      <div className="section">
        <h3 style={{ marginBottom: 12 }}>☁️ Parking</h3>
        <div className="card">
          <div className="field">
            <label>Suggest parking projects untouched for</label>
            <div className="row">
              <input
                className="input"
                style={{ width: 100 }}
                type="number"
                min={0}
                value={stale}
                onChange={(e) => setStale(e.target.value)}
                onBlur={() => {
                  const n = Math.max(0, parseInt(stale || "0", 10) || 0);
                  if (n !== config.staleDays) save({ staleDays: n }, "Updated");
                }}
              />
              <span className="muted">days (0 turns suggestions off)</span>
            </div>
          </div>
        </div>
      </div>

      <div className="section">
        <h3 style={{ marginBottom: 12 }}>🔒 Your data</h3>
        <div className="card muted" style={{ fontSize: 14 }}>
          Pelago has no account and no server. Settings live in a JSON file in{" "}
          <code>~/Library/Application Support/dev.pelago.app/</code>. Delete it to start fresh.
        </div>
      </div>
    </div>
  );
}

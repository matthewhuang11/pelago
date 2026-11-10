import { useEffect, useState } from "react";
import { api, errorText, pickFolder } from "../api";
import type { Editor, FolderSuggestion, GhStatus, GroupMeta } from "../types";
import { Checkbox, Spinner } from "./ui";
import { classNames } from "../util";

const STARTER_GROUPS: GroupMeta[] = [
  { name: "Startups", emoji: "🚀", color: "#ff9600" },
  { name: "Hackathons", emoji: "⚡", color: "#ffc800" },
  { name: "Experiments", emoji: "🧪", color: "#ce82ff" },
  { name: "School", emoji: "🎓", color: "#58cc02" },
  { name: "Websites", emoji: "🌐", color: "#1c9af0" },
  { name: "Archive", emoji: "📦", color: "#afafaf" },
];

const STEPS = ["welcome", "root", "groups", "scan", "editor", "github"] as const;
type Step = (typeof STEPS)[number];

export default function Onboarding({ defaultRoot, onDone }: { defaultRoot: string; onDone: () => void }) {
  const [step, setStep] = useState<Step>("welcome");
  const [root, setRoot] = useState(defaultRoot);
  const [groups, setGroups] = useState<GroupMeta[]>(STARTER_GROUPS);
  const [enabledGroups, setEnabledGroups] = useState<Set<string>>(new Set(STARTER_GROUPS.map((g) => g.name)));
  const [newGroup, setNewGroup] = useState("");
  const [suggestions, setSuggestions] = useState<FolderSuggestion[] | null>(null);
  const [scanPaths, setScanPaths] = useState<string[]>([]);
  const [editors, setEditors] = useState<Editor[] | null>(null);
  const [editor, setEditor] = useState("vscode");
  const [gh, setGh] = useState<GhStatus | null>(null);
  const [checkingGh, setCheckingGh] = useState(false);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.suggestScanFolders().then(setSuggestions).catch(() => setSuggestions([]));
    api
      .listEditors()
      .then((list) => {
        setEditors(list);
        const firstInstalled = list.find((e) => e.installed && e.kind === "editor");
        if (firstInstalled) setEditor(firstInstalled.id);
      })
      .catch(() => setEditors([]));
    api.ghStatus().then(setGh).catch(() => {});
  }, []);

  const idx = STEPS.indexOf(step);
  const progress = ((idx + 1) / STEPS.length) * 100;
  const next = () => setStep(STEPS[Math.min(idx + 1, STEPS.length - 1)]);
  const back = () => setStep(STEPS[Math.max(idx - 1, 0)]);

  const refreshGh = async () => {
    setCheckingGh(true);
    try {
      setGh(await api.ghStatus());
    } finally {
      setCheckingGh(false);
    }
  };

  const finish = async () => {
    setSaving(true);
    setError(null);
    try {
      await api.completeOnboarding({
        projectsRoot: root,
        scanPaths,
        defaultEditor: editor,
        groups: groups.filter((g) => enabledGroups.has(g.name)),
      });
      onDone();
    } catch (e) {
      setError(errorText(e));
      setSaving(false);
    }
  };

  const toggleSet = (set: Set<string>, value: string) => {
    const s = new Set(set);
    if (s.has(value)) s.delete(value);
    else s.add(value);
    return s;
  };

  const installedEditors = (editors ?? []).filter((e) => e.installed);

  return (
    <div className="onboard">
      <div className="titlebar" data-tauri-drag-region />
      <div className="onboard-top">
        {idx > 0 ? (
          <button className="icon-btn" onClick={back} aria-label="Back">
            ←
          </button>
        ) : (
          <span style={{ width: 34 }} />
        )}
        <div className="progress grow">
          <div style={{ width: `${progress}%` }} />
        </div>
      </div>

      <div className="onboard-body">
        {step === "welcome" && (
          <>
            <img src="/pelago.png" width={110} height={110} style={{ margin: "0 auto" }} alt="" />
            <h1>Welcome to Pelago</h1>
            <p>Every project, one archipelago. Let's set things up. It takes about a minute.</p>
            <div className="grid tight" style={{ marginTop: 10 }}>
              {[
                ["🗂️", "Organize", "Groups are real folders on disk"],
                ["⚡", "Jump in", "One click opens your editor"],
                ["✨", "Create", "New repos with optional toggles"],
                ["☁️", "Park", "Free disk space without losing your place"],
              ].map(([e, t, d]) => (
                <div className="card" key={t} style={{ textAlign: "center" }}>
                  <div style={{ fontSize: 30 }}>{e}</div>
                  <div style={{ fontWeight: 800, marginTop: 6 }}>{t}</div>
                  <div className="muted" style={{ fontSize: 13 }}>
                    {d}
                  </div>
                </div>
              ))}
            </div>
            <div className="notice info" style={{ marginTop: 8 }}>
              <span className="ico">🔒</span>
              <span>
                No account, no server. Everything stays on this Mac. GitHub features use your own <code>gh</code>{" "}
                login.
              </span>
            </div>
          </>
        )}

        {step === "root" && (
          <>
            <div className="hero-art">🏝️</div>
            <h1>Where should your projects live?</h1>
            <p>Pelago keeps projects in one folder, sorted into group folders inside it.</p>
            <div className="field">
              <label>Projects folder</label>
              <div className="row">
                <input className="input mono" value={root} onChange={(e) => setRoot(e.target.value)} />
                <button
                  className="btn ghost"
                  onClick={async () => {
                    const p = await pickFolder("Choose your projects folder");
                    if (p) setRoot(p);
                  }}
                >
                  Browse
                </button>
              </div>
              <div className="hint">
                It'll be created if it doesn't exist. Layout: <code>{root.replace(/\/$/, "")}/&lt;Group&gt;/&lt;project&gt;</code>
              </div>
            </div>
          </>
        )}

        {step === "groups" && (
          <>
            <h1>Pick your starter groups</h1>
            <p>Each group is a folder. You can rename, recolor, or add more any time.</p>
            <div className="grid tight">
              {groups.map((g) => {
                const on = enabledGroups.has(g.name);
                return (
                  <div
                    key={g.name}
                    className={classNames("card clickable", on && "selected")}
                    onClick={() => setEnabledGroups(toggleSet(enabledGroups, g.name))}
                  >
                    <div className="row">
                      <span style={{ fontSize: 26 }}>{g.emoji}</span>
                      <span style={{ fontWeight: 800 }} className="grow">
                        {g.name}
                      </span>
                      <Checkbox on={on} />
                    </div>
                  </div>
                );
              })}
            </div>
            <div className="row">
              <input
                className="input"
                placeholder="Add another group…"
                value={newGroup}
                onChange={(e) => setNewGroup(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter" && newGroup.trim()) {
                    const name = newGroup.trim();
                    if (!groups.some((g) => g.name.toLowerCase() === name.toLowerCase())) {
                      setGroups([...groups, { name, emoji: "📁", color: "#1c9af0" }]);
                      setEnabledGroups(new Set([...enabledGroups, name]));
                    }
                    setNewGroup("");
                  }
                }}
              />
              <button
                className="btn ghost"
                disabled={!newGroup.trim()}
                onClick={() => {
                  const name = newGroup.trim();
                  if (!groups.some((g) => g.name.toLowerCase() === name.toLowerCase())) {
                    setGroups([...groups, { name, emoji: "📁", color: "#1c9af0" }]);
                    setEnabledGroups(new Set([...enabledGroups, name]));
                  }
                  setNewGroup("");
                }}
              >
                Add
              </button>
            </div>
          </>
        )}

        {step === "scan" && (
          <>
            <h1>Already have projects?</h1>
            <p>
              Pick folders to scan. Nothing gets moved. They'll show up under <b>Unsorted</b> until you sort them.
            </p>
            {suggestions === null ? (
              <div className="row" style={{ justifyContent: "center", padding: 20 }}>
                <Spinner dark /> <span className="muted">Looking around your home folder…</span>
              </div>
            ) : (
              <div className="col" style={{ gap: 10 }}>
                {suggestions.map((s) => {
                  const on = scanPaths.includes(s.path);
                  return (
                    <div
                      key={s.path}
                      className={classNames("card clickable", on && "selected")}
                      style={{ padding: 14 }}
                      onClick={() => setScanPaths(on ? scanPaths.filter((p) => p !== s.path) : [...scanPaths, s.path])}
                    >
                      <div className="check-row">
                        <Checkbox on={on} />
                        <div className="grow">
                          <div style={{ fontWeight: 800 }}>{s.displayPath}</div>
                          <div className="muted ellipsis" style={{ fontSize: 13 }}>
                            {s.projectCount} projects · {s.examples.join(", ")}
                            {s.projectCount > s.examples.length ? "…" : ""}
                          </div>
                        </div>
                      </div>
                    </div>
                  );
                })}
                {scanPaths
                  .filter((p) => !suggestions.some((s) => s.path === p))
                  .map((p) => (
                    <div key={p} className="card selected" style={{ padding: 14 }}>
                      <div className="check-row">
                        <Checkbox on />
                        <div className="grow mono">{p}</div>
                        <button className="icon-btn" onClick={() => setScanPaths(scanPaths.filter((x) => x !== p))}>
                          ✕
                        </button>
                      </div>
                    </div>
                  ))}
                <button
                  className="btn ghost"
                  onClick={async () => {
                    const p = await pickFolder("Choose a folder that contains projects");
                    if (p && !scanPaths.includes(p)) setScanPaths([...scanPaths, p]);
                  }}
                >
                  + Add another folder
                </button>
              </div>
            )}
          </>
        )}

        {step === "editor" && (
          <>
            <h1>What do you code in?</h1>
            <p>This is the default. You can choose a different editor per project too.</p>
            {editors === null ? (
              <div className="row" style={{ justifyContent: "center" }}>
                <Spinner dark />
              </div>
            ) : installedEditors.length === 0 ? (
              <div className="notice warn">
                <span className="ico">🤔</span>
                <span>No editors detected. Pelago will fall back to Finder; you can add a custom command in Settings.</span>
              </div>
            ) : (
              <div className="choices">
                {installedEditors.map((e) => (
                  <button
                    key={e.id}
                    className={classNames("choice", editor === e.id && "on")}
                    onClick={() => setEditor(e.id)}
                  >
                    <span className="e">{e.kind === "terminal" ? "⌨️" : "🧑‍💻"}</span>
                    <span className="t">{e.name}</span>
                    <span className="d">{e.kind === "terminal" ? "Terminal / Finder" : "Editor"}</span>
                  </button>
                ))}
              </div>
            )}
          </>
        )}

        {step === "github" && (
          <>
            <div className="hero-art">🐙</div>
            <h1>Connect GitHub (optional)</h1>
            <p>Needed for creating repos and parking projects. Pelago uses the official GitHub CLI, so your login stays with you.</p>
            <div className="card">
              <div className="col" style={{ gap: 12 }}>
                <div className="row">
                  <span>{gh?.gitInstalled ? "✅" : "❌"}</span>
                  <b>git</b>
                  <span className="muted">{gh?.gitInstalled ? "installed" : "missing: run xcode-select --install"}</span>
                </div>
                <div className="row">
                  <span>{gh?.installed ? "✅" : "⬜"}</span>
                  <b>GitHub CLI</b>
                  <span className="muted">{gh?.installed ? "installed" : gh?.brewInstalled ? "not installed yet" : "install from cli.github.com"}</span>
                </div>
                <div className="row">
                  <span>{gh?.loggedIn ? "✅" : "⬜"}</span>
                  <b>Signed in</b>
                  <span className="muted">{gh?.loggedIn ? `as @${gh.user}` : "not yet"}</span>
                </div>
              </div>
              {!gh?.loggedIn && (
                <div className="row" style={{ marginTop: 16 }}>
                  <button className="btn" onClick={() => api.ghLogin().catch((e) => setError(errorText(e)))}>
                    {gh?.installed ? "Sign in with GitHub" : "Install & sign in"}
                  </button>
                  <button className="btn ghost" onClick={refreshGh} disabled={checkingGh}>
                    {checkingGh ? <Spinner dark /> : "I'm done, check again"}
                  </button>
                </div>
              )}
              {!gh?.loggedIn && (
                <div className="hint muted" style={{ marginTop: 10, fontSize: 13 }}>
                  Opens Terminal and runs <code>gh auth login</code>. Follow the prompts, then come back here.
                </div>
              )}
            </div>
            {error && (
              <div className="notice error">
                <span className="ico">⚠️</span>
                {error}
              </div>
            )}
          </>
        )}
      </div>

      <div className="onboard-foot">
        {step === "github" && !gh?.loggedIn ? (
          <button className="btn ghost gray" onClick={finish} disabled={saving}>
            Skip for now
          </button>
        ) : (
          <span />
        )}
        {step === "github" ? (
          <button className="btn green" onClick={finish} disabled={saving}>
            {saving ? <Spinner /> : "Let's go"}
          </button>
        ) : (
          <button
            className="btn green"
            onClick={next}
            disabled={(step === "root" && !root.trim()) || (step === "groups" && enabledGroups.size === 0)}
          >
            {step === "welcome" ? "Get started" : "Continue"}
          </button>
        )}
      </div>
    </div>
  );
}

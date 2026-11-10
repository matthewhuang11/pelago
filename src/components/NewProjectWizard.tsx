import { useEffect, useMemo, useState } from "react";
import { api, confirmPublicPublish, errorText, type CreateResult } from "../api";
import type { Config, Editor, GhStatus, GroupInfo } from "../types";
import { classNames } from "../util";
import { Modal, Spinner, Toggle } from "./ui";

const TEMPLATES = [
  { id: "blank", emoji: "📄", title: "Blank", desc: "Just a README" },
  { id: "node-ts", emoji: "🟢", title: "Node + TS", desc: "tsx, TypeScript" },
  { id: "python", emoji: "🐍", title: "Python", desc: "main.py + venv-ready" },
  { id: "react-vite", emoji: "⚛️", title: "React + Vite", desc: "TypeScript SPA" },
];

const GITHUB = [
  { id: "none", emoji: "🚫", title: "No repo", desc: "Local only" },
  { id: "private", emoji: "🔒", title: "Private", desc: "Only you can see it" },
  { id: "public", emoji: "🌍", title: "Public", desc: "Open source" },
] as const;

type Step = "basics" | "setup" | "working" | "done";

export default function NewProjectWizard({
  config,
  groups,
  editors,
  initialGroup,
  onClose,
  onCreated,
  onParkCreated,
}: {
  config: Config;
  groups: GroupInfo[];
  editors: Editor[];
  initialGroup?: string | null;
  onClose: () => void;
  onCreated: (path: string) => void;
  onParkCreated: (path: string) => Promise<void>;
}) {
  const d = config.newProjectDefaults;
  const [step, setStep] = useState<Step>("basics");
  const [name, setName] = useState("");
  const [group, setGroup] = useState<string | null>(initialGroup ?? groups[0]?.name ?? null);
  const [template, setTemplate] = useState(d.template || "blank");
  const [gitInit, setGitInit] = useState(d.gitInit);
  const [github, setGithub] = useState<"none" | "private" | "public">(d.github);
  const [repoName, setRepoName] = useState("");
  const [description, setDescription] = useState("");
  const [openAfter, setOpenAfter] = useState(d.openAfter);
  const [keepLocal, setKeepLocal] = useState(true);
  const [editor, setEditor] = useState(config.defaultEditor);
  const [gh, setGh] = useState<GhStatus | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<CreateResult | null>(null);

  useEffect(() => {
    api.ghStatus().then(setGh).catch(() => {});
  }, []);

  const slug = useMemo(
    () =>
      name
        .trim()
        .toLowerCase()
        .replace(/[^a-z0-9]+/g, "-")
        .replace(/^-+|-+$/g, ""),
    [name],
  );
  const nameError =
    name && (name.includes("/") || name.includes(":") || name.startsWith("."))
      ? "Name can't contain / or : or start with a dot"
      : null;
  const ghReady = gh?.loggedIn ?? false;
  const effectiveGithub = ghReady ? github : "none";
  const dest = `${config.projectsRoot}${group ? `/${group}` : ""}/${name.trim() || "…"}`.replace(/^\/Users\/[^/]+/, "~");

  const create = async () => {
    setStep("working");
    setError(null);
    try {
      if (effectiveGithub === "public" && !(await confirmPublicPublish(name.trim()))) {
        setStep("setup");
        return;
      }
      const res = await api.createProject(
        {
          name: name.trim(),
          group,
          template,
          gitInit: gitInit || effectiveGithub !== "none",
          github: effectiveGithub,
          repoName: repoName.trim() || undefined,
          description: description.trim() || undefined,
        },
        keepLocal && openAfter,
        editor,
      );
      setResult(res);
      setStep("done");
      onCreated(res.path);
      if (!keepLocal) {
        try { await onParkCreated(res.path); }
        catch (e) { setResult({ ...res, warnings: [...res.warnings, `Project is still local. Could not start parking: ${errorText(e)}`] }); }
      }
    } catch (e) {
      setError(errorText(e));
      setStep("setup");
    }
  };

  const installed = editors.filter((e) => e.installed);

  return (
    <Modal onClose={step === "working" ? () => {} : onClose} wide>
      <div className="modal-head">
        <span style={{ fontSize: 30 }}>✨</span>
        <h2 className="grow">New project</h2>
        {step !== "working" && (
          <button className="icon-btn" onClick={onClose}>
            ✕
          </button>
        )}
      </div>
      {(step === "basics" || step === "setup") && (
        <div className="progress" style={{ height: 10, marginBottom: 20 }}>
          <div style={{ width: step === "basics" ? "50%" : "100%" }} />
        </div>
      )}

      {step === "basics" && (
        <div className="col" style={{ gap: 18 }}>
          <div className="field">
            <label>Name</label>
            <input
              className={classNames("input", nameError && "error")}
              autoFocus
              placeholder="my-awesome-thing"
              value={name}
              onChange={(e) => setName(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && name.trim() && !nameError && setStep("setup")}
            />
            <div className="hint">{nameError ?? <span className="mono">{dest}</span>}</div>
          </div>
          <div className="field">
            <label>Group</label>
            <div className="choices" style={{ gridTemplateColumns: "repeat(auto-fill, minmax(130px, 1fr))" }}>
              {groups.map((g) => (
                <button key={g.name} className={classNames("choice", group === g.name && "on")} onClick={() => setGroup(g.name)}>
                  <span className="e">{g.emoji}</span>
                  <span className="t ellipsis">{g.name}</span>
                </button>
              ))}
              <button className={classNames("choice", group === null && "on")} onClick={() => setGroup(null)}>
                <span className="e">🏝️</span>
                <span className="t">No group</span>
              </button>
            </div>
          </div>
          <div className="field">
            <label>Template</label>
            <div className="choices">
              {TEMPLATES.map((t) => (
                <button key={t.id} className={classNames("choice", template === t.id && "on")} onClick={() => setTemplate(t.id)}>
                  <span className="e">{t.emoji}</span>
                  <span className="t">{t.title}</span>
                  <span className="d">{t.desc}</span>
                </button>
              ))}
            </div>
          </div>
        </div>
      )}

      {step === "setup" && (
        <div className="col" style={{ gap: 18 }}>
          <div className="field">
            <label>GitHub</label>
            <div className="choices">
              {GITHUB.map((g) => (
                <button
                  key={g.id}
                  className={classNames("choice", effectiveGithub === g.id && "on")}
                  onClick={() => setGithub(g.id)}
                  disabled={!ghReady && g.id !== "none"}
                  style={!ghReady && g.id !== "none" ? { opacity: 0.5, cursor: "not-allowed" } : undefined}
                >
                  <span className="e">{g.emoji}</span>
                  <span className="t">{g.title}</span>
                  <span className="d">{g.desc}</span>
                </button>
              ))}
            </div>
            {gh && !ghReady && (
              <div className="hint">
                Connect GitHub in Settings to create repos.{" "}
                <a onClick={() => api.ghLogin()}>Sign in now</a>
              </div>
            )}
          </div>
          {effectiveGithub !== "none" && (
            <div className="row" style={{ gap: 12 }}>
              <div className="field grow">
                <label>Repo name</label>
                <input className="input mono" placeholder={slug || "repo-name"} value={repoName} onChange={(e) => setRepoName(e.target.value)} />
              </div>
              <div className="field grow">
                <label>Description</label>
                <input className="input" placeholder="Optional" value={description} onChange={(e) => setDescription(e.target.value)} />
              </div>
            </div>
          )}
          <div className="card" style={{ padding: "4px 18px" }}>
            <Toggle
              on={gitInit || effectiveGithub !== "none"}
              disabled={effectiveGithub !== "none"}
              onChange={setGitInit}
              title="Initialize git"
              desc={effectiveGithub !== "none" ? "Required for a GitHub repo" : "git init + first commit"}
            />
            <Toggle on={keepLocal} onChange={setKeepLocal} disabled={!config.vaultRepo} title="Keep on this Mac" desc={config.vaultRepo ? "Turn off to review encrypted parking after creation. Nothing is deleted without your passphrase and confirmation." : "Configure a private vault in Settings to enable parking after creation."} />
            <Toggle on={keepLocal && openAfter} disabled={!keepLocal} onChange={setOpenAfter} title="Open when it's ready" desc="Jump straight into your editor" />
          </div>
          {keepLocal && openAfter && (
            <div className="field">
              <label>Open with</label>
              <select className="input" value={editor} onChange={(e) => setEditor(e.target.value)}>
                {installed.map((e) => (
                  <option key={e.id} value={e.id}>
                    {e.name}
                  </option>
                ))}
              </select>
            </div>
          )}
          {error && (
            <div className="notice error">
              <span className="ico">⚠️</span>
              <span>{error}</span>
            </div>
          )}
        </div>
      )}

      {step === "working" && (
        <div className="empty">
          <Spinner dark />
          <p>Setting up {name}…</p>
        </div>
      )}

      {step === "done" && result && (
        <div className="col" style={{ gap: 12, textAlign: "center", alignItems: "center" }}>
          <div className="hero-art">🎉</div>
          <h2>{name} is ready!</h2>
          <div className="steps-list" style={{ textAlign: "left" }}>
            {result.steps.map((s) => (
              <div key={s} className="step-line">
                ✅ {s}
              </div>
            ))}
            {result.warnings.map((w) => (
              <div key={w} className="step-line error">
                ⚠️ {w}
              </div>
            ))}
          </div>
        </div>
      )}

      <div className="modal-foot">
        {step === "basics" && (
          <button className="btn green" disabled={!name.trim() || !!nameError} onClick={() => setStep("setup")}>
            Continue
          </button>
        )}
        {step === "setup" && (
          <>
            <button className="btn ghost gray" onClick={() => setStep("basics")}>
              Back
            </button>
            <button className="btn green" onClick={create}>
              {keepLocal ? "Create project" : "Create & review parking"}
            </button>
          </>
        )}
        {step === "done" && (
          <button className="btn" onClick={onClose}>
            Done
          </button>
        )}
      </div>
    </Modal>
  );
}

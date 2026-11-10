import { useEffect, useState } from "react";
import { api } from "../api";
import type { Editor, GroupInfo, Project } from "../types";
import { formatBytes, githubUrl, projectEmoji, timeAgo } from "../util";
import { StatusChips, type ProjectActions } from "./ProjectCard";

export default function ProjectDrawer({
  p,
  groups,
  editors,
  actions,
  onClose,
  onEditorChange,
}: {
  p: Project;
  groups: GroupInfo[];
  editors: Editor[];
  actions: ProjectActions;
  onClose: () => void;
  onEditorChange: (p: Project, editor: string | null) => void;
}) {
  const [size, setSize] = useState<number | null>(p.sizeBytes);
  const group = groups.find((g) => g.name === p.group);
  const gh = githubUrl(p.git?.githubSlug);

  useEffect(() => {
    setSize(p.sizeBytes);
    if (p.status === "local") {
      api.projectSize(p.path).then(setSize).catch(() => {});
    }
  }, [p.path, p.status, p.sizeBytes]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const installed = editors.filter((e) => e.installed);

  return (
    <>
      <div className="drawer-overlay" onClick={onClose} />
      <aside className="drawer">
        <div className="row" style={{ marginBottom: 18, alignItems: "flex-start" }}>
          <div style={{ fontSize: 40 }}>{projectEmoji(p.stack)}</div>
          <div className="grow">
            <h2 style={{ wordBreak: "break-word" }}>{p.name}</h2>
            <div className="muted" style={{ fontSize: 13 }}>
              {group ? `${group.emoji} ${group.name}` : p.location === "external" ? "Unsorted" : "Ungrouped"}
            </div>
          </div>
          <button className="icon-btn" onClick={onClose}>
            ✕
          </button>
        </div>

        <div className="row wrap" style={{ marginBottom: 18 }}>
          <StatusChips p={p} />
        </div>

        <div className="col" style={{ gap: 10 }}>
          {p.status === "local" ? (
            <>
              <button className="btn block" onClick={() => actions.open(p)}>
                Open in {installed.find((e) => e.id === (p.editor ?? ""))?.name ?? "editor"}
              </button>
              <div className="row">
                <button className="btn ghost grow" onClick={() => actions.reveal(p)}>
                  Finder
                </button>
                <button className="btn ghost grow" onClick={() => actions.open(p, "terminal")}>
                  Terminal
                </button>
                {gh && (
                  <button className="btn ghost grow" onClick={() => api.openUrl(gh)}>
                    GitHub
                  </button>
                )}
              </div>
              {(actions.move || actions.park) && (
                <div className="row">
                  {actions.move && (
                    <button className="btn ghost gray grow" onClick={() => actions.move!(p)}>
                      Move
                    </button>
                  )}
                  {actions.park && (
                    <button className="btn ghost gray grow" onClick={() => actions.park!(p)}>
                      Park
                    </button>
                  )}
                </div>
              )}
            </>
          ) : (
            actions.resume && (
              <button className="btn green block" onClick={() => actions.resume!(p)}>
                Restore & open
              </button>
            )
          )}
        </div>

        <h3 style={{ margin: "26px 0 12px" }}>Details</h3>
        <div className="kv">
          <div className="k">Path</div>
          <div className="v mono">{p.displayPath}</div>
          {p.git && (
            <>
              <div className="k">Branch</div>
              <div className="v">{p.git.branch ?? "detached"}</div>
              <div className="k">Remote</div>
              <div className="v">{p.git.remoteUrl ?? <span className="muted">none (local only)</span>}</div>
              <div className="k">Changes</div>
              <div className="v">
                {p.git.dirty === 0 ? "clean" : `${p.git.dirty} uncommitted`}
                {p.git.ahead > 0 && ` · ${p.git.ahead} unpushed`}
                {p.git.behind > 0 && ` · ${p.git.behind} behind`}
              </div>
              <div className="k">Last commit</div>
              <div className="v">{timeAgo(p.git.lastCommit)}</div>
            </>
          )}
          <div className="k">Stack</div>
          <div className="v">{p.stack.length ? p.stack.join(", ") : <span className="muted">unknown</span>}</div>
          <div className="k">{p.status === "parked" ? "Freed" : "Size"}</div>
          <div className="v">{size == null ? <span className="muted">measuring…</span> : formatBytes(size)}</div>
          {p.parkedAt && (
            <>
              <div className="k">Parked</div>
              <div className="v">{timeAgo(p.parkedAt)}</div>
            </>
          )}
          <div className="k">Opened</div>
          <div className="v">{timeAgo(p.lastOpened)}</div>
        </div>

        {p.status === "local" && (
          <div className="field" style={{ marginTop: 22 }}>
            <label>Open with</label>
            <select
              className="input"
              value={p.editor ?? ""}
              onChange={(e) => onEditorChange(p, e.target.value || null)}
            >
              <option value="">Default editor</option>
              {installed.map((e) => (
                <option key={e.id} value={e.id}>
                  {e.name}
                </option>
              ))}
            </select>
          </div>
        )}

        {p.duplicates.length > 0 && (
          <div className="notice warn" style={{ marginTop: 22, alignItems: "flex-start" }}>
            <span className="ico">⧉</span>
            <div>
              <b>Same repo checked out elsewhere:</b>
              {p.duplicates.map((d) => (
                <div key={d} className="mono" style={{ fontSize: 12 }}>
                  {d.replace(/^\/Users\/[^/]+/, "~")}
                </div>
              ))}
            </div>
          </div>
        )}
        {p.nestedRepos.length > 0 && (
          <div className="notice info" style={{ marginTop: 12, alignItems: "flex-start" }}>
            <span className="ico">🪆</span>
            <div>
              <b>Contains other git repos:</b> {p.nestedRepos.join(", ")}
            </div>
          </div>
        )}
      </aside>
    </>
  );
}

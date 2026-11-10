import { useEffect, useRef, useState } from "react";
import type { Editor, Project, GroupInfo } from "../types";
import { classNames, formatBytes, projectEmoji, stackColor, timeAgo } from "../util";

export interface ProjectActions {
  open: (p: Project, editor?: string) => void;
  details: (p: Project) => void;
  togglePin: (p: Project) => void;
  reveal: (p: Project) => void;
  move?: (p: Project) => void;
  park?: (p: Project) => void;
  resume?: (p: Project) => void;
}

function tileColor(p: Project, groups: GroupInfo[]): string {
  const g = groups.find((x) => x.name === p.group);
  if (g) return g.color;
  const s = p.stack[0];
  return s ? stackColor(s) : "#1c9af0";
}

export function StatusChips({ p }: { p: Project }) {
  const g = p.git;
  return (
    <>
      {p.status === "parked" && <span className="chip blue">☁️ Parked</span>}
      {p.status === "local" && !g && !p.empty && <span className="chip">No git</span>}
      {p.empty && <span className="chip">Empty</span>}
      {g && !g.remoteUrl && <span className="chip orange">Local only</span>}
      {g && g.dirty > 0 && <span className="chip orange">● {g.dirty} changed</span>}
      {g && g.ahead > 0 && <span className="chip purple">↑ {g.ahead}</span>}
      {p.duplicates.length > 0 && <span className="chip red">⧉ Duplicate</span>}
      {p.nestedRepos.length > 0 && <span className="chip purple">Nested repos</span>}
    </>
  );
}

export function ProjectCard({
  p,
  groups,
  actions,
  showGroup,
}: {
  p: Project;
  groups: GroupInfo[];
  actions: ProjectActions;
  showGroup?: boolean;
}) {
  const [menu, setMenu] = useState(false);
  const menuRef = useRef<HTMLDivElement>(null);
  const group = groups.find((g) => g.name === p.group);

  useEffect(() => {
    if (!menu) return;
    const close = (e: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(e.target as Node)) setMenu(false);
    };
    window.addEventListener("mousedown", close);
    return () => window.removeEventListener("mousedown", close);
  }, [menu]);

  const onClick = () => {
    if (p.status === "parked") actions.resume?.(p);
    else actions.open(p);
  };

  return (
    <div
      className={classNames("card clickable project-card", p.status === "parked" && "parked")}
      onClick={onClick}
      title={p.status === "parked" ? "Click to restore and open" : `Open ${p.name}`}
    >
      <div className="top">
        <div className="tile" style={{ background: tileColor(p, groups) + "22", color: tileColor(p, groups) }}>
          {projectEmoji(p.stack)}
        </div>
        <div className="grow" style={{ minWidth: 0, paddingRight: p.pinned ? 40 : 0 }}>
          <div className="name" title={p.name}>{p.name}</div>
          <div className="path ellipsis">{p.git?.githubSlug ?? p.displayPath}</div>
        </div>
      </div>
      <div className="pin row" style={{ gap: 6 }} onClick={(e) => e.stopPropagation()}>
        <button
          className={classNames("icon-btn", p.pinned && "on")}
          title={p.pinned ? "Unpin" : "Pin"}
          onClick={() => actions.togglePin(p)}
        >
          {p.pinned ? "★" : "☆"}
        </button>
        <div style={{ position: "relative" }} ref={menuRef}>
          <button className="icon-btn" title="More" onClick={() => setMenu(!menu)}>
            ⋯
          </button>
          {menu && (
            <div className="menu" onClick={() => setMenu(false)}>
              {p.status === "local" ? (
                <>
                  <button onClick={() => actions.open(p)}>🧑‍💻 Open</button>
                  <button onClick={() => actions.reveal(p)}>📂 Show in Finder</button>
                  <button onClick={() => actions.open(p, "terminal")}>⌨️ Open in Terminal</button>
                  {actions.move && <button onClick={() => actions.move!(p)}>🧭 Move to group…</button>}
                  {actions.park && <button onClick={() => actions.park!(p)}>☁️ Park (free up space)</button>}
                </>
              ) : (
                actions.resume && <button onClick={() => actions.resume!(p)}>⬇️ Restore & open</button>
              )}
              <hr />
              <button onClick={() => actions.details(p)}>ℹ️ Details</button>
            </div>
          )}
        </div>
      </div>
      <div className="meta">
        {showGroup && group && (
          <span className="chip">
            {group.emoji} {group.name}
          </span>
        )}
        {p.stack.slice(0, 2).map((s) => (
          <span className="chip" key={s}>
            <span className="swatch" style={{ background: stackColor(s) }} />
            {s}
          </span>
        ))}
        <StatusChips p={p} />
      </div>
      <div className="foot">
        {p.git?.branch && <span className="ellipsis">⎇ {p.git.branch}</span>}
        <span className="spacer" />
        {p.status === "parked" ? (
          <span>{formatBytes(p.sizeBytes)} freed</span>
        ) : (
          <span>{timeAgo(p.lastActivity)}</span>
        )}
      </div>
    </div>
  );
}

export function ProjectGrid({
  projects,
  groups,
  actions,
  showGroup,
  empty,
}: {
  projects: Project[];
  groups: GroupInfo[];
  actions: ProjectActions;
  showGroup?: boolean;
  empty?: React.ReactNode;
}) {
  if (projects.length === 0) return <>{empty ?? null}</>;
  return (
    <div className="grid">
      {projects.map((p) => (
        <ProjectCard key={p.id} p={p} groups={groups} actions={actions} showGroup={showGroup} />
      ))}
    </div>
  );
}

export function editorName(editors: Editor[], id: string | null | undefined): string {
  return editors.find((e) => e.id === id)?.name ?? id ?? "";
}

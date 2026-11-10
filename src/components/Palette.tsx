import { useEffect, useMemo, useRef, useState } from "react";
import type { GroupInfo, Project } from "../types";
import { classNames, projectEmoji } from "../util";

export interface Command {
  id: string;
  title: string;
  subtitle?: string;
  emoji: string;
  run: () => void;
}

function score(q: string, text: string): number {
  if (!q) return 1;
  const t = text.toLowerCase();
  const i = t.indexOf(q);
  if (i === 0) return 100;
  if (i > 0) return 60 - Math.min(i, 30);
  // subsequence match
  let ti = 0;
  for (const ch of q) {
    ti = t.indexOf(ch, ti);
    if (ti < 0) return 0;
    ti++;
  }
  return 10;
}

export default function Palette({
  projects,
  groups,
  commands,
  onOpen,
  onClose,
}: {
  projects: Project[];
  groups: GroupInfo[];
  commands: Command[];
  onOpen: (p: Project) => void;
  onClose: () => void;
}) {
  const [q, setQ] = useState("");
  const [sel, setSel] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);

  const items: Command[] = useMemo(() => {
    const query = q.trim().toLowerCase();
    const projectItems: (Command & { s: number; recent: number; activity: number })[] = projects
      .map((p) => {
        const g = groups.find((x) => x.name === p.group);
        return {
          id: p.id,
          title: p.name,
          subtitle: `${g ? `${g.emoji} ${g.name} · ` : ""}${p.status === "parked" ? "☁️ parked" : p.displayPath}`,
          emoji: projectEmoji(p.stack),
          run: () => onOpen(p),
          s: Math.max(score(query, p.name), score(query, p.git?.githubSlug ?? "") * 0.8, score(query, p.group ?? "") * 0.5),
          recent: p.lastOpened ?? 0,
          activity: p.lastActivity,
        };
      })
      .filter((x) => x.s > 0);
    const cmdItems = commands
      .map((c) => ({ ...c, s: query ? score(query, c.title) * 0.9 : 0.5 }))
      .filter((x) => x.s > 0);
    if (!query) {
      projectItems.sort((a, b) => b.recent - a.recent || b.activity - a.activity);
      return [...projectItems.slice(0, 8), ...cmdItems];
    }
    const all: (Command & { s: number; recent?: number })[] = [...projectItems, ...cmdItems];
    return all.sort((a, b) => b.s - a.s || (b.recent ?? 0) - (a.recent ?? 0)).slice(0, 30);
  }, [q, projects, groups, commands, onOpen]);

  useEffect(() => setSel(0), [q]);
  useEffect(() => {
    listRef.current?.querySelector(".palette-item.on")?.scrollIntoView({ block: "nearest" });
  }, [sel]);

  const onKey = (e: React.KeyboardEvent) => {
    if (e.key === "Escape") onClose();
    else if (e.key === "ArrowDown") {
      e.preventDefault();
      setSel((s) => Math.min(s + 1, items.length - 1));
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setSel((s) => Math.max(s - 1, 0));
    } else if (e.key === "Enter" && items[sel]) {
      items[sel].run();
      onClose();
    }
  };

  return (
    <div className="palette" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="palette-box">
        <input autoFocus placeholder="Jump to a project or run a command…" value={q} onChange={(e) => setQ(e.target.value)} onKeyDown={onKey} />
        <div className="palette-list" ref={listRef}>
          {items.length === 0 && <div className="muted" style={{ padding: 16 }}>No matches</div>}
          {items.map((it, i) => (
            <div
              key={it.id}
              className={classNames("palette-item", i === sel && "on")}
              onMouseEnter={() => setSel(i)}
              onClick={() => {
                it.run();
                onClose();
              }}
            >
              <span className="e">{it.emoji}</span>
              <div className="grow">
                <div className="t">{it.title}</div>
                {it.subtitle && <div className="d ellipsis">{it.subtitle}</div>}
              </div>
              {i === sel && <span className="kbd">↵</span>}
            </div>
          ))}
        </div>
      </div>
    </div>
  );
}

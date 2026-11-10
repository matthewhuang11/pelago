import { useCallback, useEffect, useMemo, useState } from "react";
import "./App.css";
import { api } from "./api";
import type { Config, Editor, Project, ProjectList, View } from "./types";
import { ToastProvider, useToast } from "./components/Toast";
import Onboarding from "./components/Onboarding";
import Settings from "./components/Settings";
import ProjectDrawer from "./components/ProjectDrawer";
import Palette, { type Command } from "./components/Palette";
import { ProjectGrid, type ProjectActions } from "./components/ProjectCard";
import { Spinner } from "./components/ui";
import { classNames, formatBytes } from "./util";

export default function App() {
  return (
    <ToastProvider>
      <Root />
    </ToastProvider>
  );
}

function Root() {
  const [config, setConfig] = useState<Config | null>(null);
  useEffect(() => {
    api.getConfig().then(setConfig);
  }, []);
  if (!config) return <div className="titlebar" data-tauri-drag-region />;
  if (!config.onboarded) {
    return <Onboarding defaultRoot={config.projectsRoot} onDone={() => api.getConfig().then(setConfig)} />;
  }
  return <Shell config={config} setConfig={setConfig} />;
}

type SortKey = "recent" | "name" | "opened";

function Shell({ config, setConfig }: { config: Config; setConfig: (c: Config) => void }) {
  const toast = useToast();
  const [list, setList] = useState<ProjectList | null>(null);
  const [loading, setLoading] = useState(false);
  const [editors, setEditors] = useState<Editor[]>([]);
  const [view, setView] = useState<View>({ kind: "home" });
  const [query, setQuery] = useState("");
  const [sort, setSort] = useState<SortKey>("recent");
  const [detail, setDetail] = useState<Project | null>(null);
  const [palette, setPalette] = useState(false);

  const refresh = useCallback(async () => {
    setLoading(true);
    try {
      const l = await api.listProjects();
      setList(l);
      setDetail((d) => (d ? l.projects.find((p) => p.id === d.id) ?? null : null));
    } catch (e) {
      toast.error(e);
    } finally {
      setLoading(false);
    }
  }, [toast]);

  const refreshEditors = useCallback(
    (force = false) => api.listEditors(force).then(setEditors).catch(toast.error),
    [toast],
  );

  useEffect(() => {
    refresh();
    refreshEditors();
  }, [refresh, refreshEditors]);

  // Re-scan when the window regains focus (projects change outside Pelago).
  useEffect(() => {
    let last = Date.now();
    const onFocus = () => {
      if (Date.now() - last > 4000) {
        last = Date.now();
        refresh();
      }
    };
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [refresh]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.metaKey && e.key.toLowerCase() === "k") {
        e.preventDefault();
        setPalette((x) => !x);
      } else if (e.metaKey && e.key.toLowerCase() === "r") {
        e.preventDefault();
        refresh();
      } else if (e.metaKey && e.key === ",") {
        e.preventDefault();
        setView({ kind: "settings" });
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [refresh]);

  const projects = list?.projects ?? [];
  const groups = list?.groups ?? [];

  const patchProject = (id: string, patch: Partial<Project>) =>
    setList((l) => (l ? { ...l, projects: l.projects.map((p) => (p.id === id ? { ...p, ...patch } : p)) } : l));

  const actions: ProjectActions = {
    open: async (p, editor) => {
      try {
        await api.openProject(p.path, editor);
        patchProject(p.id, { lastOpened: Date.now() / 1000 });
      } catch (e) {
        toast.error(e);
      }
    },
    details: (p) => setDetail(p),
    togglePin: async (p) => {
      try {
        await api.setPinned(p.path, !p.pinned);
        patchProject(p.id, { pinned: !p.pinned });
      } catch (e) {
        toast.error(e);
      }
    },
    reveal: (p) => api.revealInFinder(p.path).catch(toast.error),
  };

  const sorted = useCallback(
    (ps: Project[]) => {
      const q = query.trim().toLowerCase();
      const filtered = q
        ? ps.filter(
            (p) =>
              p.name.toLowerCase().includes(q) ||
              (p.git?.githubSlug ?? "").toLowerCase().includes(q) ||
              p.stack.some((s) => s.toLowerCase().includes(q)),
          )
        : ps;
      const out = [...filtered];
      if (sort === "name") out.sort((a, b) => a.name.localeCompare(b.name));
      else if (sort === "opened") out.sort((a, b) => (b.lastOpened ?? 0) - (a.lastOpened ?? 0) || b.lastActivity - a.lastActivity);
      else out.sort((a, b) => b.lastActivity - a.lastActivity);
      return out;
    },
    [query, sort],
  );

  const unsorted = projects.filter((p) => p.location === "external");
  const parked = projects.filter((p) => p.status === "parked");
  const pinned = projects.filter((p) => p.pinned);
  const ungrouped = projects.filter((p) => p.location === "root" && !p.group);

  const commands: Command[] = useMemo(
    () => [
      { id: "cmd:home", title: "Go to Home", emoji: "🏠", run: () => setView({ kind: "home" }) },
      { id: "cmd:unsorted", title: "Go to Unsorted", emoji: "📥", run: () => setView({ kind: "unsorted" }) },
      { id: "cmd:parked", title: "Go to Parked", emoji: "☁️", run: () => setView({ kind: "parked" }) },
      { id: "cmd:settings", title: "Open Settings", emoji: "⚙️", run: () => setView({ kind: "settings" }) },
      { id: "cmd:refresh", title: "Rescan projects", emoji: "🔄", run: () => refresh() },
      ...groups.map((g) => ({
        id: "cmd:group:" + g.name,
        title: `Go to ${g.name}`,
        emoji: g.emoji,
        run: () => setView({ kind: "group", name: g.name }),
      })),
    ],
    [groups, refresh],
  );

  const navItem = (v: View, icon: string, label: string, count?: number) => {
    const active =
      view.kind === v.kind && (v.kind !== "group" || (view.kind === "group" && view.name === v.name));
    return (
      <button
        key={label}
        className={classNames("nav-item", active && "active", v.kind === "group" && "nav-group")}
        onClick={() => setView(v)}
      >
        <span className="icon">{icon}</span>
        <span className="ellipsis">{label}</span>
        {count !== undefined && count > 0 && <span className="count">{count}</span>}
      </button>
    );
  };

  const toolbar = (
    <div className="row" style={{ marginBottom: 20, gap: 12 }}>
      <div className="search">
        <span className="ico">🔍</span>
        <input className="input" placeholder="Filter projects…" value={query} onChange={(e) => setQuery(e.target.value)} />
      </div>
      <select className="input" style={{ width: 170 }} value={sort} onChange={(e) => setSort(e.target.value as SortKey)}>
        <option value="recent">Recently active</option>
        <option value="opened">Recently opened</option>
        <option value="name">Name</option>
      </select>
      <span className="spacer" />
      <button className="btn ghost gray small" onClick={() => setPalette(true)} title="Quick open (⌘K)">
        ⌘K
      </button>
      <button className="icon-btn" onClick={refresh} title="Rescan (⌘R)">
        {loading ? <Spinner dark /> : "↻"}
      </button>
    </div>
  );

  let content: React.ReactNode;
  if (!list) {
    content = (
      <div className="empty">
        <Spinner dark />
        <p>Scanning your projects…</p>
      </div>
    );
  } else if (view.kind === "settings") {
    content = (
      <Settings
        config={config}
        editors={editors}
        onChanged={(c) => {
          setConfig(c);
          refresh();
        }}
        onRefreshEditors={() => refreshEditors(true)}
      />
    );
  } else if (view.kind === "home") {
    const totalParked = parked.reduce((n, p) => n + (p.sizeBytes ?? 0), 0);
    const dirty = projects.filter((p) => (p.git?.dirty ?? 0) > 0).length;
    const recent = [...projects]
      .filter((p) => p.status === "local")
      .sort((a, b) => (b.lastOpened ?? 0) - (a.lastOpened ?? 0) || b.lastActivity - a.lastActivity)
      .slice(0, 6);
    content = (
      <div className="main-inner">
        <div className="banner">
          <div>
            <h2>Welcome back!</h2>
            <p>
              {projects.length} projects across {groups.filter((g) => g.count > 0).length} groups.{" "}
              {unsorted.length > 0 ? `${unsorted.length} still waiting to be sorted.` : "Everything's sorted. 🎉"}
            </p>
          </div>
          <div className="art">🏝️</div>
        </div>

        <div className="stats section" style={{ marginTop: 20 }}>
          <div className="card stat">
            <span className="ico">🗂️</span>
            <div>
              <div className="big">{projects.length}</div>
              <div className="label">Projects</div>
            </div>
          </div>
          <div className="card stat">
            <span className="ico">✏️</span>
            <div>
              <div className="big">{dirty}</div>
              <div className="label">With uncommitted work</div>
            </div>
          </div>
          <div className="card stat">
            <span className="ico">📥</span>
            <div>
              <div className="big">{unsorted.length}</div>
              <div className="label">Unsorted</div>
            </div>
          </div>
          <div className="card stat">
            <span className="ico">☁️</span>
            <div>
              <div className="big">{parked.length}</div>
              <div className="label">Parked{totalParked ? ` · ${formatBytes(totalParked)} freed` : ""}</div>
            </div>
          </div>
        </div>

        {list.warnings
          .filter((w) => w.kind !== "missingRoot")
          .map((w) => (
            <div key={w.kind + w.path} className="notice warn" style={{ marginTop: 16 }}>
              <span className="ico">⚠️</span>
              <span className="grow">{w.message}</span>
            </div>
          ))}

        {pinned.length > 0 && (
          <div className="section">
            <div className="section-head">
              <h2>⭐ Pinned</h2>
            </div>
            <ProjectGrid projects={pinned} groups={groups} actions={actions} showGroup />
          </div>
        )}

        <div className="section">
          <div className="section-head">
            <h2>Jump back in</h2>
            <span className="spacer" />
            <span className="muted">
              <span className="kbd">⌘K</span> to search
            </span>
          </div>
          <ProjectGrid
            projects={recent}
            groups={groups}
            actions={actions}
            showGroup
            empty={
              <div className="empty card">
                <div className="art">🌊</div>
                <h3>No projects yet</h3>
                <p>Add a folder to scan in Settings, or create your first project.</p>
              </div>
            }
          />
        </div>

        <div className="section">
          <div className="section-head">
            <h2>Groups</h2>
          </div>
          <div className="grid tight">
            {groups.map((g) => (
              <div key={g.name} className="card clickable" onClick={() => setView({ kind: "group", name: g.name })}>
                <div style={{ fontSize: 30 }}>{g.emoji}</div>
                <div style={{ fontWeight: 800, marginTop: 6 }}>{g.name}</div>
                <div className="muted" style={{ fontSize: 13 }}>
                  {g.count} project{g.count === 1 ? "" : "s"}
                </div>
                <div className="progress" style={{ height: 8, marginTop: 10 }}>
                  <div style={{ width: `${Math.min(100, g.count * 10)}%`, background: g.color }} />
                </div>
              </div>
            ))}
          </div>
        </div>
      </div>
    );
  } else {
    let title = "";
    let emoji = "";
    let subtitle: React.ReactNode = null;
    let ps: Project[] = [];
    let empty: React.ReactNode = null;
    if (view.kind === "pinned") {
      title = "Pinned";
      emoji = "⭐";
      ps = pinned;
      empty = <EmptyState art="⭐" title="Nothing pinned" text="Hover a project card and tap the star to pin it here." />;
    } else if (view.kind === "unsorted") {
      title = "Unsorted";
      emoji = "📥";
      ps = unsorted;
      subtitle = "Projects found in your scan folders. Move them into a group when you're ready, one at a time.";
      empty = <EmptyState art="🎉" title="All sorted!" text="Every project lives in a group." />;
    } else if (view.kind === "parked") {
      title = "Parked";
      emoji = "☁️";
      ps = parked;
      subtitle = "Saved safely to GitHub and removed from this Mac. Click one to bring it back exactly where you left off.";
      empty = <EmptyState art="☁️" title="Nothing parked" text="Park a project you're not using to free disk space." />;
    } else if (view.kind === "group") {
      const g = groups.find((x) => x.name === view.name);
      title = view.name;
      emoji = g?.emoji ?? "📁";
      ps = projects.filter((p) => p.group === view.name);
      subtitle = <span className="mono">{`${list.root}/${view.name}`.replace(/^\/Users\/[^/]+/, "~")}</span>;
      empty = <EmptyState art={emoji} title={`No projects in ${view.name} yet`} text="Create one or move an existing project here." />;
    }
    content = (
      <div className="main-inner">
        <div className="page-head">
          <span className="emoji">{emoji}</span>
          <div className="grow">
            <h1>{title}</h1>
            {subtitle && (
              <div className="muted" style={{ fontSize: 14, marginTop: 2 }}>
                {subtitle}
              </div>
            )}
          </div>
        </div>
        {toolbar}
        <ProjectGrid projects={sorted(ps)} groups={groups} actions={actions} showGroup={view.kind !== "group"} empty={empty} />
      </div>
    );
  }

  return (
    <div className="app">
      <div className="titlebar" data-tauri-drag-region />
      <nav className="sidebar">
        <div className="brand">
          <img src="/pelago.png" alt="" />
          <span>pelago</span>
        </div>
        {navItem({ kind: "home" }, "🏠", "Home")}
        {navItem({ kind: "pinned" }, "⭐", "Pinned", pinned.length)}
        {navItem({ kind: "unsorted" }, "📥", "Unsorted", unsorted.length)}
        {navItem({ kind: "parked" }, "☁️", "Parked", parked.length)}
        <div className="nav-label caps faint">Groups</div>
        {groups.map((g) => navItem({ kind: "group", name: g.name }, g.emoji, g.name, g.count))}
        {ungrouped.length > 0 && (
          <div className="muted" style={{ fontSize: 12, padding: "6px 14px" }}>
            {ungrouped.length} loose project{ungrouped.length === 1 ? "" : "s"} in root
          </div>
        )}
        <span className="spacer" />
        {navItem({ kind: "settings" }, "⚙️", "Settings")}
      </nav>
      <main className="main">{content}</main>

      {detail && (
        <ProjectDrawer
          p={detail}
          groups={groups}
          editors={editors}
          actions={actions}
          onClose={() => setDetail(null)}
          onEditorChange={async (p, ed) => {
            try {
              await api.setProjectEditor(p.path, ed);
              patchProject(p.id, { editor: ed });
              setDetail({ ...p, editor: ed });
            } catch (e) {
              toast.error(e);
            }
          }}
        />
      )}
      {palette && (
        <Palette
          projects={projects}
          groups={groups}
          commands={commands}
          onOpen={(p) => (p.status === "parked" ? setDetail(p) : actions.open(p))}
          onClose={() => setPalette(false)}
        />
      )}
    </div>
  );
}

function EmptyState({ art, title, text }: { art: string; title: string; text: string }) {
  return (
    <div className="empty card">
      <div className="art">{art}</div>
      <h3>{title}</h3>
      <p>{text}</p>
    </div>
  );
}

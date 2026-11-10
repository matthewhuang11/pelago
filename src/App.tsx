import { useCallback, useEffect, useMemo, useState } from "react";
import "./App.css";
import { api, confirmPublicPublish, errorText, inTauri } from "./api";
import { listen } from "@tauri-apps/api/event";
import type { Config, Editor, Project, ProjectList, View } from "./types";
import { ToastProvider, useToast } from "./components/Toast";
import Onboarding from "./components/Onboarding";
import Settings from "./components/Settings";
import ProjectDrawer from "./components/ProjectDrawer";
import Palette, { type Command } from "./components/Palette";
import { ProjectGrid, type ProjectActions } from "./components/ProjectCard";
import { Spinner } from "./components/ui";
import NewProjectWizard from "./components/NewProjectWizard";
import MoveDialog from "./components/MoveDialog";
import GroupDialog from "./components/GroupDialog";
import ParkingDialog from "./components/ParkingDialog";
import type { GroupInfo } from "./types";
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
  const [error, setError] = useState<string | null>(null);
  const load = useCallback(() => {
    setError(null);
    api.getConfig().then(setConfig).catch((e) => setError(errorText(e)));
  }, []);
  useEffect(() => {
    load();
  }, [load]);
  if (error) return <div className="empty"><h2>Couldn't load Pelago</h2><p role="alert">{error}</p><button className="btn" onClick={load}>Try again</button></div>;
  if (!config) return <div className="empty"><Spinner dark /><p>Loading Pelago…</p></div>;
  if (!config.onboarded) {
    return <Onboarding defaultRoot={config.projectsRoot} onDone={load} />;
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
  const [creating, setCreating] = useState<{ group: string | null } | null>(null);
  const [moving, setMoving] = useState<Project | null>(null);
  const [parking, setParking] = useState<{ project: Project; restore: boolean } | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [editingGroup, setEditingGroup] = useState<GroupInfo | null | "new">(null);

  useEffect(() => {
    if (!inTauri) return;
    let disposed = false;
    let unlisten: (() => void) | undefined;
    listen("pelago:quick-open", () => { if (!disposed) setPalette(true); })
      .then((stop) => { if (disposed) stop(); else unlisten = stop; })
      .catch(toast.error);
    return () => { disposed = true; unlisten?.(); };
  }, [toast]);

  const refresh = useCallback(async () => {
    setLoading(true);
    setLoadError(null);
    try {
      const l = await api.listProjects();
      setList(l);
      setDetail((d) => (d ? l.projects.find((p) => p.id === d.id) ?? null : null));
    } catch (e) {
      setLoadError(errorText(e));
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

  // Browser preview only: jump to a scene from URL params (used by scripts/screenshot.sh).
  useEffect(() => {
    if (inTauri || !list) return;
    import("./mock").then(({ mockScene }) => {
      if (mockScene.view === "unsorted") setView({ kind: "unsorted" });
      if (mockScene.view === "parked") setView({ kind: "parked" });
      if (mockScene.view === "settings") setView({ kind: "settings" });
      if (mockScene.view === "group") setView({ kind: "group", name: "Startups" });
      if (mockScene.open === "new") setCreating({ group: "Startups" });
      if (mockScene.open === "move") setMoving(list.projects.find((p) => p.location === "external") ?? null);
      if (mockScene.open === "palette") setPalette(true);
      if (mockScene.open === "detail") setDetail(list.projects[1] ?? null);
      if (mockScene.open === "park") setParking({ project: list.projects.find((p) => p.status === "local")!, restore: false });
      if (mockScene.open === "restore") setParking({ project: list.projects.find((p) => p.status === "parked")!, restore: true });
    });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [list === null]);

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
      if (p.status === "parked") { setParking({ project: p, restore: true }); return; }
      try {
        await api.openProject(p.path, editor);
        patchProject(p.id, { lastOpened: Date.now() / 1000 });
      } catch (e) {
        toast.error(e);
      }
    },
    details: (p) => setDetail(p),
    park: (p) => { setDetail(null); setParking({ project: p, restore: false }); },
    resume: (p) => { setDetail(null); setParking({ project: p, restore: true }); },
    togglePin: async (p) => {
      try {
        await api.setPinned(p.path, !p.pinned);
        patchProject(p.id, { pinned: !p.pinned });
      } catch (e) {
        toast.error(e);
      }
    },
    reveal: (p) => api.revealInFinder(p.path).catch(toast.error),
    move: (p) => {
      setDetail(null);
      setMoving(p);
    },
    publish: async (p, isPrivate) => {
      try {
        if (!isPrivate && !(await confirmPublicPublish(p.name))) return;
        toast.show(`Creating ${isPrivate ? "private" : "public"} repo for ${p.name}…`);
        const slug = await api.publishProject(p.path, isPrivate);
        toast.show(`Published to github.com/${slug}`, "success");
        refresh();
      } catch (e) {
        toast.error(e);
      }
    },
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
        {loadError ? <><h3>Couldn't scan projects</h3><p role="alert">{loadError}</p><button className="btn" onClick={refresh}>Try again</button></> : <><Spinner dark /><p>Scanning your projects…</p></>}
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
    const stale = config.staleDays > 0 ? projects
      .filter((p) => p.status === "local" && Math.max(p.lastActivity, p.lastOpened ?? 0) < Date.now() / 1000 - config.staleDays * 86400)
      .sort((a, b) => Math.max(a.lastActivity, a.lastOpened ?? 0) - Math.max(b.lastActivity, b.lastOpened ?? 0))
      .slice(0, 3) : [];
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
              {w.kind === "emptyOuterRepo" && (
                <button
                  className="btn small"
                  onClick={async () => {
                    try {
                      await api.removeEmptyRepo(w.path);
                      toast.show("Moved the empty .git folder to the Trash", "success");
                      refresh();
                    } catch (e) {
                      toast.error(e);
                    }
                  }}
                >
                  Remove it
                </button>
              )}
            </div>
          ))}

        {stale.length > 0 && <div className="section">
          <div className="section-head"><h2>☁️ Make room for what's next</h2></div>
          <div className="card col" style={{ gap: 14 }}>
            <p className="muted" style={{ margin: 0 }}>These projects haven't been active or opened in {config.staleDays} days. Parking is optional and always requires your passphrase and explicit local-delete confirmation.</p>
            {stale.map((p) => <div className="row" key={p.id}><div className="grow"><b>{p.name}</b><div className="mono hint ellipsis">{p.displayPath}</div></div><button className="btn ghost small" onClick={() => actions.park?.(p)}>Review parking</button></div>)}
          </div>
        </div>}

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
      subtitle = "Encrypted snapshots in your private GitHub vault. Restore saved files with your passphrase. Unsaved buffers are not included, and editor tabs are not guaranteed to restore.";
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
          {view.kind === "group" && (
            <>
              <button
                className="btn ghost gray small"
                onClick={() => setEditingGroup(groups.find((g) => g.name === view.name) ?? null)}
              >
                Edit group
              </button>
              <button className="btn small" onClick={() => setCreating({ group: view.name })}>
                + New here
              </button>
            </>
          )}
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
        <div className="nav-label caps faint row">
          <span className="grow">Groups</span>
          <button className="icon-btn" style={{ width: 24, height: 24, fontSize: 13 }} title="New group" onClick={() => setEditingGroup("new")}>
            +
          </button>
        </div>
        {groups.map((g) => navItem({ kind: "group", name: g.name }, g.emoji, g.name, g.count))}
        {ungrouped.length > 0 && (
          <div className="muted" style={{ fontSize: 12, padding: "6px 14px" }}>
            {ungrouped.length} loose project{ungrouped.length === 1 ? "" : "s"} in root
          </div>
        )}
        <span className="spacer" />
        <button className="btn green block" style={{ marginBottom: 8 }} onClick={() => setCreating({ group: view.kind === "group" ? view.name : null })}>
          + New project
        </button>
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
      {creating && (
        <NewProjectWizard
          config={config}
          groups={groups}
          editors={editors}
          initialGroup={creating.group}
          onClose={() => setCreating(null)}
          onCreated={() => {
            api.getConfig().then(setConfig).catch(toast.error);
            refresh();
          }}
          onParkCreated={async (path) => {
            const updated = await api.listProjects();
            const project = updated.projects.find((p) => p.path === path);
            if (!project) throw new Error("Project was created locally but could not be found for parking. Rescan and park it from its card.");
            setList(updated);
            setCreating(null);
            setParking({ project, restore: false });
          }}
        />
      )}
      {moving && (
        <MoveDialog
          project={moving}
          groups={groups}
          onClose={() => setMoving(null)}
          onMoved={(rec) => {
            refresh();
            toast.show(`Moved ${rec.name}`, "success", {
              label: "Undo",
              run: () =>
                api
                  .undoMove(rec.id)
                  .then(() => {
                    toast.show(`Moved ${rec.name} back`, "success");
                    refresh();
                  })
                  .catch(toast.error),
            });
          }}
        />
      )}
      {parking && <ParkingDialog project={parking.project} restore={parking.restore} vaultRepo={config.vaultRepo} onClose={() => setParking(null)} onComplete={async () => { await refresh(); }} />}
      {editingGroup && (
        <GroupDialog
          group={editingGroup === "new" ? null : editingGroup}
          onClose={() => setEditingGroup(null)}
          onSaved={(c, name) => {
            setConfig(c);
            if (view.kind === "group") setView({ kind: "group", name });
            refresh();
          }}
          onDeleted={(c) => {
            setConfig(c);
            setView({ kind: "home" });
            refresh();
          }}
        />
      )}
      {palette && (
        <Palette
          projects={projects}
          groups={groups}
          commands={commands}
          onOpen={(p) => actions.open(p)}
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

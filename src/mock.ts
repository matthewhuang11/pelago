// Browser preview mode: lets you run `npm run dev` and open http://localhost:1420
// in a normal browser to work on the UI without the Rust backend.
// Never used inside the real app.
import type { Config, Editor, Project, ProjectList } from "./types";
import type { MoveRecord } from "./api";

const now = Math.floor(Date.now() / 1000);
const day = 86400;

const params = new URLSearchParams(location.search);

const config: Config = {
  version: 1,
  onboarded: params.get("onboarding") !== "1",
  projectsRoot: "/Users/you/Code",
  scanPaths: ["/Users/you/Old Projects"],
  defaultEditor: "vscode",
  groups: [
    { name: "Startups", emoji: "🚀", color: "#ff9600" },
    { name: "Hackathons", emoji: "⚡", color: "#ffc800" },
    { name: "Experiments", emoji: "🧪", color: "#ce82ff" },
    { name: "School", emoji: "🎓", color: "#58cc02" },
    { name: "Websites", emoji: "🌐", color: "#1c9af0" },
    { name: "Archive", emoji: "📦", color: "#afafaf" },
  ],
  projects: {},
  customEditors: [],
  vaultRepo: params.get("vault") === "missing" ? null : "you/pelago-vault",
  staleDays: 60,
  newProjectDefaults: { gitInit: true, github: "private", openAfter: true, template: "blank" },
};

function proj(name: string, group: string | null, stack: string[], extra: Partial<Project> = {}): Project {
  const path = group ? `/Users/you/Code/${group}/${name}` : `/Users/you/Old Projects/${name}`;
  return {
    id: path,
    name,
    path,
    displayPath: path.replace("/Users/you", "~"),
    group,
    location: group ? "root" : "external",
    status: "local",
    git: {
      branch: "main",
      hasCommits: true,
      remoteUrl: `git@github.com:you/${name}.git`,
      githubSlug: `you/${name}`,
      dirty: 0,
      ahead: 0,
      behind: 0,
      hasUpstream: true,
      lastCommit: now - 3 * day,
    },
    stack,
    empty: false,
    nestedRepos: [],
    duplicates: [],
    lastActivity: now - 3 * day,
    pinned: false,
    editor: null,
    lastOpened: null,
    suggestedGroup: null,
    sizeBytes: null,
    parkedAt: null,
    ...extra,
  };
}

const projects: Project[] = [
  proj("rocket-app", "Startups", ["Next.js", "TypeScript"], { pinned: true, lastActivity: now - 3600, lastOpened: now - 3600 }),
  proj("billing-api", "Startups", ["Rust"], { git: { ...proj("billing-api", "Startups", []).git!, dirty: 4, ahead: 2 }, lastActivity: now - day }),
  proj("hack-the-north", "Hackathons", ["React", "TypeScript"], { lastActivity: now - 40 * day }),
  proj("ml-playground", "Experiments", ["Python", "Jupyter"], { lastActivity: now - 2 * day, lastOpened: now - 2 * day }),
  proj("portfolio", "Websites", ["HTML"], { lastActivity: now - 9 * day }),
  proj("cs-homework", "School", ["Java"], { git: { ...proj("cs-homework", "School", []).git!, remoteUrl: null, githubSlug: null }, lastActivity: now - 5 * day }),
  proj("old-blog", null, ["JavaScript"], { suggestedGroup: "Websites", lastActivity: now - 200 * day }),
  proj("scratchpad", null, ["Python"], { git: null, lastActivity: now - 90 * day }),
  proj("game-jam", "Hackathons", ["C++"], {
    status: "parked",
    git: null,
    sizeBytes: 1.9 * 1024 ** 3,
    parkedAt: now - 7 * day,
    lastActivity: now - 120 * day,
  }),
];

const editors: Editor[] = [
  { id: "vscode", name: "VS Code", kind: "editor", installed: true, appPath: "/Applications/Visual Studio Code.app" },
  { id: "cursor", name: "Cursor", kind: "editor", installed: true, appPath: "/Applications/Cursor.app" },
  { id: "terminal", name: "Terminal", kind: "terminal", installed: true, appPath: "/System/Applications/Utilities/Terminal.app" },
  { id: "finder", name: "Finder", kind: "terminal", installed: true, appPath: "/System/Library/CoreServices/Finder.app" },
];

const moves: MoveRecord[] = [];

function list(): ProjectList {
  const groups = config.groups.map((g) => ({
    ...g,
    exists: true,
    count: projects.filter((p) => p.group === g.name).length,
  }));
  return { projects, groups, warnings: [], root: config.projectsRoot };
}

const handlers: Record<string, (args: any) => unknown> = {
  get_config: () => config,
  save_settings: ({ patch }) => Object.assign(config, patch),
  complete_onboarding: ({ input }) => Object.assign(config, input, { onboarded: true }),
  suggest_scan_folders: () => [
    { path: "/Users/you/Old Projects", displayPath: "~/Old Projects", projectCount: 12, examples: ["old-blog", "scratchpad", "todo-app", "chat"] },
    { path: "/Users/you/Desktop/code", displayPath: "~/Desktop/code", projectCount: 3, examples: ["a", "b", "c"] },
  ],
  list_projects: () => list(),
  project_size: () => 48 * 1024 * 1024,
  list_editors: () => editors,
  gh_status: () => ({ installed: true, loggedIn: true, user: "you", gitInstalled: true, brewInstalled: true }),
  preview_move: ({ path, group, newName }) => {
    const name = newName || path.split("/").pop();
    const to = `${config.projectsRoot}${group ? `/${group}` : ""}/${name}`;
    return {
      from: path,
      to,
      toDisplay: to.replace("/Users/you", "~"),
      ok: true,
      blockers: [],
      warnings: ["VS Code's recent-files list and open tabs are tied to the old path."],
    };
  },
  list_moves: () => [...moves],
  move_project: ({ path, group, newName }) => {
    const p = projects.find((p) => p.path === path);
    if (!p) throw new Error("Project not found");
    const to = `${config.projectsRoot}${group ? `/${group}` : ""}/${newName || p.name}`;
    const record = { id: String(Date.now()), name: p.name, from: path, to, at: Date.now() / 1000, undone: false };
    Object.assign(p, { path: to, id: to, displayPath: to.replace("/Users/you", "~"), group, location: "root" });
    moves.push(record);
    return record;
  },
  undo_move: ({ id }) => {
    const record = moves.find((m) => m.id === id);
    if (!record || record.undone) throw new Error("Move cannot be undone");
    const p = projects.find((p) => p.path === record.to);
    if (!p) throw new Error("Project not found");
    Object.assign(p, { path: record.from, id: record.from, displayPath: record.from.replace("/Users/you", "~"), group: null, location: "external" });
    record.undone = true;
    return { ...record };
  },
  preview_park: ({ path }) => ({ path, sizeBytes: 48 * 1024 ** 2, fileCount: 1264, warnings: ["Save every editor buffer and close processes before continuing. Unsaved buffers are not included. Editor tabs are not guaranteed to restore."] }),
  park_project: ({ path, passphrase }) => {
    if (!config.vaultRepo) throw new Error("Configure a private vault repository first");
    if (typeof passphrase !== "string" || passphrase.length < 16) throw new Error("Passphrase is too short");
    const p = projects.find((p) => p.path === path);
    if (!p || p.status !== "local") throw new Error("Local project not found");
    Object.assign(p, { status: "parked", parkedAt: Date.now() / 1000, sizeBytes: 48 * 1024 ** 2 });
    return { ...p };
  },
  restore_project: ({ path, passphrase }) => {
    if (!passphrase) throw new Error("Enter a passphrase");
    const p = projects.find((p) => p.path === path);
    if (!p || p.status !== "parked") throw new Error("Parked project not found");
    Object.assign(p, { status: "local", parkedAt: null });
  },
  create_project: ({ req }) => {
    const p = proj(req.name, req.group, [], { path: `${config.projectsRoot}${req.group ? `/${req.group}` : ""}/${req.name}`, location: "root" });
    p.id = p.path;
    p.displayPath = p.path.replace("/Users/you", "~");
    projects.push(p);
    return { path: p.path, githubSlug: req.github !== "none" ? `you/${req.name}` : null, steps: ["Created folder", "Initialized git repo"], warnings: [] };
  },
};

export async function mockInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  await new Promise((r) => setTimeout(r, 60));
  if (params.get("fail") === cmd) throw new Error(`Preview simulation: ${cmd} failed. No files were changed.`);
  if (["park_project", "restore_project"].includes(cmd)) await new Promise((r) => setTimeout(r, 900));
  const h = handlers[cmd];
  if (!h) {
    // Never log command arguments: they may contain a snapshot passphrase.
    console.info(`[mock] ${cmd}`);
    return undefined as T;
  }
  return h(args ?? {}) as T;
}

export const mockState = { config, projects, editors };

/** `?view=unsorted`, `?open=new`, `?open=move` etc. drive screenshot scenes. */
export const mockScene = {
  view: params.get("view"),
  open: params.get("open"),
};

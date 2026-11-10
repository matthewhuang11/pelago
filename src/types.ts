export interface GitInfo {
  branch: string | null;
  hasCommits: boolean;
  remoteUrl: string | null;
  githubSlug: string | null;
  dirty: number;
  ahead: number;
  behind: number;
  hasUpstream: boolean;
  lastCommit: number | null;
}

export interface Project {
  id: string;
  name: string;
  path: string;
  displayPath: string;
  group: string | null;
  location: "root" | "external";
  status: "local" | "parked";
  git: GitInfo | null;
  stack: string[];
  empty: boolean;
  nestedRepos: string[];
  duplicates: string[];
  lastActivity: number;
  pinned: boolean;
  editor: string | null;
  lastOpened: number | null;
  suggestedGroup: string | null;
  sizeBytes: number | null;
  parkedAt: number | null;
}

export interface GroupInfo {
  name: string;
  emoji: string;
  color: string;
  exists: boolean;
  count: number;
}

export interface Warning {
  kind: "emptyOuterRepo" | "missingScanPath" | "missingRoot" | string;
  path: string;
  message: string;
}

export interface ProjectList {
  projects: Project[];
  groups: GroupInfo[];
  warnings: Warning[];
  root: string;
}

export interface GroupMeta {
  name: string;
  emoji: string;
  color: string;
}

export interface CustomEditor {
  id: string;
  name: string;
  command: string;
}

export interface NewProjectDefaults {
  gitInit: boolean;
  github: "none" | "private" | "public";
  openAfter: boolean;
  template: string;
}

export interface Config {
  version: number;
  onboarded: boolean;
  projectsRoot: string;
  scanPaths: string[];
  defaultEditor: string;
  groups: GroupMeta[];
  projects: Record<string, { pinned: boolean; editor: string | null; lastOpened: number | null }>;
  customEditors: CustomEditor[];
  vaultRepo: string | null;
  staleDays: number;
  newProjectDefaults: NewProjectDefaults;
}

export interface Editor {
  id: string;
  name: string;
  kind: "editor" | "terminal" | "custom";
  installed: boolean;
  appPath: string | null;
}

export interface GhStatus {
  installed: boolean;
  loggedIn: boolean;
  user: string | null;
  gitInstalled: boolean;
  brewInstalled: boolean;
}

export interface FolderSuggestion {
  path: string;
  displayPath: string;
  projectCount: number;
  examples: string[];
}

export type View =
  | { kind: "home" }
  | { kind: "pinned" }
  | { kind: "group"; name: string }
  | { kind: "unsorted" }
  | { kind: "parked" }
  | { kind: "settings" };

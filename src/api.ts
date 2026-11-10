import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import { open as openDialog } from "@tauri-apps/plugin-dialog";
import type {
  Config,
  Editor,
  FolderSuggestion,
  GhStatus,
  GroupMeta,
  ProjectList,
  CustomEditor,
  NewProjectDefaults,
} from "./types";

export const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

async function invoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  if (inTauri) return tauriInvoke<T>(cmd, args);
  const { mockInvoke } = await import("./mock");
  return mockInvoke<T>(cmd, args);
}

export interface SettingsPatch {
  projectsRoot?: string;
  scanPaths?: string[];
  defaultEditor?: string;
  staleDays?: number;
  vaultRepo?: string | null;
  customEditors?: CustomEditor[];
  newProjectDefaults?: NewProjectDefaults;
  groups?: GroupMeta[];
}

export const api = {
  getConfig: () => invoke<Config>("get_config"),
  saveSettings: (patch: SettingsPatch) => invoke<Config>("save_settings", { patch }),
  completeOnboarding: (input: {
    projectsRoot: string;
    scanPaths: string[];
    defaultEditor: string;
    groups: GroupMeta[];
  }) => invoke<Config>("complete_onboarding", { input }),
  suggestScanFolders: () => invoke<FolderSuggestion[]>("suggest_scan_folders"),
  listProjects: () => invoke<ProjectList>("list_projects"),
  projectSize: (path: string) => invoke<number>("project_size", { path }),
  openProject: (path: string, editor?: string) => invoke<void>("open_project", { path, editor }),
  revealInFinder: (path: string) => invoke<void>("reveal_in_finder", { path }),
  openUrl: (url: string) => invoke<void>("open_url", { url }),
  setPinned: (path: string, pinned: boolean) => invoke<void>("set_pinned", { path, pinned }),
  setProjectEditor: (path: string, editor: string | null) =>
    invoke<void>("set_project_editor", { path, editor }),
  listEditors: (refresh = false) => invoke<Editor[]>("list_editors", { refresh }),
  ghStatus: () => invoke<GhStatus>("gh_status"),
  ghLogin: () => invoke<void>("gh_login"),
};

export async function pickFolder(title: string, defaultPath?: string): Promise<string | null> {
  if (!inTauri) return window.prompt(title, defaultPath ?? "") || null;
  const picked = await openDialog({ directory: true, multiple: false, title, defaultPath });
  return typeof picked === "string" ? picked : null;
}

export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return JSON.stringify(e);
}

import { invoke } from "@tauri-apps/api/core";
import type {
  AppEntry,
  ApplyResult,
  ClipEntry,
  Config,
  Graph,
  HotkeyValidation,
  InstalledExtension,
  MemoryFile,
  MemoryHit,
  MonitorInfo,
  NewTask,
  NoteDetail,
  PathsInfo,
  ProjectInfo,
  RunResult,
  RuntimeStatus,
  SearchResults,
  StatusInfo,
  StoreItem,
  Task,
  TaskPatch,
  WindowInfo,
  WindowLayout,
  WindowRule,
} from "./types";

/**
 * Ponte tipada para os comandos Tauri do backend Rust.
 * Os nomes dos argumentos seguem camelCase (Tauri converte de snake_case).
 */
export const api = {
  version: () => invoke<string>("app_version"),
  getConfig: () => invoke<Config>("get_config"),
  getPaths: () => invoke<PathsInfo>("get_paths"),
  getStatus: () => invoke<StatusInfo>("get_status"),
  setConfig: (patch: unknown) => invoke<Config>("set_config", { patch }),
  setOnboardingDone: () => invoke<Config>("set_onboarding_done"),
  applyLauncherSettings: (mode: "replace" | "off", hotkey: string) =>
    invoke<Config>("apply_launcher_settings", { mode, hotkey }),
  validateHotkey: (
    combo: string,
    excludeLayoutId: number | null = null,
    excludeLauncher = false,
  ) =>
    invoke<HotkeyValidation>("validate_hotkey", {
      combo,
      excludeLayoutId,
      excludeLauncher,
    }),
  suspendHotkeys: () => invoke<void>("suspend_hotkeys"),
  resumeHotkeys: () => invoke<void>("resume_hotkeys"),

  search: (query: string, limit = 12) =>
    invoke<SearchResults>("search", { query, limit }),
  searchApps: (query: string, limit = 8) =>
    invoke<AppEntry[]>("search_apps", { query, limit }),
  listInstalledApps: () => invoke<AppEntry[]>("list_installed_apps"),
  launchApp: (exePath: string) => invoke<void>("launch_app", { exePath }),
  openPath: (path: string) => invoke<void>("open_path", { path }),
  rebuildAppsIndex: () => invoke<number>("rebuild_apps_index"),
  rebuildFilesIndex: () => invoke<void>("rebuild_files_index"),

  copyText: (text: string) => invoke<void>("copy_text", { text }),
  listClipboard: (limit = 50) => invoke<ClipEntry[]>("list_clipboard", { limit }),
  deleteClipboard: (id: number) => invoke<void>("delete_clipboard", { id }),
  pinClipboard: (id: number, pinned: boolean) =>
    invoke<void>("pin_clipboard", { id, pinned }),

  listExtensions: () => invoke<InstalledExtension[]>("list_extensions"),
  installExtension: (request: { kind: string; value: string }) =>
    invoke<InstalledExtension>("install_extension", { request }),
  uninstallExtension: (extId: string) =>
    invoke<void>("uninstall_extension", { extId }),
  setExtensionEnabled: (extId: string, enabled: boolean) =>
    invoke<void>("set_extension_enabled", { extId, enabled }),
  setExtensionPermissions: (extId: string, permissions: string[]) =>
    invoke<InstalledExtension>("set_extension_permissions", {
      extId,
      permissions,
    }),
  runExtensionCommand: (
    extId: string,
    commandId: string,
    args: unknown = {},
  ) =>
    invoke<RunResult>("run_extension_command", { extId, commandId, args }),
  runtimeStatus: () => invoke<RuntimeStatus>("runtime_status"),
  ensureRuntime: () => invoke<RuntimeStatus>("ensure_runtime"),
  listStore: (url: string | null = null) =>
    invoke<StoreItem[]>("list_store", { url }),

  reindexMemory: () => invoke<number>("reindex_memory"),
  listProjects: () => invoke<ProjectInfo[]>("list_projects"),
  createProject: (slug: string, title: string) =>
    invoke<ProjectInfo>("create_project", { slug, title }),
  listMemoryFiles: (project: string | null) =>
    invoke<MemoryFile[]>("list_memory_files", { project }),
  readNote: (path: string) => invoke<NoteDetail>("read_note", { path }),
  saveNote: (path: string, title: string, tags: string[], body: string) =>
    invoke<NoteDetail>("save_note", { path, title, tags, body }),
  createNote: (project: string | null, title: string) =>
    invoke<NoteDetail>("create_note", { project, title }),
  deleteMemoryFile: (path: string) =>
    invoke<void>("delete_memory_file", { path }),
  searchMemory: (
    project: string | null,
    query: string,
    tags: string[] = [],
    limit = 50,
  ) =>
    invoke<MemoryHit[]>("search_memory", { project, query, tags, limit }),
  getGraph: (project: string | null) =>
    invoke<Graph>("get_graph", { project }),
  listTasks: (
    project: string | null,
    status: string | null = null,
    query: string | null = null,
  ) => invoke<Task[]>("list_tasks", { project, status, query }),
  createTask: (input: NewTask) => invoke<Task>("create_task", { input }),
  updateTask: (id: string, patch: TaskPatch) =>
    invoke<Task>("update_task", { id, patch }),
  deleteTask: (id: string) => invoke<void>("delete_task", { id }),

  listOpenWindows: () => invoke<WindowInfo[]>("list_open_windows"),
  listWindowsWithMonitor: () =>
    invoke<WindowInfo[]>("list_windows_with_monitor"),
  listMonitors: () => invoke<MonitorInfo[]>("list_monitors"),
  listWindowLayouts: () => invoke<WindowLayout[]>("list_window_layouts"),
  saveCurrentLayout: (name: string, hotkey: string | null) =>
    invoke<WindowLayout>("save_current_layout", { name, hotkey }),
  deleteWindowLayout: (id: number) =>
    invoke<void>("delete_window_layout", { id }),
  applyWindowLayout: (id: number) =>
    invoke<ApplyResult>("apply_window_layout", { id }),
  setWindowLayoutHotkey: (id: number, hotkey: string | null) =>
    invoke<void>("set_window_layout_hotkey", { id, hotkey }),
  listWindowRules: () => invoke<WindowRule[]>("list_window_rules"),
  upsertWindowRule: (rule: WindowRule) =>
    invoke<number>("upsert_window_rule", { rule }),
  deleteWindowRule: (id: number) => invoke<void>("delete_window_rule", { id }),
  applyWindowRules: () => invoke<ApplyResult>("apply_window_rules"),

  showLauncher: () => invoke<void>("show_launcher"),
  hideLauncher: () => invoke<void>("hide_launcher"),
  toggleLauncher: () => invoke<void>("toggle_launcher"),
  quit: () => invoke<void>("quit_app"),
};

export type {
  AppEntry,
  ApplyResult,
  ClipEntry,
  Config,
  Graph,
  HotkeyValidation,
  InstalledExtension,
  MemoryFile,
  MemoryHit,
  MonitorInfo,
  NewTask,
  NoteDetail,
  PathsInfo,
  ProjectInfo,
  RunResult,
  RuntimeStatus,
  SearchResults,
  StatusInfo,
  StoreItem,
  Task,
  TaskPatch,
  WindowInfo,
  WindowLayout,
  WindowRule,
};

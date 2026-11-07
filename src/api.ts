import { invoke } from "@tauri-apps/api/core";
import type {
  AppEntry,
  ClipEntry,
  Config,
  PathsInfo,
  SearchResults,
  StatusInfo,
  WindowInfo,
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

  listOpenWindows: () => invoke<WindowInfo[]>("list_open_windows"),

  showLauncher: () => invoke<void>("show_launcher"),
  hideLauncher: () => invoke<void>("hide_launcher"),
  toggleLauncher: () => invoke<void>("toggle_launcher"),
  quit: () => invoke<void>("quit_app"),
};

export type {
  AppEntry,
  ClipEntry,
  Config,
  PathsInfo,
  SearchResults,
  StatusInfo,
  WindowInfo,
};

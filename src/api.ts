import { invoke } from "@tauri-apps/api/core";
import type {
  AppEntry,
  Config,
  PathsInfo,
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

  searchApps: (query: string, limit = 8) =>
    invoke<AppEntry[]>("search_apps", { query, limit }),
  listInstalledApps: () => invoke<AppEntry[]>("list_installed_apps"),
  launchApp: (exePath: string) => invoke<void>("launch_app", { exePath }),
  rebuildAppsIndex: () => invoke<number>("rebuild_apps_index"),

  listOpenWindows: () => invoke<WindowInfo[]>("list_open_windows"),
  getWindowList: () => invoke<WindowInfo[]>("list_open_windows"),

  showLauncher: () => invoke<void>("show_launcher"),
  hideLauncher: () => invoke<void>("hide_launcher"),
  toggleLauncher: () => invoke<void>("toggle_launcher"),
  quit: () => invoke<void>("quit_app"),
};

export type { AppEntry, Config, PathsInfo, StatusInfo, WindowInfo };

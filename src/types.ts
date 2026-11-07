export interface AppEntry {
  name: string;
  exePath: string;
  iconPath: string | null;
  keywords: string[];
}

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface WindowInfo {
  handle: number;
  process: string;
  title: string;
  class: string | null;
  monitor: number;
  rect: Rect;
}

export interface PathsInfo {
  config: string;
  db: string;
  workspace: string;
}

export interface StatusInfo {
  version: string;
  appsIndexed: number;
  openWindows: number;
  paths: PathsInfo;
}

export type StartMenuMode = "ask" | "replace" | "win_x";

export interface Config {
  version: number;
  onboardingDone: boolean;
  startMenu: { mode: StartMenuMode; fallback: string };
  hotkeys: { launcher: string; windowLayouts: Record<string, string> };
  workspace: { path: string; activeProject: string | null };
  search: {
    includeApps: boolean;
    includeFiles: boolean;
    indexDirs: string[];
    excludePatterns: string[];
    maxResults: number;
  };
  tray: { showOnClose: boolean; startOnBoot: boolean; hideOnBlur: boolean };
  mcp: {
    enabled: boolean;
    stdio: { enabled: boolean };
    http: { enabled: boolean; host: string; port: number };
  };
  appearance: { theme: string };
}

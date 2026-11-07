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

export interface FileEntry {
  path: string;
  name: string;
  ext: string | null;
  isDir: boolean;
}

export interface ClipEntry {
  id: number;
  kind: string;
  text: string | null;
  copiedAt: number;
  pinned: boolean;
}

export interface CommandEntry {
  extId: string;
  commandId: string;
  title: string;
  subtitle: string | null;
  icon: string | null;
}

export interface SearchResults {
  apps: AppEntry[];
  files: FileEntry[];
  clipboard: ClipEntry[];
  commands: CommandEntry[];
  calculation: string | null;
}

export interface PathsInfo {
  config: string;
  db: string;
  workspace: string;
}

export interface StatusInfo {
  version: string;
  appsIndexed: number;
  filesIndexed: number;
  clipboardCount: number;
  extensionsCount: number;
  openWindows: number;
  winHookActive: boolean;
  paths: PathsInfo;
}

export interface ManifestCommand {
  id: string;
  title: string;
  subtitle: string | null;
  keywords: string[];
  mode: string;
  icon: string | null;
}

export interface InstalledExtension {
  id: string;
  name: string;
  version: string;
  description: string;
  author: string;
  installSource: string;
  installPath: string;
  enabled: boolean;
  permissions: string[];
  granted: string[];
  pending: boolean;
  commands: ManifestCommand[];
}

export interface RunResult {
  kind: string;
  text: string | null;
}

export interface RuntimeStatus {
  available: boolean;
  path: string | null;
  version: string | null;
}

export interface StoreItem {
  id: string;
  name: string;
  version: string;
  description: string;
  url: string;
  source: string;
}

export interface MonitorInfo {
  index: number;
  name: string;
  x: number;
  y: number;
  width: number;
  height: number;
  workX: number;
  workY: number;
  workWidth: number;
  workHeight: number;
  primary: boolean;
}

export interface LayoutSlot {
  app: string | null;
  matchField: string;
  monitor: number;
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface WindowLayout {
  id: number;
  name: string;
  hotkey: string | null;
  slots: LayoutSlot[];
}

export interface WindowRule {
  id: number;
  name: string;
  appMatch: string;
  matchField: string;
  monitor: number;
  x: number;
  y: number;
  w: number;
  h: number;
  enabled: boolean;
}

export interface ApplyResult {
  moved: number;
  missed: string[];
}

export interface HotkeyValidation {
  valid: boolean;
  conflict: string | null;
}

export type StartMenuMode = "ask" | "replace" | "off";

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
  clipboard: { enabled: boolean; maxEntries: number };
  windows: { autoApplyRules: boolean };
  tray: { showOnClose: boolean; startOnBoot: boolean; hideOnBlur: boolean };
  mcp: {
    enabled: boolean;
    stdio: { enabled: boolean };
    http: { enabled: boolean; host: string; port: number };
  };
  extensions: { storeUrl: string; denoPath: string | null };
  appearance: { theme: string };
}

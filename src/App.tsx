import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import "./App.css";
import { api } from "./api";
import { Clipboard } from "./Clipboard";
import { Extensions } from "./Extensions";
import { HotkeyInput } from "./HotkeyInput";
import { Icon, type IconName } from "./Icons";
import { Mcp } from "./Mcp";
import { Memory } from "./Memory";
import { Select } from "./Select";
import { WindowManager } from "./WindowManager";
import { useI18n } from "./i18n";
import { ACCENTS, normalizeAccent } from "./theme";
import type { Config, SearchResults, StatusInfo } from "./types";

type Screen =
  | "search"
  | "onboarding"
  | "settings"
  | "extensions"
  | "windows"
  | "memory"
  | "clipboard"
  | "mcp";

const SCREEN_SIZE: Record<Screen, [number, number]> = {
  search: [760, 460],
  onboarding: [760, 460],
  settings: [900, 680],
  extensions: [900, 640],
  windows: [900, 640],
  memory: [1120, 740],
  clipboard: [900, 640],
  mcp: [900, 680],
};

interface SettingsDraft {
  hideOnBlur: boolean;
  startOnBoot: boolean;
  theme: string;
  accent: string;
  language: string;
  clipboardEnabled: boolean;
  clipboardMax: number;
  includeApps: boolean;
  includeFiles: boolean;
  indexDirs: string[];
  searchMaxResults: number;
  autoApplyRules: boolean;
  restoreLastLayout: boolean;
  mcpEnabled: boolean;
  mcpPort: number;
  storeUrl: string;
  denoPath: string;
}

const DEFAULT_SETTINGS: SettingsDraft = {
  hideOnBlur: true,
  startOnBoot: false,
  theme: "dark",
  accent: "teal",
  language: "en",
  clipboardEnabled: true,
  clipboardMax: 500,
  includeApps: true,
  includeFiles: true,
  indexDirs: [],
  searchMaxResults: 8,
  autoApplyRules: true,
  restoreLastLayout: false,
  mcpEnabled: true,
  mcpPort: 19000,
  storeUrl: "",
  denoPath: "",
};

type Row =
  | { kind: "calculator"; key: string; title: string; subtitle: string; value: string }
  | {
      kind: "app";
      key: string;
      title: string;
      subtitle: string;
      path: string;
      icon: string | null;
    }
  | { kind: "file"; key: string; title: string; subtitle: string; path: string }
  | {
      kind: "clipboard";
      key: string;
      title: string;
      subtitle: string;
      text: string;
      id: number;
      isImage: boolean;
    }
  | { kind: "web"; key: string; title: string; subtitle: string; url: string }
  | {
      kind: "command";
      key: string;
      title: string;
      subtitle: string;
      extId: string;
      commandId: string;
    };

const WEB_ACTIONS: Array<{
  prefix: string;
  label: string;
  build: (term: string) => string;
}> = [
  {
    prefix: "g",
    label: "Search Google",
    build: (term) => `https://www.google.com/search?q=${encodeURIComponent(term)}`,
  },
  {
    prefix: "yt",
    label: "Search YouTube",
    build: (term) =>
      `https://www.youtube.com/results?search_query=${encodeURIComponent(term)}`,
  },
  {
    prefix: "gh",
    label: "Search GitHub",
    build: (term) => `https://github.com/search?q=${encodeURIComponent(term)}`,
  },
  {
    prefix: "w",
    label: "Search Wikipedia",
    build: (term) =>
      `https://pt.wikipedia.org/w/index.php?search=${encodeURIComponent(term)}`,
  },
  {
    prefix: "ddg",
    label: "Search DuckDuckGo",
    build: (term) => `https://duckduckgo.com/?q=${encodeURIComponent(term)}`,
  },
  {
    prefix: "maps",
    label: "Open in Google Maps",
    build: (term) =>
      `https://www.google.com/maps/search/?api=1&query=${encodeURIComponent(term)}`,
  },
];

function webAction(
  query: string,
  t: (key: string) => string,
): Row | null {
  const trimmed = query.trim();
  const lower = trimmed.toLowerCase();
  for (const action of WEB_ACTIONS) {
    if (!lower.startsWith(`${action.prefix} `)) continue;
    const term = trimmed.slice(action.prefix.length).trim();
    if (!term) return null;
    const url = action.build(term);
    return {
      kind: "web",
      key: `web:${action.prefix}:${term}`,
      title: `${t(action.label)}: ${term}`,
      subtitle: url,
      url,
    };
  }
  return null;
}

function badgeLabel(row: Row): string {
  switch (row.kind) {
    case "app":
      return row.title.trim().charAt(0).toUpperCase() || "A";
    case "file": {
      const dot = row.title.lastIndexOf(".");
      if (dot > 0 && dot < row.title.length - 1) {
        const extension = row.title.slice(dot + 1);
        if (extension.length <= 3) return extension.toUpperCase();
      }
      return "F";
    }
    case "clipboard":
      return "C";
    case "calculator":
      return "=";
    case "web":
      return "↗";
    case "command":
      return "›";
  }
}

function rowGlyph(kind: Row["kind"]): IconName | null {
  switch (kind) {
    case "clipboard":
      return "clipboard";
    case "calculator":
      return "calculator";
    case "web":
      return "globe";
    case "command":
      return "command";
    default:
      return null;
  }
}

function rowKindLabel(kind: Row["kind"], t: (key: string) => string): string {
  switch (kind) {
    case "app":
      return t("App");
    case "file":
      return t("File");
    case "clipboard":
      return t("Clipboard");
    case "calculator":
      return t("Calculator");
    case "web":
      return t("Web");
    case "command":
      return t("Command");
  }
}

function rowActionLabel(kind: Row["kind"], t: (key: string) => string): string {
  switch (kind) {
    case "app":
    case "file":
      return t("Open");
    case "clipboard":
      return t("Copy");
    case "calculator":
      return t("Copy result");
    case "web":
      return t("Open in browser");
    case "command":
      return t("Run");
  }
}

function compactCount(value: number, language: string): string {
  return new Intl.NumberFormat(language === "pt-BR" ? "pt-BR" : "en", {
    notation: "compact",
    maximumFractionDigits: 1,
  }).format(value);
}

function countLabel(value: number, language: string): string {
  return value >= 1000 ? compactCount(value, language) : String(value);
}

function App() {
  const { t, language, setLanguage } = useI18n();
  const [screen, setScreen] = useState<Screen>("search");
  const [config, setConfig] = useState<Config | null>(null);
  const [status, setStatus] = useState<StatusInfo | null>(null);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResults | null>(null);
  const [selected, setSelected] = useState(0);
  const [loading, setLoading] = useState(true);
  const [toast, setToast] = useState<string | null>(null);
  const [settingsSection, setSettingsSection] = useState("launcher");

  const [mode, setMode] = useState<"off" | "replace">("off");
  const [hotkey, setHotkey] = useState("Alt+Space");
  const [settingsMsg, setSettingsMsg] = useState<string | null>(null);
  const [settingsBusy, setSettingsBusy] = useState(false);
  const [settings, setSettings] = useState<SettingsDraft>(DEFAULT_SETTINGS);
  const [newIndexDir, setNewIndexDir] = useState("");

  const updateRef = useRef<Update | null>(null);
  const [updateStatus, setUpdateStatus] = useState<
    "idle" | "checking" | "latest" | "available" | "downloading" | "ready" | "error"
  >("idle");
  const [updateInfo, setUpdateInfo] = useState<{
    version: string;
    notes: string | null;
    date: string | null;
  } | null>(null);
  const [updateProgress, setUpdateProgress] = useState(0);
  const [updateMessage, setUpdateMessage] = useState<string | null>(null);

  const inputRef = useRef<HTMLInputElement>(null);
  const resultsRef = useRef<HTMLUListElement>(null);
  const settingsContentRef = useRef<HTMLDivElement>(null);

  const refreshStatus = useCallback(async () => {
    try {
      setStatus(await api.getStatus());
    } catch (error) {
      console.error("Falha ao atualizar status:", error);
    }
  }, []);

  const applyConfig = useCallback((cfg: Config) => {
    setConfig(cfg);
    document.documentElement.dataset.accent = normalizeAccent(
      cfg.appearance.accent,
    );
    setMode(cfg.startMenu.mode === "replace" ? "replace" : "off");
    setHotkey(cfg.hotkeys.launcher);
    setLanguage(cfg.appearance.language || "en");
    setSettings({
      hideOnBlur: cfg.tray.hideOnBlur,
      startOnBoot: cfg.tray.startOnBoot,
      theme: cfg.appearance.theme || "dark",
      accent: normalizeAccent(cfg.appearance.accent),
      language: cfg.appearance.language || "en",
      clipboardEnabled: cfg.clipboard.enabled,
      clipboardMax: cfg.clipboard.maxEntries,
      includeApps: cfg.search.includeApps,
      includeFiles: cfg.search.includeFiles,
      indexDirs: [...cfg.search.indexDirs],
      searchMaxResults: cfg.search.maxResults,
      autoApplyRules: cfg.windows.autoApplyRules,
      restoreLastLayout: cfg.windows.restoreLastLayout ?? false,
      mcpEnabled: cfg.mcp.enabled,
      mcpPort: cfg.mcp.http.port,
      storeUrl: cfg.extensions.storeUrl,
      denoPath: cfg.extensions.denoPath ?? "",
    });
  }, [setLanguage]);

  const openScreen = useCallback((target: Screen) => {
    setScreen((current) => (current === target ? "search" : target));
  }, []);

  const settingsDirty = useMemo(() => {
    if (!config) return false;
    return (
      mode !== (config.startMenu.mode === "replace" ? "replace" : "off") ||
      hotkey.trim() !== config.hotkeys.launcher.trim() ||
      settings.hideOnBlur !== config.tray.hideOnBlur ||
      settings.startOnBoot !== config.tray.startOnBoot ||
      settings.theme !== config.appearance.theme ||
      normalizeAccent(settings.accent) !==
        normalizeAccent(config.appearance.accent) ||
      settings.language !== config.appearance.language ||
      settings.clipboardEnabled !== config.clipboard.enabled ||
      Number(settings.clipboardMax) !== config.clipboard.maxEntries ||
      settings.includeApps !== config.search.includeApps ||
      settings.includeFiles !== config.search.includeFiles ||
      settings.indexDirs.join("|") !== config.search.indexDirs.join("|") ||
      Number(settings.searchMaxResults) !== config.search.maxResults ||
      settings.autoApplyRules !== config.windows.autoApplyRules ||
      settings.restoreLastLayout !==
        (config.windows.restoreLastLayout ?? false) ||
      settings.mcpEnabled !== config.mcp.enabled ||
      Number(settings.mcpPort) !== config.mcp.http.port ||
      settings.storeUrl.trim() !== config.extensions.storeUrl.trim() ||
      settings.denoPath.trim() !== (config.extensions.denoPath ?? "").trim()
    );
  }, [config, mode, hotkey, settings]);

  const navItems = useMemo<
    Array<{ key: Screen; label: string; icon: IconName }>
  >(
    () => [
      { key: "clipboard", label: t("Clipboard"), icon: "clipboard" },
      { key: "mcp", label: "MCP", icon: "mcp" },
      { key: "memory", label: t("Memory & Tasks"), icon: "memory" },
      { key: "windows", label: t("Window manager"), icon: "windows" },
      { key: "extensions", label: t("Extensions"), icon: "extensions" },
      { key: "settings", label: t("Settings"), icon: "settings" },
    ],
    [t],
  );

  const settingsSections = useMemo(
    () => [
      { id: "launcher", label: t("Launcher") },
      { id: "appearance", label: t("Appearance") },
      { id: "updates", label: t("Updates") },
      { id: "search", label: t("Search & index") },
      { id: "clipboard", label: t("Clipboard") },
      { id: "windows", label: t("Windows") },
      { id: "mcp", label: "MCP" },
      { id: "extensions", label: t("Extensions") },
    ],
    [t],
  );

  const load = useCallback(async () => {
    try {
      const [cfg, st] = await Promise.all([api.getConfig(), api.getStatus()]);
      applyConfig(cfg);
      setStatus(st);
      setScreen(cfg.onboardingDone ? "search" : "onboarding");
    } catch (error) {
      console.error("Falha ao carregar estado:", error);
    } finally {
      setLoading(false);
    }
  }, [applyConfig]);

  useEffect(() => {
    void load();
  }, [load]);

  useEffect(() => {
    const media = window.matchMedia("(prefers-color-scheme: light)");
    const apply = () => {
      const resolved =
        settings.theme === "auto"
          ? media.matches
            ? "light"
            : "dark"
          : settings.theme;
      document.documentElement.dataset.theme =
        resolved === "light" ? "light" : "dark";
      document.documentElement.dataset.accent = normalizeAccent(
        settings.accent,
      );
    };
    apply();
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [settings.theme, settings.accent]);

  // O config.json também muda por fora (ex.: MCP vinculando um projeto).
  useEffect(() => {
    let active = true;
    const unlistenPromise = listen("config-changed", () => {
      if (!active) return;
      void (async () => {
        try {
          const [cfg, st] = await Promise.all([api.getConfig(), api.getStatus()]);
          if (!active) return;
          setStatus(st);
          // Não sobrescreve edições ainda não salvas na tela de configurações.
          if (screen !== "settings") applyConfig(cfg);
        } catch (error) {
          console.error("Falha ao recarregar configuração:", error);
        }
      })();
    });

    return () => {
      active = false;
      void unlistenPromise.then((unlisten) => unlisten()).catch(() => {});
    };
  }, [applyConfig, screen]);

  useEffect(() => {
    if (screen === "search") {
      inputRef.current?.focus();
    }
  }, [screen]);

  // Mantém o item selecionado visível quando a navegação é feita pelo teclado.
  useEffect(() => {
    const node = resultsRef.current?.querySelector<HTMLElement>(
      `[data-result-index="${selected}"]`,
    );
    node?.scrollIntoView({ block: "nearest" });
  }, [selected]);

  // Destaca a seção visível no índice de configurações.
  useEffect(() => {
    if (screen !== "settings") return;
    const container = settingsContentRef.current;
    if (!container) return;
    const update = () => {
      const sections = [
        ...container.querySelectorAll<HTMLElement>("[data-settings-section]"),
      ];
      const threshold = container.getBoundingClientRect().top + 28;
      let current = sections[0]?.dataset.settingsSection;
      for (const section of sections) {
        if (section.getBoundingClientRect().top <= threshold) {
          current = section.dataset.settingsSection;
        }
      }
      if (current) setSettingsSection(current);
    };
    update();
    container.addEventListener("scroll", update, { passive: true });
    return () => container.removeEventListener("scroll", update);
  }, [screen]);

  // A tela de memória precisa de mais espaço.
  useEffect(() => {
    const window = getCurrentWindow();
    const [width, height] = SCREEN_SIZE[screen];
    void window
      .setSize(new LogicalSize(width, height))
      .then(() => window.center())
      .catch(() => {});
  }, [screen]);

  useEffect(() => {
    const onFocus = () => {
      if (screen === "search") inputRef.current?.focus();
    };
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [screen]);

  // Busca com debounce curto.
  useEffect(() => {
    if (screen !== "search") return;
    const timer = window.setTimeout(() => {
      api
        .search(query, 12)
        .then((res) => {
          setResults(res);
          setSelected(0);
        })
        .catch((error) => console.error("Falha na busca:", error));
    }, 80);
    return () => window.clearTimeout(timer);
  }, [query, screen]);

  // Toast some sozinho.
  useEffect(() => {
    if (!toast) return;
    const timer = window.setTimeout(() => setToast(null), 4500);
    return () => window.clearTimeout(timer);
  }, [toast]);

  const rows = useMemo<Row[]>(() => {
    const out: Row[] = [];
    const web = webAction(query, t);
    if (web) out.push(web);
    if (!results) return out;

    if (results.calculation) {
      out.push({
        kind: "calculator",
        key: "calc",
        title: results.calculation,
        subtitle: t("Copy result"),
        value: results.calculation,
      });
    }
    for (const app of results.apps) {
      out.push({
        kind: "app",
        key: `app:${app.exePath}`,
        title: app.name,
        subtitle: app.exePath,
        path: app.exePath,
        icon: app.iconPath,
      });
    }
    for (const file of results.files) {
      out.push({
        kind: "file",
        key: `file:${file.path}`,
        title: file.name,
        subtitle: file.path,
        path: file.path,
      });
    }
    for (const clip of results.clipboard) {
      const isImage = clip.kind === "image";
      const text = (clip.text ?? "").replace(/\s+/g, " ").trim();
      out.push({
        kind: "clipboard",
        key: `clip:${clip.id}`,
        title: isImage
          ? t("Copied image")
          : text.length > 90
            ? `${text.slice(0, 90)}…`
            : text || t("(empty)"),
        subtitle: isImage
          ? t("Clipboard · image")
          : t("Clipboard"),
        text: clip.text ?? "",
        id: clip.id,
        isImage,
      });
    }
    for (const command of results.commands) {
      out.push({
        kind: "command",
        key: `cmd:${command.extId}:${command.commandId}`,
        title: command.title,
        subtitle:
          command.subtitle ?? `${command.extId} › ${command.commandId}`,
        extId: command.extId,
        commandId: command.commandId,
      });
    }
    return out;
  }, [results, query, t]);

  const activate = useCallback(
    async (row: Row) => {
      try {
        if (row.kind === "web") {
          await api.openPath(row.url);
          await api.hideLauncher();
        } else if (row.kind === "app") {
          await api.launchApp(row.path);
          void api.touchUsage(row.kind, row.key).catch(() => {});
          await api.hideLauncher();
        } else if (row.kind === "file") {
          await api.openPath(row.path);
          void api.touchUsage(row.kind, row.key).catch(() => {});
          await api.hideLauncher();
        } else if (row.kind === "calculator") {
          await api.copyText(row.value);
          await api.hideLauncher();
        } else if (row.kind === "clipboard") {
          await api.copyClipboard(row.id);
          void api.touchUsage(row.kind, row.key).catch(() => {});
          await api.hideLauncher();
        } else {
          const result = await api.runExtensionCommand(
            row.extId,
            row.commandId,
            {},
          );
          void api.touchUsage(row.kind, row.key).catch(() => {});
          if (result.kind === "message") {
            setToast(result.text ?? result.toast ?? t("Command executed."));
            return;
          }
          await api.hideLauncher();
          if (result.toast) {
            void api.notify("Beta Start", result.toast).catch(() => {});
          }
        }
        await refreshStatus();
      } catch (error) {
        setToast(String(error));
      }
    },
    [refreshStatus, t],
  );

  // Atalhos de teclado globais.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      // Não interfere enquanto o usuário está gravando uma hotkey.
      if (document.body.dataset.recordingHotkey === "true") return;
      if (screen !== "search" && screen !== "onboarding") {
        if (event.key === "Escape") setScreen("search");
        return;
      }
      if (screen !== "search") return;

      if (event.key === "ArrowDown") {
        event.preventDefault();
        setSelected((current) =>
          Math.min(current + 1, Math.max(rows.length - 1, 0)),
        );
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        setSelected((current) => Math.max(current - 1, 0));
      } else if (event.key === "Enter") {
        const row = rows[selected];
        if (row) void activate(row);
      } else if (event.key === "Escape") {
        void api.hideLauncher();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [screen, rows, selected, activate]);

  const updateSettings = useCallback((patch: Partial<SettingsDraft>) => {
    setSettings((current) => ({ ...current, ...patch }));
  }, []);

  const clearSearch = useCallback(() => {
    setQuery("");
    setSelected(0);
    inputRef.current?.focus();
  }, []);

  const jumpToSettingsSection = useCallback((id: string) => {
    const target = settingsContentRef.current?.querySelector<HTMLElement>(
      `[data-settings-section="${id}"]`,
    );
    const reduceMotion = window.matchMedia(
      "(prefers-reduced-motion: reduce)",
    ).matches;
    target?.scrollIntoView({
      behavior: reduceMotion ? "auto" : "smooth",
      block: "start",
    });
    setSettingsSection(id);
  }, []);

  /// Tema e idioma têm efeito imediato: persiste na hora em vez de esperar
  /// o botão Save (que fica no fim de uma tela longa).
  const saveAppearance = useCallback(
    async (patch: { theme?: string; language?: string; accent?: string }) => {
      updateSettings(patch);
      if (patch.language) setLanguage(patch.language);
      const appearance: Record<string, string> = {};
      if (patch.theme !== undefined) appearance.theme = patch.theme;
      if (patch.language !== undefined) appearance.language = patch.language;
      if (patch.accent !== undefined) {
        appearance.accent = normalizeAccent(patch.accent);
      }
      try {
        const cfg = await api.setConfig({ appearance });
        setConfig(cfg);
      } catch (error) {
        setSettingsMsg(String(error));
      }
    },
    [setLanguage, updateSettings],
  );

  const addIndexDir = useCallback((value: string) => {
    const path = value.trim();
    if (!path) return;
    setSettings((current) =>
      current.indexDirs.includes(path)
        ? current
        : { ...current, indexDirs: [...current.indexDirs, path] },
    );
    setNewIndexDir("");
  }, []);

  const removeIndexDir = useCallback((value: string) => {
    setSettings((current) => ({
      ...current,
      indexDirs: current.indexDirs.filter((item) => item !== value),
    }));
  }, []);

  const pickIndexDir = useCallback(async () => {
    try {
      await api.setNativeDialogOpen(true);
      const selected = await open({
        directory: true,
        multiple: false,
        title: t("Select a folder to index"),
      });
      if (selected && !Array.isArray(selected)) addIndexDir(selected);
    } catch (error) {
      setSettingsMsg(String(error));
    } finally {
      await api.setNativeDialogOpen(false).catch(() => {});
    }
  }, [addIndexDir, t]);

  const saveSettings = useCallback(async () => {
    setSettingsBusy(true);
    setSettingsMsg(null);
    // Um atalho inválido não pode impedir o resto das configurações de salvar.
    let launcherError: string | null = null;
    try {
      try {
        await api.applyLauncherSettings(mode, hotkey.trim());
      } catch (error) {
        launcherError = String(error);
      }
      const cfg = await api.setConfig({
        tray: {
          hideOnBlur: settings.hideOnBlur,
          startOnBoot: settings.startOnBoot,
        },
        clipboard: {
          enabled: settings.clipboardEnabled,
          maxEntries: Math.max(
            50,
            Math.min(5000, Number(settings.clipboardMax) || 500),
          ),
        },
        search: {
          includeApps: settings.includeApps,
          includeFiles: settings.includeFiles,
          indexDirs: settings.indexDirs,
          maxResults: Math.max(
            4,
            Math.min(30, Number(settings.searchMaxResults) || 8),
          ),
        },
        windows: {
          autoApplyRules: settings.autoApplyRules,
          restoreLastLayout: settings.restoreLastLayout,
        },
        appearance: {
          theme: settings.theme,
          language: settings.language,
          accent: settings.accent,
        },
        mcp: {
          enabled: settings.mcpEnabled,
          http: {
            enabled: settings.mcpEnabled,
            port: Math.max(
              1024,
              Math.min(65535, Number(settings.mcpPort) || 19000),
            ),
          },
        },
        extensions: {
          storeUrl: settings.storeUrl.trim(),
          denoPath: settings.denoPath.trim() || null,
        },
      });
      applyConfig(cfg);
      if (launcherError) {
        setSettingsMsg(launcherError);
      } else {
        setToast(t("Settings saved."));
        setScreen("search");
      }
    } catch (error) {
      setSettingsMsg(String(error));
    } finally {
      setSettingsBusy(false);
    }
  }, [mode, hotkey, settings, applyConfig, t]);

  const checkUpdates = useCallback(async () => {
    setUpdateStatus("checking");
    setUpdateMessage(null);
    setUpdateProgress(0);
    try {
      if (updateRef.current) {
        await updateRef.current.close().catch(() => {});
        updateRef.current = null;
      }
      const update = await check();
      if (!update) {
        setUpdateInfo(null);
        setUpdateStatus("latest");
        return;
      }
      updateRef.current = update;
      setUpdateInfo({
        version: update.version,
        notes: update.body ?? null,
        date: update.date ?? null,
      });
      setUpdateStatus("available");
    } catch (error) {
      setUpdateStatus("error");
      setUpdateMessage(String(error));
    }
  }, []);

  const installUpdate = useCallback(async () => {
    const update = updateRef.current;
    if (!update) return;
    setUpdateStatus("downloading");
    setUpdateProgress(0);
    setUpdateMessage(null);
    let downloaded = 0;
    let total = 0;
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") {
          total = event.data.contentLength ?? 0;
        } else if (event.event === "Progress") {
          downloaded += event.data.chunkLength;
          setUpdateProgress(
            total > 0 ? Math.min(100, Math.round((downloaded / total) * 100)) : 0,
          );
        } else if (event.event === "Finished") {
          setUpdateProgress(100);
        }
      });
      setUpdateStatus("ready");
      // No Windows o instalador já encerra o app; nos demais, reinicia aqui.
      await relaunch();
    } catch (error) {
      setUpdateStatus("error");
      setUpdateMessage(String(error));
    }
  }, []);

  const dismissUpdate = useCallback(async () => {
    if (updateRef.current) {
      await updateRef.current.close().catch(() => {});
      updateRef.current = null;
    }
    setUpdateInfo(null);
    setUpdateStatus("idle");
    setUpdateMessage(null);
  }, []);

  const chooseOnboarding = useCallback(
    async (next: "replace" | "off") => {
      try {
        const cfg = await api.applyLauncherSettings(
          next,
          hotkey || "Alt+Space",
        );
        const done = await api.setOnboardingDone();
        applyConfig({ ...cfg, onboardingDone: done.onboardingDone });
        setScreen("search");
      } catch (error) {
        console.error("Falha ao salvar preferência:", error);
      }
    },
    [hotkey, applyConfig],
  );

  const reindexFiles = useCallback(async () => {
    try {
      await api.rebuildFilesIndex();
      setSettingsMsg(t("Reindexing started in the background."));
    } catch (error) {
      setSettingsMsg(String(error));
    }
  }, [t]);

  if (loading) {
    return (
      <div className="launcher">
        <div className="loading">{t("Loading Beta Start…")}</div>
      </div>
    );
  }

  return (
    <div className="launcher">
      <header className="topbar" data-tauri-drag-region>
        <div className="brand">
          <span className="brand-mark" aria-hidden="true">
            β
          </span>
          <span className="brand-name">Beta Start</span>
          <span className="version">v{status?.version ?? "0.1.0"}</span>
        </div>
        <div className="topbar-right">
          <span
            className="status"
            title={
              status
                ? t("{apps} apps · {files} files · {clips} clips · {exts} ext", {
                    apps: status.appsIndexed,
                    files: status.filesIndexed,
                    clips: status.clipboardCount,
                    exts: status.extensionsCount,
                  })
                : undefined
            }
          >
            {status
              ? t("{apps} apps · {files} files", {
                  apps: status.appsIndexed,
                  files: countLabel(status.filesIndexed, language),
                })
              : t("loading…")}
          </span>
          <nav className="topbar-nav" aria-label={t("Modules")}>
            {navItems.map((item) => (
              <button
                key={item.key}
                className={`icon-btn${
                  screen === item.key ? " is-active" : ""
                }`}
                type="button"
                data-tip={item.label}
                aria-label={item.label}
                aria-pressed={screen === item.key}
                onClick={() => openScreen(item.key)}
              >
                <Icon name={item.icon} size={16} />
              </button>
            ))}
          </nav>
        </div>
      </header>

      {screen === "onboarding" && (
        <section className="onboarding">
          <div className="onboarding-brand">
            <span className="brand-mark" aria-hidden="true">
              β
            </span>
            <span>Beta Start</span>
          </div>
          <h1>{t("How do you want to open Beta Start?")}</h1>
          <p>{t("You can change this later in the settings.")}</p>
          <div className="choices">
            <button
              className="choice"
              type="button"
              onClick={() => void chooseOnboarding("off")}
            >
              <span className="choice-icon" aria-hidden="true">
                <Icon name="command" size={18} />
              </span>
              <span className="choice-body">
                <span className="choice-title">
                  {t("Use Alt + Space")}
                  <span className="choice-tag">{t("Recommended")}</span>
                </span>
                <span className="choice-desc">
                  {t("Does not touch the Windows key. Opens/closes Beta Start with Alt+Space.")}
                </span>
              </span>
              <kbd className="choice-key">Alt + Space</kbd>
            </button>
            <button
              className="choice"
              type="button"
              onClick={() => void chooseOnboarding("replace")}
            >
              <span className="choice-icon" aria-hidden="true">
                <Icon name="windows" size={18} />
              </span>
              <span className="choice-body">
                <span className="choice-title">
                  {t("Replace the Windows key")}
                </span>
                <span className="choice-desc">
                  {t("The Win key alone opens Beta Start. Combos such as Win+E keep working.")}
                </span>
              </span>
              <kbd className="choice-key">Win</kbd>
            </button>
          </div>
          <p className="hint">
            {t("Note: Win+X is reserved by Windows (power menu), so we don't use it.")}
          </p>
        </section>
      )}

      {screen === "settings" && (
        <section className="settings">
          <div className="ext-header">
            <h1>{t("Settings")}</h1>
            <button className="secondary" onClick={() => setScreen("search")}>
              {t("Back")}
            </button>
          </div>

          <nav className="settings-jump" aria-label={t("Settings sections")}>
            {settingsSections.map((section) => (
              <button
                key={section.id}
                type="button"
                className={settingsSection === section.id ? "is-active" : ""}
                onClick={() => jumpToSettingsSection(section.id)}
              >
                {section.label}
              </button>
            ))}
          </nav>

          <div className="settings-content" ref={settingsContentRef}>
          <div className="settings-section" data-settings-section="launcher">
            <h2>{t("Launcher")}</h2>
            <div className="field">
              <span className="field-label">{t("Launcher shortcut")}</span>
              <HotkeyInput
                value={hotkey}
                onChange={(combo) => setHotkey(combo)}
                excludeLauncher
                label={t("Launcher shortcut")}
                placeholder={t("Click and press the shortcut (e.g. Alt+Space)")}
              />
              <span className="field-hint">
                {t("Click the field and press the desired combination. Default: Alt+Space.")}
              </span>
            </div>

            <div className="field">
              <span className="field-label">{t("Windows key")}</span>
              <div className="radio-row" role="radiogroup" aria-label={t("Windows key")}>
                <button
                  className={`radio ${mode === "off" ? "active" : ""}`}
                  type="button"
                  role="radio"
                  aria-checked={mode === "off"}
                  onClick={() => setMode("off")}
                >
                  {t("Do not replace (recommended)")}
                </button>
                <button
                  className={`radio ${mode === "replace" ? "active" : ""}`}
                  type="button"
                  role="radio"
                  aria-checked={mode === "replace"}
                  onClick={() => setMode("replace")}
                >
                  {t("Replace the Win key")}
                </button>
              </div>
              <span className="field-hint">
                {t("Win+X is used by Windows itself, so the default is Alt+Space.")}
              </span>
              <span className="field-hint">
                {t("Win key hook: {status}", {
                  status: status?.winHookActive ? t("active") : t("inactive"),
                })}
              </span>
            </div>

            <label className="check-row">
              <input
                type="checkbox"
                checked={settings.hideOnBlur}
                onChange={(event) =>
                  updateSettings({ hideOnBlur: event.target.checked })
                }
              />
              <span>{t("Close when clicking outside the launcher")}</span>
            </label>
            <label className="check-row">
              <input
                type="checkbox"
                checked={settings.startOnBoot}
                onChange={(event) =>
                  updateSettings({ startOnBoot: event.target.checked })
                }
              />
              <span>{t("Start with Windows")}</span>
            </label>
          </div>

          <div className="settings-section" data-settings-section="appearance">
            <h2>{t("Appearance")}</h2>
            <span className="field-hint">
              {t("Theme, accent, and language are saved immediately.")}
            </span>
            <label className="field">
              <span className="field-label">{t("Theme")}</span>
              <Select
                value={settings.theme}
                onChange={(event) =>
                  void saveAppearance({ theme: event.target.value })
                }
              >
                <option value="dark">{t("Dark")}</option>
                <option value="light">{t("Light")}</option>
                <option value="auto">{t("Automatic (Windows)")}</option>
              </Select>
            </label>
            <div className="field">
              <span className="field-label">{t("Accent color")}</span>
              <div
                className="accent-row"
                role="group"
                aria-label={t("Accent color")}
              >
                {ACCENTS.map((accent) => {
                  const active =
                    normalizeAccent(settings.accent) === accent.id;
                  return (
                    <button
                      key={accent.id}
                      type="button"
                      data-accent={accent.id}
                      className={`accent-option${
                        active ? " is-active" : ""
                      }`}
                      aria-pressed={active}
                      title={t(accent.label)}
                      onClick={() =>
                        void saveAppearance({ accent: accent.id })
                      }
                    >
                      <span className="accent-dot" aria-hidden="true" />
                      {t(accent.label)}
                    </button>
                  );
                })}
              </div>
            </div>
            <label className="field">
              <span className="field-label">{t("Language")}</span>
              <Select
                value={settings.language}
                onChange={(event) =>
                  void saveAppearance({ language: event.target.value })
                }
              >
                <option value="en">English</option>
                <option value="pt-BR">Português (Brasil)</option>
              </Select>
            </label>
          </div>

          <div className="settings-section" data-settings-section="updates">
            <h2>{t("Updates")}</h2>
            <div className="settings-grid">
              <div className="field">
                <span className="field-label">{t("Installed version")}</span>
                <code className="update-version">
                  {status?.version ?? t("loading…")}
                </code>
              </div>
              <button
                className="secondary"
                disabled={
                  updateStatus === "checking" ||
                  updateStatus === "downloading"
                }
                onClick={() => void checkUpdates()}
              >
                {updateStatus === "checking"
                  ? t("Checking…")
                  : t("Check for updates")}
              </button>
            </div>

            {updateStatus === "latest" && (
              <span className="field-hint">
                {t("You are already on the latest version.")}
              </span>
            )}

            {updateStatus === "available" && updateInfo && (
              <div className="update-card">
                <strong>
                  {t("New version {version} available", {
                    version: updateInfo.version,
                  })}
                </strong>
                {updateInfo.date && (
                  <span className="field-hint">
                    {t("Published on {date}", {
                      date: new Date(updateInfo.date).toLocaleString(language),
                    })}
                  </span>
                )}
                {updateInfo.notes && (
                  <div className="update-notes">{updateInfo.notes}</div>
                )}
                <div className="settings-actions">
                  <button
                    className="primary"
                    onClick={() => void installUpdate()}
                  >
                    {t("Download and install")}
                  </button>
                  <button
                    className="secondary"
                    onClick={() => void dismissUpdate()}
                  >
                    {t("Later")}
                  </button>
                </div>
              </div>
            )}

            {updateStatus === "downloading" && (
              <div className="update-card">
                <strong>{t("Downloading update…")}</strong>
                <div className="update-progress">
                  <div
                    className="update-progress-bar"
                    style={{ width: `${updateProgress}%` }}
                  />
                </div>
                <span className="field-hint">{updateProgress}%</span>
              </div>
            )}

            {updateStatus === "ready" && (
              <span className="field-hint">
                {t("Update installed. Restarting…")}
              </span>
            )}

            {updateStatus === "error" && (
              <>
                <span className="settings-msg">{updateMessage}</span>
                <span className="field-hint">
                  {t("If the endpoint is still the placeholder, configure the releases repository in tauri.conf.json (or use the GitHub release workflow).")}
                </span>
              </>
            )}

            <span className="field-hint">
              {t("Updates are signed with minisign. The app only installs a package whose signature matches the embedded public key.")}
            </span>
          </div>

          <div className="settings-section" data-settings-section="search">
            <h2>{t("Search & index")}</h2>
            <div className="settings-grid">
              <label className="check-row">
                <input
                  type="checkbox"
                  checked={settings.includeApps}
                  onChange={(event) =>
                    updateSettings({ includeApps: event.target.checked })
                  }
                />
                <span>{t("Search applications")}</span>
              </label>
              <label className="check-row">
                <input
                  type="checkbox"
                  checked={settings.includeFiles}
                  onChange={(event) =>
                    updateSettings({ includeFiles: event.target.checked })
                  }
                />
                <span>{t("Search files")}</span>
              </label>
              <label className="field">
                <span className="field-label">{t("Maximum results")}</span>
                <input
                  className="text-input"
                  type="number"
                  min={4}
                  max={30}
                  value={settings.searchMaxResults}
                  onChange={(event) =>
                    updateSettings({
                      searchMaxResults: Number(event.target.value),
                    })
                  }
                />
              </label>
            </div>

            <div className="field">
              <span className="field-label">{t("Indexed folders")}</span>
              <div className="dir-list">
                {settings.indexDirs.length === 0 && (
                  <span className="field-hint">
                    {t("Default: Documents, Desktop, and Downloads.")}
                  </span>
                )}
                {settings.indexDirs.map((dir) => (
                  <div className="dir-row" key={dir}>
                    <code>{dir}</code>
                    <button
                      className="secondary"
                      onClick={() => removeIndexDir(dir)}
                    >
                      {t("Remove")}
                    </button>
                  </div>
                ))}
              </div>
              <div className="inline">
                <input
                  className="text-input"
                  value={newIndexDir}
                  onChange={(event) => setNewIndexDir(event.target.value)}
                  placeholder={t("C:\\path\\to\\index")}
                  aria-label={t("Folder to index")}
                  spellCheck={false}
                />
                <button
                  className="secondary"
                  disabled={!newIndexDir.trim()}
                  onClick={() => addIndexDir(newIndexDir)}
                >
                  {t("Add")}
                </button>
                <button
                  className="secondary"
                  onClick={() => void pickIndexDir()}
                >
                  {t("Choose…")}
                </button>
              </div>
            </div>
          </div>

          <div className="settings-section" data-settings-section="clipboard">
            <h2>{t("Clipboard")}</h2>
            <div className="settings-grid">
              <label className="check-row">
                <input
                  type="checkbox"
                  checked={settings.clipboardEnabled}
                  onChange={(event) =>
                    updateSettings({ clipboardEnabled: event.target.checked })
                  }
                />
                <span>{t("Save clipboard history")}</span>
              </label>
              <label className="field">
                <span className="field-label">{t("Maximum items")}</span>
                <input
                  className="text-input"
                  type="number"
                  min={50}
                  max={5000}
                  value={settings.clipboardMax}
                  onChange={(event) =>
                    updateSettings({ clipboardMax: Number(event.target.value) })
                  }
                />
              </label>
            </div>
          </div>

          <div className="settings-section" data-settings-section="windows">
            <h2>{t("Windows")}</h2>
            <label className="check-row">
              <input
                type="checkbox"
                checked={settings.autoApplyRules}
                onChange={(event) =>
                  updateSettings({ autoApplyRules: event.target.checked })
                }
              />
              <span>{t("Apply rules automatically to new windows")}</span>
            </label>
            <label className="check-row">
              <input
                type="checkbox"
                checked={settings.restoreLastLayout}
                onChange={(event) =>
                  updateSettings({ restoreLastLayout: event.target.checked })
                }
              />
              <span>{t("Reapply the last layout at startup")}</span>
            </label>
          </div>

          <div className="settings-section" data-settings-section="mcp">
            <h2>MCP</h2>
            <div className="settings-grid">
              <label className="check-row">
                <input
                  type="checkbox"
                  checked={settings.mcpEnabled}
                  onChange={(event) =>
                    updateSettings({ mcpEnabled: event.target.checked })
                  }
                />
                <span>{t("MCP server enabled")}</span>
              </label>
              <label className="field">
                <span className="field-label">{t("HTTP/SSE port")}</span>
                <input
                  className="text-input"
                  type="number"
                  min={1024}
                  max={65535}
                  value={settings.mcpPort}
                  onChange={(event) =>
                    updateSettings({ mcpPort: Number(event.target.value) })
                  }
                />
              </label>
            </div>
            <span className="field-hint">
              {t("Server/port changes are applied when the app restarts.")}
            </span>
          </div>

          <div className="settings-section" data-settings-section="extensions">
            <h2>{t("Extensions")}</h2>
            <label className="field">
              <span className="field-label">{t("Store URL")}</span>
              <input
                className="text-input"
                value={settings.storeUrl}
                onChange={(event) =>
                  updateSettings({ storeUrl: event.target.value })
                }
                spellCheck={false}
              />
            </label>
            <label className="field">
              <span className="field-label">{t("Deno path (optional)")}</span>
              <input
                className="text-input"
                value={settings.denoPath}
                onChange={(event) =>
                  updateSettings({ denoPath: event.target.value })
                }
                placeholder={t("Leave empty to detect automatically")}
                spellCheck={false}
              />
            </label>
          </div>

          {status && (
            <div className="paths">
              <div>
                <span>Workspace</span>
                <code>{status.paths.workspace}</code>
              </div>
              <div>
                <span>{t("Database")}</span>
                <code>{status.paths.db}</code>
              </div>
              <div>
                <span>Config</span>
                <code>{status.paths.config}</code>
              </div>
            </div>
          )}
          </div>

          {settingsMsg && <p className="settings-msg">{settingsMsg}</p>}

          <div className="save-bar">
            <span
              className={`save-state${settingsDirty ? " is-dirty" : ""}`}
              aria-live="polite"
            >
              {settingsBusy ? (
                t("Saving…")
              ) : settingsDirty ? (
                <>
                  <span className="save-dot" aria-hidden="true" />
                  {t("Unsaved changes")}
                </>
              ) : (
                <>
                  <Icon name="check" size={13} />
                  {t("All changes saved")}
                </>
              )}
            </span>
            <div className="settings-actions">
              <button
                className="secondary"
                type="button"
                onClick={() => void reindexFiles()}
              >
                {t("Reindex files")}
              </button>
              <button
                className="primary"
                type="button"
                onClick={() => void saveSettings()}
                disabled={settingsBusy || !settingsDirty}
              >
                {settingsBusy ? t("Saving…") : t("Save changes")}
              </button>
            </div>
          </div>
        </section>
      )}

      {screen === "extensions" && (
        <Extensions
          onBack={() => setScreen("search")}
          onChanged={() => void refreshStatus()}
        />
      )}

      {screen === "windows" && (
        <WindowManager onBack={() => setScreen("search")} />
      )}

      {screen === "memory" && (
        <Memory onBack={() => setScreen("search")} />
      )}

      {screen === "clipboard" && (
        <Clipboard
          onBack={() => setScreen("search")}
          onChanged={() => void refreshStatus()}
        />
      )}

      {screen === "mcp" && <Mcp onBack={() => setScreen("search")} />}

      {screen === "search" && (
        <>
          <div className="search-wrap">
            <Icon className="search-icon" name="search" size={18} />
            <input
              ref={inputRef}
              className="search"
              placeholder={t("Search apps, files, clipboard, or calculate…")}
              aria-label={t("Search apps, files, clipboard, or calculate…")}
              role="combobox"
              aria-autocomplete="list"
              aria-controls="search-results"
              aria-expanded="true"
              aria-activedescendant={
                rows.length > 0 ? `search-result-${selected}` : undefined
              }
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              spellCheck={false}
              autoComplete="off"
            />
            {query && (
              <button
                className="search-clear"
                type="button"
                aria-label={t("Clear search")}
                title={t("Clear search")}
                onClick={clearSearch}
              >
                <Icon name="clear" size={14} />
              </button>
            )}
          </div>
          <span className="visually-hidden" aria-live="polite">
            {t("{count} result(s)", { count: rows.length })}
          </span>

          <ul
            className="results"
            id="search-results"
            ref={resultsRef}
            role="listbox"
            aria-label={t("Search results")}
          >
            {rows.length === 0 && (
              <li className="empty" role="presentation">
                <span className="empty-icon" aria-hidden="true">
                  <Icon
                    name={query.trim() ? "search" : "command"}
                    size={22}
                  />
                </span>
                <strong>
                  {query.trim()
                    ? t("Nothing found for “{query}”", {
                        query: query.trim(),
                      })
                    : t("Start typing to search")}
                </strong>
                <span className="empty-lead">
                  {t("Search apps, files, clipboard and commands from one place.")}
                </span>
                <span className="empty-examples">
                  <span>
                    <code>g</code> {t("web search")}
                  </span>
                  <span>
                    <code>12*8+3</code> {t("calculator")}
                  </span>
                </span>
              </li>
            )}
            {rows.map((row, index) => {
              const glyph = rowGlyph(row.kind);
              return (
                <li
                  key={row.key}
                  id={`search-result-${index}`}
                  data-result-index={index}
                  className={`result ${
                    index === selected ? "selected" : ""
                  }`}
                  role="option"
                  aria-selected={index === selected}
                  onMouseEnter={() => setSelected(index)}
                  onClick={() => void activate(row)}
                >
                  <span
                    className={`app-icon badge-${row.kind}${
                      row.kind === "app" && row.icon ? " has-icon" : ""
                    }`}
                  >
                    {row.kind === "app" && row.icon ? (
                      <img src={row.icon} alt="" />
                    ) : glyph ? (
                      <Icon name={glyph} size={15} />
                    ) : (
                      badgeLabel(row)
                    )}
                  </span>
                  <span className="app-text">
                    <span className="app-name">{row.title}</span>
                    <span className="app-path">{row.subtitle}</span>
                  </span>
                  <span className="visually-hidden">
                    {rowKindLabel(row.kind, t)}
                  </span>
                  <span className="result-meta" aria-hidden="true">
                    {index === selected ? (
                      <span className="result-action">
                        {rowActionLabel(row.kind, t)}
                        <Icon name="arrow-right" size={13} />
                      </span>
                    ) : (
                      <span className="result-kind">
                        {rowKindLabel(row.kind, t)}
                      </span>
                    )}
                  </span>
                </li>
              );
            })}
          </ul>

          <footer className="footer">
            <span>
              <kbd>↑</kbd>
              <kbd>↓</kbd> {t("navigate")}
            </span>
            <span>
              <kbd>Enter</kbd> {t("open")}
            </span>
            <span>
              <kbd>Esc</kbd> {t("close")}
            </span>
            <span className="workspace" title={status?.paths.workspace}>
              {status?.paths.workspace}
            </span>
          </footer>
        </>
      )}

      {toast && (
        <div className="toast" role="status" aria-live="polite">
          {toast}
        </div>
      )}
    </div>
  );
}

export default App;

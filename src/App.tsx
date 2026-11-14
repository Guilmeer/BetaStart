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
import { Mcp } from "./Mcp";
import { Memory } from "./Memory";
import { WindowManager } from "./WindowManager";
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
  windows: [760, 460],
  memory: [1120, 740],
  clipboard: [900, 640],
  mcp: [900, 680],
};

interface SettingsDraft {
  hideOnBlur: boolean;
  startOnBoot: boolean;
  theme: string;
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
    label: "Buscar no Google",
    build: (term) => `https://www.google.com/search?q=${encodeURIComponent(term)}`,
  },
  {
    prefix: "yt",
    label: "Buscar no YouTube",
    build: (term) =>
      `https://www.youtube.com/results?search_query=${encodeURIComponent(term)}`,
  },
  {
    prefix: "gh",
    label: "Buscar no GitHub",
    build: (term) => `https://github.com/search?q=${encodeURIComponent(term)}`,
  },
  {
    prefix: "w",
    label: "Buscar na Wikipédia",
    build: (term) =>
      `https://pt.wikipedia.org/w/index.php?search=${encodeURIComponent(term)}`,
  },
  {
    prefix: "ddg",
    label: "Buscar no DuckDuckGo",
    build: (term) => `https://duckduckgo.com/?q=${encodeURIComponent(term)}`,
  },
  {
    prefix: "maps",
    label: "Abrir no Google Maps",
    build: (term) =>
      `https://www.google.com/maps/search/?api=1&query=${encodeURIComponent(term)}`,
  },
];

function webAction(query: string): Row | null {
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
      title: `${action.label}: ${term}`,
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

function App() {
  const [screen, setScreen] = useState<Screen>("search");
  const [, setConfig] = useState<Config | null>(null);
  const [status, setStatus] = useState<StatusInfo | null>(null);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResults | null>(null);
  const [selected, setSelected] = useState(0);
  const [loading, setLoading] = useState(true);
  const [toast, setToast] = useState<string | null>(null);

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

  const refreshStatus = useCallback(async () => {
    try {
      setStatus(await api.getStatus());
    } catch (error) {
      console.error("Falha ao atualizar status:", error);
    }
  }, []);

  const applyConfig = useCallback((cfg: Config) => {
    setConfig(cfg);
    setMode(cfg.startMenu.mode === "replace" ? "replace" : "off");
    setHotkey(cfg.hotkeys.launcher);
    setSettings({
      hideOnBlur: cfg.tray.hideOnBlur,
      startOnBoot: cfg.tray.startOnBoot,
      theme: cfg.appearance.theme || "dark",
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
  }, []);

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
      document.documentElement.dataset.theme = resolved === "light" ? "light" : "dark";
    };
    apply();
    media.addEventListener("change", apply);
    return () => media.removeEventListener("change", apply);
  }, [settings.theme]);

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
    const web = webAction(query);
    if (web) out.push(web);
    if (!results) return out;

    if (results.calculation) {
      out.push({
        kind: "calculator",
        key: "calc",
        title: results.calculation,
        subtitle: "Copiar resultado",
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
          ? "Imagem copiada"
          : text.length > 90
            ? `${text.slice(0, 90)}…`
            : text || "(vazio)",
        subtitle: isImage
          ? "Área de transferência · imagem"
          : "Área de transferência",
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
  }, [results, query]);

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
            setToast(result.text ?? result.toast ?? "Comando executado.");
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
    [refreshStatus],
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
        title: "Selecionar pasta para indexar",
      });
      if (selected && !Array.isArray(selected)) addIndexDir(selected);
    } catch (error) {
      setSettingsMsg(String(error));
    } finally {
      await api.setNativeDialogOpen(false).catch(() => {});
    }
  }, [addIndexDir]);

  const saveSettings = useCallback(async () => {
    setSettingsBusy(true);
    setSettingsMsg(null);
    try {
      await api.applyLauncherSettings(mode, hotkey.trim());
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
        appearance: { theme: settings.theme },
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
      setToast("Configurações salvas.");
      setScreen("search");
    } catch (error) {
      setSettingsMsg(String(error));
    } finally {
      setSettingsBusy(false);
    }
  }, [mode, hotkey, settings, applyConfig]);

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
      setSettingsMsg("Reindexação iniciada em segundo plano.");
    } catch (error) {
      setSettingsMsg(String(error));
    }
  }, []);

  if (loading) {
    return (
      <div className="launcher">
        <div className="loading">Carregando Beta Start…</div>
      </div>
    );
  }

  return (
    <div className="launcher">
      <header className="topbar" data-tauri-drag-region>
        <div className="brand">
          <span className="logo" />
          <span className="brand-name">Beta Start</span>
          <span className="version">v{status?.version ?? "0.1.0"}</span>
        </div>
        <div className="topbar-right">
          <span className="status">
            {status
              ? `${status.appsIndexed} apps · ${status.filesIndexed} arq · ${status.clipboardCount} clips · ${status.extensionsCount} ext`
              : "carregando…"}
          </span>
          <button
            className="icon-btn"
            title="Área de transferência"
            onClick={() =>
              setScreen(screen === "clipboard" ? "search" : "clipboard")
            }
          >
            📋
          </button>
          <button
            className="icon-btn"
            title="MCP"
            onClick={() => setScreen(screen === "mcp" ? "search" : "mcp")}
          >
            🔌
          </button>
          <button
            className="icon-btn"
            title="Memória & Tasks"
            onClick={() =>
              setScreen(screen === "memory" ? "search" : "memory")
            }
          >
            🧠
          </button>
          <button
            className="icon-btn"
            title="Gerenciador de janelas"
            onClick={() =>
              setScreen(screen === "windows" ? "search" : "windows")
            }
          >
            🪟
          </button>
          <button
            className="icon-btn"
            title="Extensões"
            onClick={() =>
              setScreen(screen === "extensions" ? "search" : "extensions")
            }
          >
            🧩
          </button>
          <button
            className="icon-btn"
            title="Configurações"
            onClick={() =>
              setScreen(screen === "settings" ? "search" : "settings")
            }
          >
            ⚙
          </button>
        </div>
      </header>

      {screen === "onboarding" && (
        <section className="onboarding">
          <h1>Como você quer abrir o Beta Start?</h1>
          <p>Você pode mudar isso depois no ícone ⚙.</p>
          <div className="choices">
            <button
              className="choice"
              onClick={() => void chooseOnboarding("off")}
            >
              <strong>Usar Alt + Espaço (recomendado)</strong>
              <span>
                Não mexe na tecla Windows. Abre/fecha o Beta Start com
                Alt+Espaço.
              </span>
            </button>
            <button
              className="choice"
              onClick={() => void chooseOnboarding("replace")}
            >
              <strong>Substituir a tecla Windows</strong>
              <span>
                A tecla Win sozinha abre o Beta Start. Combos como Win+E
                continuam funcionando.
              </span>
            </button>
          </div>
          <p className="hint">
            Obs.: Win+X é reservado pelo Windows (menu de energia), por isso não
            usamos.
          </p>
        </section>
      )}

      {screen === "settings" && (
        <section className="settings">
          <div className="ext-header">
            <h1>Configurações</h1>
            <button className="secondary" onClick={() => setScreen("search")}>
              Voltar
            </button>
          </div>

          <div className="settings-section">
            <h2>Launcher</h2>
            <div className="field">
              <span className="field-label">Atalho do launcher</span>
              <HotkeyInput
                value={hotkey}
                onChange={(combo) => setHotkey(combo)}
                excludeLauncher
                placeholder="Clique e pressione o atalho (ex.: Alt+Space)"
              />
              <span className="field-hint">
                Clique no campo e pressione a combinação desejada. Padrão: Alt+Space.
              </span>
            </div>

            <div className="field">
              <span className="field-label">Tecla Windows</span>
              <div className="radio-row">
                <button
                  className={`radio ${mode === "off" ? "active" : ""}`}
                  onClick={() => setMode("off")}
                >
                  Não substituir (recomendado)
                </button>
                <button
                  className={`radio ${mode === "replace" ? "active" : ""}`}
                  onClick={() => setMode("replace")}
                >
                  Substituir a tecla Win
                </button>
              </div>
              <span className="field-hint">
                Win+X é usado pelo próprio Windows; por isso o padrão é Alt+Space.
              </span>
              <span className="field-hint">
                Hook da tecla Win:{" "}
                {status?.winHookActive ? "ativo" : "inativo"}
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
              <span>Fechar ao clicar fora do launcher</span>
            </label>
            <label className="check-row">
              <input
                type="checkbox"
                checked={settings.startOnBoot}
                onChange={(event) =>
                  updateSettings({ startOnBoot: event.target.checked })
                }
              />
              <span>Iniciar com o Windows</span>
            </label>
          </div>

          <div className="settings-section">
            <h2>Aparência</h2>
            <label className="field">
              <span className="field-label">Tema</span>
              <select
                className="text-input"
                value={settings.theme}
                onChange={(event) =>
                  updateSettings({ theme: event.target.value })
                }
              >
                <option value="dark">Escuro</option>
                <option value="light">Claro</option>
                <option value="auto">Automático (Windows)</option>
              </select>
            </label>
          </div>

          <div className="settings-section">
            <h2>Atualizações</h2>
            <div className="settings-grid">
              <div className="field">
                <span className="field-label">Versão instalada</span>
                <code className="update-version">
                  {status?.version ?? "carregando…"}
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
                  ? "Verificando…"
                  : "Verificar atualizações"}
              </button>
            </div>

            {updateStatus === "latest" && (
              <span className="field-hint">
                Você já está na versão mais recente.
              </span>
            )}

            {updateStatus === "available" && updateInfo && (
              <div className="update-card">
                <strong>Nova versão {updateInfo.version} disponível</strong>
                {updateInfo.date && (
                  <span className="field-hint">
                    Publicada em{" "}
                    {new Date(updateInfo.date).toLocaleString()}
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
                    Baixar e instalar
                  </button>
                  <button
                    className="secondary"
                    onClick={() => void dismissUpdate()}
                  >
                    Depois
                  </button>
                </div>
              </div>
            )}

            {updateStatus === "downloading" && (
              <div className="update-card">
                <strong>Baixando atualização…</strong>
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
                Atualização instalada. Reiniciando…
              </span>
            )}

            {updateStatus === "error" && (
              <>
                <span className="settings-msg">{updateMessage}</span>
                <span className="field-hint">
                  Se o endpoint ainda for o placeholder, configure o repositório
                  de releases em <code>tauri.conf.json</code> (ou use o workflow
                  de release no GitHub).
                </span>
              </>
            )}

            <span className="field-hint">
              As atualizações são assinadas (minisign). O app só instala um
              pacote cuja assinatura combine com a chave pública embutida.
            </span>
          </div>

          <div className="settings-section">
            <h2>Busca e índice</h2>
            <div className="settings-grid">
              <label className="check-row">
                <input
                  type="checkbox"
                  checked={settings.includeApps}
                  onChange={(event) =>
                    updateSettings({ includeApps: event.target.checked })
                  }
                />
                <span>Buscar aplicativos</span>
              </label>
              <label className="check-row">
                <input
                  type="checkbox"
                  checked={settings.includeFiles}
                  onChange={(event) =>
                    updateSettings({ includeFiles: event.target.checked })
                  }
                />
                <span>Buscar arquivos</span>
              </label>
              <label className="field">
                <span className="field-label">Máximo de resultados</span>
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
              <span className="field-label">Pastas indexadas</span>
              <div className="dir-list">
                {settings.indexDirs.length === 0 && (
                  <span className="field-hint">
                    Padrão: Documentos, Área de Trabalho e Downloads.
                  </span>
                )}
                {settings.indexDirs.map((dir) => (
                  <div className="dir-row" key={dir}>
                    <code>{dir}</code>
                    <button
                      className="secondary"
                      onClick={() => removeIndexDir(dir)}
                    >
                      Remover
                    </button>
                  </div>
                ))}
              </div>
              <div className="inline">
                <input
                  className="text-input"
                  value={newIndexDir}
                  onChange={(event) => setNewIndexDir(event.target.value)}
                  placeholder="C:\caminho\para\indexar"
                  spellCheck={false}
                />
                <button
                  className="secondary"
                  disabled={!newIndexDir.trim()}
                  onClick={() => addIndexDir(newIndexDir)}
                >
                  Adicionar
                </button>
                <button
                  className="secondary"
                  onClick={() => void pickIndexDir()}
                >
                  Escolher…
                </button>
              </div>
            </div>
          </div>

          <div className="settings-section">
            <h2>Clipboard</h2>
            <div className="settings-grid">
              <label className="check-row">
                <input
                  type="checkbox"
                  checked={settings.clipboardEnabled}
                  onChange={(event) =>
                    updateSettings({ clipboardEnabled: event.target.checked })
                  }
                />
                <span>Salvar histórico da área de transferência</span>
              </label>
              <label className="field">
                <span className="field-label">Máximo de itens</span>
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

          <div className="settings-section">
            <h2>Janelas</h2>
            <label className="check-row">
              <input
                type="checkbox"
                checked={settings.autoApplyRules}
                onChange={(event) =>
                  updateSettings({ autoApplyRules: event.target.checked })
                }
              />
              <span>Aplicar regras automaticamente a janelas novas</span>
            </label>
            <label className="check-row">
              <input
                type="checkbox"
                checked={settings.restoreLastLayout}
                onChange={(event) =>
                  updateSettings({ restoreLastLayout: event.target.checked })
                }
              />
              <span>Reaplicar o último layout ao iniciar</span>
            </label>
          </div>

          <div className="settings-section">
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
                <span>Servidor MCP habilitado</span>
              </label>
              <label className="field">
                <span className="field-label">Porta HTTP/SSE</span>
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
              Alterações de servidor/porta são aplicadas ao reiniciar o app.
            </span>
          </div>

          <div className="settings-section">
            <h2>Extensões</h2>
            <label className="field">
              <span className="field-label">URL da loja</span>
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
              <span className="field-label">Caminho do Deno (opcional)</span>
              <input
                className="text-input"
                value={settings.denoPath}
                onChange={(event) =>
                  updateSettings({ denoPath: event.target.value })
                }
                placeholder="Deixe vazio para detectar automaticamente"
                spellCheck={false}
              />
            </label>
          </div>

          <div className="settings-actions">
            <button
              className="primary"
              onClick={() => void saveSettings()}
              disabled={settingsBusy}
            >
              {settingsBusy ? "Salvando…" : "Salvar"}
            </button>
            <button className="secondary" onClick={() => void reindexFiles()}>
              Reindexar arquivos
            </button>
          </div>

          {settingsMsg && <p className="settings-msg">{settingsMsg}</p>}

          {status && (
            <div className="paths">
              <div>
                <span>Workspace</span>
                <code>{status.paths.workspace}</code>
              </div>
              <div>
                <span>Banco</span>
                <code>{status.paths.db}</code>
              </div>
              <div>
                <span>Config</span>
                <code>{status.paths.config}</code>
              </div>
            </div>
          )}
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
          <input
            ref={inputRef}
            className="search"
            placeholder="Buscar apps, arquivos, clipboard ou calcular…"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            spellCheck={false}
            autoComplete="off"
          />

          <ul className="results">
            {rows.length === 0 && (
              <li className="empty">Nenhum resultado.</li>
            )}
            {rows.map((row, index) => (
              <li
                key={row.key}
                className={`result ${index === selected ? "selected" : ""}`}
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
                  ) : (
                    badgeLabel(row)
                  )}
                </span>
                <span className="app-text">
                  <span className="app-name">{row.title}</span>
                  <span className="app-path">{row.subtitle}</span>
                </span>
              </li>
            ))}
          </ul>

          <footer className="footer">
            <span>
              <kbd>↑</kbd>
              <kbd>↓</kbd> navegar
            </span>
            <span>
              <kbd>Enter</kbd> abrir
            </span>
            <span>
              <kbd>Esc</kbd> fechar
            </span>
            <span className="workspace" title={status?.paths.workspace}>
              {status?.paths.workspace}
            </span>
          </footer>
        </>
      )}

      {toast && <div className="toast">{toast}</div>}
    </div>
  );
}

export default App;

import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWindow, LogicalSize } from "@tauri-apps/api/window";
import "./App.css";
import { api } from "./api";
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
  | "mcp";

type Row =
  | { kind: "calculator"; key: string; title: string; subtitle: string; value: string }
  | { kind: "app"; key: string; title: string; subtitle: string; path: string }
  | { kind: "file"; key: string; title: string; subtitle: string; path: string }
  | { kind: "clipboard"; key: string; title: string; subtitle: string; text: string }
  | {
      kind: "command";
      key: string;
      title: string;
      subtitle: string;
      extId: string;
      commandId: string;
    };

function badgeLabel(kind: Row["kind"]): string {
  switch (kind) {
    case "app":
      return "A";
    case "file":
      return "F";
    case "clipboard":
      return "C";
    case "calculator":
      return "=";
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
    if (screen === "search") {
      inputRef.current?.focus();
    }
  }, [screen]);

  // A tela de memória precisa de mais espaço.
  useEffect(() => {
    const window = getCurrentWindow();
    const width = screen === "memory" ? 1120 : screen === "mcp" ? 900 : 760;
    const height = screen === "memory" ? 740 : screen === "mcp" ? 680 : 460;
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
    if (!results) return [];
    const out: Row[] = [];

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
      const text = (clip.text ?? "").replace(/\s+/g, " ").trim();
      out.push({
        kind: "clipboard",
        key: `clip:${clip.id}`,
        title: text.length > 90 ? `${text.slice(0, 90)}…` : text || "(vazio)",
        subtitle: "Área de transferência",
        text: clip.text ?? "",
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
  }, [results]);

  const activate = useCallback(
    async (row: Row) => {
      try {
        if (row.kind === "app") {
          await api.launchApp(row.path);
          await api.hideLauncher();
        } else if (row.kind === "file") {
          await api.openPath(row.path);
          await api.hideLauncher();
        } else if (row.kind === "calculator") {
          await api.copyText(row.value);
          await api.hideLauncher();
        } else if (row.kind === "clipboard") {
          await api.copyText(row.text);
          await api.hideLauncher();
        } else {
          const result = await api.runExtensionCommand(
            row.extId,
            row.commandId,
            {},
          );
          if (result.kind === "message") {
            setToast(result.text ?? "Comando executado.");
            return;
          }
          await api.hideLauncher();
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

  const saveSettings = useCallback(async () => {
    setSettingsBusy(true);
    setSettingsMsg(null);
    try {
      const cfg = await api.applyLauncherSettings(mode, hotkey.trim());
      applyConfig(cfg);
      setScreen("search");
    } catch (error) {
      setSettingsMsg(String(error));
    } finally {
      setSettingsBusy(false);
    }
  }, [mode, hotkey, applyConfig]);

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
          <h1>Configurações</h1>

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
        <Extensions onBack={() => setScreen("search")} />
      )}

      {screen === "windows" && (
        <WindowManager onBack={() => setScreen("search")} />
      )}

      {screen === "memory" && (
        <Memory onBack={() => setScreen("search")} />
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
                <span className={`app-icon badge-${row.kind}`}>
                  {badgeLabel(row.kind)}
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

import { useCallback, useEffect, useRef, useState } from "react";
import "./App.css";
import { api } from "./api";
import type { AppEntry, Config, StatusInfo } from "./types";

type Screen = "search" | "onboarding";

function App() {
  const [screen, setScreen] = useState<Screen>("search");
  const [, setConfig] = useState<Config | null>(null);
  const [status, setStatus] = useState<StatusInfo | null>(null);
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<AppEntry[]>([]);
  const [selected, setSelected] = useState(0);
  const [loading, setLoading] = useState(true);
  const inputRef = useRef<HTMLInputElement>(null);

  const refreshStatus = useCallback(async () => {
    try {
      setStatus(await api.getStatus());
    } catch (error) {
      console.error("Falha ao atualizar status:", error);
    }
  }, []);

  const load = useCallback(async () => {
    try {
      const [cfg, st] = await Promise.all([api.getConfig(), api.getStatus()]);
      setConfig(cfg);
      setStatus(st);
      setScreen(cfg.onboardingDone ? "search" : "onboarding");
    } catch (error) {
      console.error("Falha ao carregar estado:", error);
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  // Foca a busca sempre que a tela de busca aparece ou a janela ganha foco.
  useEffect(() => {
    if (screen === "search") {
      inputRef.current?.focus();
    }
  }, [screen]);

  useEffect(() => {
    const onFocus = () => inputRef.current?.focus();
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, []);

  // Busca com debounce curto.
  useEffect(() => {
    if (screen !== "search") return;
    const timer = window.setTimeout(() => {
      api
        .searchApps(query, 8)
        .then((res) => {
          setResults(res);
          setSelected(0);
        })
        .catch((error) => console.error("Falha na busca:", error));
    }, 80);
    return () => window.clearTimeout(timer);
  }, [query, screen]);

  const launch = useCallback(
    async (app: AppEntry) => {
      try {
        await api.launchApp(app.exePath);
        await api.hideLauncher();
        await refreshStatus();
      } catch (error) {
        console.error("Falha ao abrir app:", error);
      }
    },
    [refreshStatus],
  );

  const chooseStartMenu = useCallback(
    async (mode: "replace" | "win_x") => {
      try {
        const cfg = await api.setConfig({ startMenu: { mode } });
        const done = await api.setOnboardingDone();
        setConfig({ ...cfg, onboardingDone: done.onboardingDone });
        setScreen("search");
      } catch (error) {
        console.error("Falha ao salvar preferência:", error);
      }
    },
    [],
  );

  // Atalhos globais de teclado.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "ArrowDown") {
        event.preventDefault();
        setSelected((current) =>
          Math.min(current + 1, Math.max(results.length - 1, 0)),
        );
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        setSelected((current) => Math.max(current - 1, 0));
      } else if (event.key === "Enter") {
        const app = results[selected];
        if (app) void launch(app);
      } else if (event.key === "Escape") {
        void api.hideLauncher();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [results, selected, launch]);

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
        <div className="status">
          {status
            ? `${status.appsIndexed} apps · ${status.openWindows} janelas`
            : "carregando…"}
        </div>
      </header>

      {screen === "onboarding" ? (
        <section className="onboarding">
          <h1>Como você quer abrir o Beta Start?</h1>
          <p>Você pode mudar isso depois nas configurações.</p>
          <div className="choices">
            <button
              className="choice"
              onClick={() => void chooseStartMenu("replace")}
            >
              <strong>Substituir a tecla Win</strong>
              <span>
                A tecla Windows abre o Beta Start. Win+Shift abre o Menu Iniciar
                original.
              </span>
            </button>
            <button
              className="choice"
              onClick={() => void chooseStartMenu("win_x")}
            >
              <strong>Usar Win+X</strong>
              <span>
                A tecla Windows continua nativa; Win+X abre o Beta Start.
              </span>
            </button>
          </div>
          <p className="hint">
            Dica: por enquanto use <kbd>Alt</kbd>+<kbd>Espaço</kbd> para
            abrir/fechar.
          </p>
        </section>
      ) : (
        <>
          <input
            ref={inputRef}
            className="search"
            placeholder="Buscar aplicativos, arquivos e comandos…"
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            spellCheck={false}
            autoComplete="off"
          />

          <ul className="results">
            {results.length === 0 && (
              <li className="empty">Nenhum resultado.</li>
            )}
            {results.map((app, index) => (
              <li
                key={`${app.exePath}-${index}`}
                className={`result ${index === selected ? "selected" : ""}`}
                onMouseEnter={() => setSelected(index)}
                onClick={() => void launch(app)}
              >
                <span className="app-icon">
                  {app.name.charAt(0).toUpperCase()}
                </span>
                <span className="app-text">
                  <span className="app-name">{app.name}</span>
                  <span className="app-path">{app.exePath}</span>
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
    </div>
  );
}

export default App;

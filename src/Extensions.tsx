import { useCallback, useEffect, useState } from "react";
import { api } from "./api";
import type { InstalledExtension, RuntimeStatus, StoreItem } from "./types";

function runtimeLabel(loading: boolean, runtime: RuntimeStatus | null): string {
  if (loading) return "verificando…";
  if (!runtime) return "erro ao verificar";
  return runtime.available ? runtime.version ?? "disponível" : "ausente";
}

const PERMISSION_HINTS: Record<string, string> = {
  "fs:read": "ler arquivos do workspace",
  "fs:write": "gravar arquivos no workspace",
  network: "acessar a internet",
  shell: "executar programas",
  "clipboard:read": "ler a área de transferência",
  "clipboard:write": "gravar na área de transferência",
  memory: "ler/gravar na memória",
  tasks: "ler/gravar tasks",
  windows: "listar/mover janelas",
  search: "usar a busca do launcher",
};

export function Extensions({
  onBack,
  onChanged,
}: {
  onBack: () => void;
  onChanged?: () => void;
}) {
  const [items, setItems] = useState<InstalledExtension[]>([]);
  const [runtime, setRuntime] = useState<RuntimeStatus | null>(null);
  const [store, setStore] = useState<StoreItem[]>([]);
  const [storeUrl, setStoreUrl] = useState("");
  const [localPath, setLocalPath] = useState("");
  const [gitUrl, setGitUrl] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [loading, setLoading] = useState(true);

  const refresh = useCallback(async (): Promise<boolean> => {
    const [extensionsResult, runtimeResult] = await Promise.allSettled([
      api.listExtensions(),
      api.runtimeStatus(),
    ]);

    if (extensionsResult.status === "fulfilled") {
      setItems(extensionsResult.value);
    }
    if (runtimeResult.status === "fulfilled") {
      setRuntime(runtimeResult.value);
    }

    setLoading(false);

    const errors = [extensionsResult, runtimeResult]
      .filter(
        (result): result is PromiseRejectedResult =>
          result.status === "rejected",
      )
      .map((result) => String(result.reason));
    if (errors.length > 0) {
      setMessage(errors.join(" · "));
      return false;
    }
    return true;
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  // Usa a URL da loja salva no config como ponto de partida.
  useEffect(() => {
    let active = true;
    api
      .getConfig()
      .then((cfg) => {
        if (active) {
          setStoreUrl((current) => current || cfg.extensions.storeUrl);
        }
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, []);

  const run = useCallback(
    async (action: () => Promise<unknown>, success?: string) => {
      setBusy(true);
      setMessage(null);
      try {
        await action();
        const refreshed = await refresh();
        onChanged?.();
        if (success && refreshed) setMessage(success);
      } catch (error) {
        setMessage(String(error));
      } finally {
        setBusy(false);
      }
    },
    [refresh, onChanged],
  );

  const reload = useCallback(async () => {
    setBusy(true);
    setMessage(null);
    try {
      await refresh();
    } finally {
      setBusy(false);
    }
  }, [refresh]);

  const install = (kind: "local" | "git", value: string) =>
    run(
      () => api.installExtension({ kind, value }),
      "Extensão instalada. Aprove as permissões abaixo.",
    );

  const loadStore = async () => {
    setBusy(true);
    setMessage(null);
    try {
      const url = storeUrl.trim() || null;
      const list = await api.listStore(url);
      setStore(list);

      // Guarda a URL digitada para a próxima visita.
      if (url) {
        try {
          await api.setConfig({ extensions: { storeUrl: url } });
        } catch (error) {
          setMessage(
            `Loja carregada, mas não foi possível salvar a URL: ${String(error)}`,
          );
          return;
        }
      }

      setMessage(
        list.length > 0
          ? `${list.length} extensão(ões) encontrada(s) na loja.`
          : "A loja está vazia.",
      );
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  };

  const openFolder = async (ext: InstalledExtension) => {
    try {
      await api.openPath(ext.installPath);
    } catch (error) {
      setMessage(String(error));
    }
  };

  const openLogs = async () => {
    try {
      await api.openExtensionLogs();
    } catch (error) {
      setMessage(String(error));
    }
  };

  return (
    <section className="extensions">
      <div className="ext-header">
        <h1>Extensões</h1>
        <div className="inline">
          <button
            className="secondary"
            disabled={busy}
            onClick={() => void reload()}
          >
            Atualizar
          </button>
          <button className="secondary" onClick={() => void openLogs()}>
            Logs
          </button>
          <button className="secondary" onClick={onBack}>
            Voltar
          </button>
        </div>
      </div>

      <div className="ext-card">
        <div className="ext-card-row">
          <div>
            <strong>Runtime Deno</strong>
            <div className="field-hint">
              {runtime?.path ?? "Não encontrado no sistema."}
            </div>
          </div>
          <span className={runtime?.available ? "ok" : "warn"}>
            {runtimeLabel(loading, runtime)}
          </span>
        </div>
        {runtime && !runtime.available && (
          <button
            className="primary"
            disabled={busy}
            onClick={() => void run(() => api.ensureRuntime(), "Runtime Deno pronto.")}
          >
            Instalar runtime Deno (~30 MB)
          </button>
        )}
      </div>

      <h2>Instaladas</h2>
      {loading && <p className="field-hint">Carregando…</p>}
      {!loading && items.length === 0 && (
        <p className="field-hint">Nenhuma extensão instalada ainda.</p>
      )}
      {items.map((ext) => {
        const permissions = ext.permissions ?? [];
        const granted = ext.granted ?? [];
        const commands = ext.commands ?? [];
        return (
          <div className="ext-card" key={ext.id}>
            <div className="ext-card-row">
              <div>
                <strong>{ext.name}</strong>{" "}
                <span className="ext-id">
                  {ext.id} · v{ext.version}
                </span>
                <div className="field-hint">{ext.description}</div>
              </div>
              <label className="toggle">
                <input
                  type="checkbox"
                  checked={ext.enabled}
                  onChange={(event) =>
                    void run(() =>
                      api.setExtensionEnabled(ext.id, event.target.checked),
                    )
                  }
                />
                ativa
              </label>
            </div>

            {commands.length > 0 && (
              <div className="field-hint">
                {commands.length} comando(s):{" "}
                {commands.map((command) => command.title).join(" · ")}
              </div>
            )}

            <div className="ext-perms">
              {permissions.length === 0 && (
                <span className="field-hint">Sem permissões.</span>
              )}
              {permissions.map((permission) => (
                <span
                  key={permission}
                  className={
                    granted.includes(permission) ? "perm granted" : "perm pending"
                  }
                  title={PERMISSION_HINTS[permission] ?? permission}
                >
                  {permission}
                </span>
              ))}
            </div>

            <div className="ext-actions">
              {ext.pending && (
                <button
                  className="primary"
                  disabled={busy}
                  onClick={() =>
                    void run(
                      () => api.setExtensionPermissions(ext.id, permissions),
                      "Permissões aprovadas.",
                    )
                  }
                >
                  Aprovar permissões
                </button>
              )}
              <button
                className="secondary"
                disabled={busy || !ext.installPath}
                onClick={() => void openFolder(ext)}
              >
                Abrir pasta
              </button>
              <button
                className="secondary"
                disabled={busy}
                onClick={() => {
                  const confirmed = window.confirm(
                    `Desinstalar "${ext.name}"?\n\nA pasta da extensão será removida do disco.`,
                  );
                  if (!confirmed) return;
                  void run(
                    () => api.uninstallExtension(ext.id),
                    "Extensão removida.",
                  );
                }}
              >
                Desinstalar
              </button>
            </div>
          </div>
        );
      })}

      <h2>Instalar</h2>

      <label className="field">
        <span className="field-label">Pasta local</span>
        <div className="inline">
          <input
            className="text-input"
            value={localPath}
            onChange={(event) => setLocalPath(event.target.value)}
            placeholder="C:\caminho\da\extensao"
            spellCheck={false}
          />
          <button
            className="secondary"
            disabled={busy || !localPath.trim()}
            onClick={() => void install("local", localPath.trim())}
          >
            Instalar
          </button>
        </div>
        <span className="field-hint">
          A pasta precisa conter um <code>extension.json</code>.
        </span>
      </label>

      <label className="field">
        <span className="field-label">Repositório Git</span>
        <div className="inline">
          <input
            className="text-input"
            value={gitUrl}
            onChange={(event) => setGitUrl(event.target.value)}
            placeholder="https://github.com/usuario/extensao"
            spellCheck={false}
          />
          <button
            className="secondary"
            disabled={busy || !gitUrl.trim()}
            onClick={() => void install("git", gitUrl.trim())}
          >
            Instalar
          </button>
        </div>
      </label>

      <h2>Loja</h2>
      <div className="inline">
        <input
          className="text-input"
          value={storeUrl}
          onChange={(event) => setStoreUrl(event.target.value)}
          placeholder="URL do index.json ou caminho local"
          spellCheck={false}
        />
        <button className="secondary" disabled={busy} onClick={() => void loadStore()}>
          Carregar
        </button>
      </div>

      {store.map((item) => (
        <div className="ext-card" key={item.id}>
          <div className="ext-card-row">
            <div>
              <strong>{item.name}</strong>{" "}
              <span className="ext-id">
                {item.id} · v{item.version}
              </span>
              <div className="field-hint">{item.description}</div>
            </div>
            <button
              className="primary"
              disabled={busy || !item.url}
              onClick={() =>
                void run(
                  () =>
                    api.installExtension({
                      kind: item.source,
                      value: item.url,
                    }),
                  "Extensão instalada. Aprove as permissões.",
                )
              }
            >
              Instalar
            </button>
          </div>
        </div>
      ))}

      {message && <p className="settings-msg">{message}</p>}
    </section>
  );
}

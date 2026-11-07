import { useCallback, useEffect, useState } from "react";
import { api } from "./api";
import type { InstalledExtension, RuntimeStatus, StoreItem } from "./types";

export function Extensions({ onBack }: { onBack: () => void }) {
  const [items, setItems] = useState<InstalledExtension[]>([]);
  const [runtime, setRuntime] = useState<RuntimeStatus | null>(null);
  const [store, setStore] = useState<StoreItem[]>([]);
  const [storeUrl, setStoreUrl] = useState("");
  const [localPath, setLocalPath] = useState("");
  const [gitUrl, setGitUrl] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const [extensions, status] = await Promise.all([
        api.listExtensions(),
        api.runtimeStatus(),
      ]);
      setItems(extensions);
      setRuntime(status);
    } catch (error) {
      setMessage(String(error));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const run = useCallback(
    async (action: () => Promise<unknown>, success?: string) => {
      setBusy(true);
      setMessage(null);
      try {
        await action();
        if (success) setMessage(success);
        await refresh();
      } catch (error) {
        setMessage(String(error));
      } finally {
        setBusy(false);
      }
    },
    [refresh],
  );

  const install = (kind: "local" | "git", value: string) =>
    run(
      () => api.installExtension({ kind, value }),
      "Extensão instalada. Aprove as permissões abaixo.",
    );

  const loadStore = async () => {
    setBusy(true);
    setMessage(null);
    try {
      const list = await api.listStore(storeUrl.trim() || null);
      setStore(list);
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

  return (
    <section className="extensions">
      <div className="ext-header">
        <h1>Extensões</h1>
        <button className="secondary" onClick={onBack}>
          Voltar
        </button>
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
            {runtime?.available ? runtime.version ?? "disponível" : "ausente"}
          </span>
        </div>
        {!runtime?.available && (
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
      {items.length === 0 && (
        <p className="field-hint">Nenhuma extensão instalada ainda.</p>
      )}
      {items.map((ext) => (
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

          <div className="ext-perms">
            {ext.permissions.length === 0 && (
              <span className="field-hint">Sem permissões.</span>
            )}
            {ext.permissions.map((permission) => (
              <span
                key={permission}
                className={
                  ext.granted.includes(permission)
                    ? "perm granted"
                    : "perm pending"
                }
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
                    () => api.setExtensionPermissions(ext.id, ext.permissions),
                    "Permissões aprovadas.",
                  )
                }
              >
                Aprovar permissões
              </button>
            )}
            <button
              className="secondary"
              disabled={busy}
              onClick={() =>
                void run(
                  () => api.uninstallExtension(ext.id),
                  "Extensão removida.",
                )
              }
            >
              Desinstalar
            </button>
          </div>
        </div>
      ))}

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
              disabled={busy}
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

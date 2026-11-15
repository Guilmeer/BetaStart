import { useCallback, useEffect, useState } from "react";
import { api } from "./api";
import { useI18n } from "./i18n";
import type { InstalledExtension, RuntimeStatus, StoreItem } from "./types";

function runtimeLabel(
  loading: boolean,
  runtime: RuntimeStatus | null,
  t: (key: string) => string,
): string {
  if (loading) return t("checking…");
  if (!runtime) return t("could not check");
  return runtime.available ? runtime.version ?? t("available") : t("missing");
}

const PERMISSION_HINTS: Record<string, string> = {
  "fs:read": "read files in the workspace",
  "fs:write": "write files in the workspace",
  network: "access the internet",
  shell: "run programs",
  "clipboard:read": "read the clipboard",
  "clipboard:write": "write to the clipboard",
  memory: "read/write memory",
  tasks: "read/write tasks",
  windows: "list/move windows",
  search: "use the launcher search",
};

export function Extensions({
  onBack,
  onChanged,
}: {
  onBack: () => void;
  onChanged?: () => void;
}) {
  const { t } = useI18n();
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
      t("Extension installed. Approve the permissions below."),
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
            t("Store loaded, but the URL could not be saved: {error}", {
              error: String(error),
            }),
          );
          return;
        }
      }

      setMessage(
        list.length > 0
          ? t("{count} extension(s) found in the store.", { count: list.length })
          : t("The store is empty."),
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
        <h1>{t("Extensions")}</h1>
        <div className="inline">
          <button
            className="secondary"
            disabled={busy}
            onClick={() => void reload()}
          >
            {t("Refresh")}
          </button>
          <button className="secondary" onClick={() => void openLogs()}>
            {t("Logs")}
          </button>
          <button className="secondary" onClick={onBack}>
            {t("Back")}
          </button>
        </div>
      </div>

      <div className="ext-card">
        <div className="ext-card-row">
          <div>
            <strong>{t("Deno runtime")}</strong>
            <div className="field-hint">
              {runtime?.path ?? t("Not found on the system.")}
            </div>
          </div>
          <span className={runtime?.available ? "ok" : "warn"}>
            {runtimeLabel(loading, runtime, t)}
          </span>
        </div>
        {runtime && !runtime.available && (
          <button
            className="primary"
            disabled={busy}
            onClick={() => void run(() => api.ensureRuntime(), t("Deno runtime ready."))}
          >
            {t("Install Deno runtime (~30 MB)")}
          </button>
        )}
      </div>

      <h2>{t("Installed")}</h2>
      {loading && <p className="field-hint">{t("Loading…")}</p>}
      {!loading && items.length === 0 && (
        <p className="field-hint">{t("No extensions installed yet.")}</p>
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
                {t("active (extension)")}
              </label>
            </div>

            {commands.length > 0 && (
              <div className="field-hint">
                {t("{count} command(s):", { count: commands.length })}{" "}
                {commands.map((command) => command.title).join(" · ")}
              </div>
            )}

            <div className="ext-perms">
              {permissions.length === 0 && (
                <span className="field-hint">{t("No permissions.")}</span>
              )}
              {permissions.map((permission) => (
                <span
                  key={permission}
                  className={
                    granted.includes(permission) ? "perm granted" : "perm pending"
                  }
                  title={
                    PERMISSION_HINTS[permission]
                      ? t(PERMISSION_HINTS[permission])
                      : permission
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
                      () => api.setExtensionPermissions(ext.id, permissions),
                      t("Permissions approved."),
                    )
                  }
                >
                  {t("Approve permissions")}
                </button>
              )}
              <button
                className="secondary"
                disabled={busy || !ext.installPath}
                onClick={() => void openFolder(ext)}
              >
                {t("Open folder")}
              </button>
              <button
                className="secondary danger"
                disabled={busy}
                onClick={() => {
                  const confirmed = window.confirm(
                    t('Uninstall "{name}"?\n\nThe extension folder will be removed from disk.', {
                      name: ext.name,
                    }),
                  );
                  if (!confirmed) return;
                  void run(
                    () => api.uninstallExtension(ext.id),
                    t("Extension removed."),
                  );
                }}
              >
                {t("Uninstall")}
              </button>
            </div>
          </div>
        );
      })}

      <h2>{t("Install")}</h2>

      <label className="field">
        <span className="field-label">{t("Local folder")}</span>
        <div className="inline">
          <input
            className="text-input"
            value={localPath}
            onChange={(event) => setLocalPath(event.target.value)}
            placeholder={t("C:\\path\\to\\extension")}
            spellCheck={false}
          />
          <button
            className="secondary"
            disabled={busy || !localPath.trim()}
            onClick={() => void install("local", localPath.trim())}
          >
            {t("Install")}
          </button>
        </div>
        <span className="field-hint">
          {t("The folder must contain an extension.json file.")}
        </span>
      </label>

      <label className="field">
        <span className="field-label">{t("Git repository")}</span>
        <div className="inline">
          <input
            className="text-input"
            value={gitUrl}
            onChange={(event) => setGitUrl(event.target.value)}
            placeholder={t("https://github.com/user/extension")}
            spellCheck={false}
          />
          <button
            className="secondary"
            disabled={busy || !gitUrl.trim()}
            onClick={() => void install("git", gitUrl.trim())}
          >
            {t("Install")}
          </button>
        </div>
      </label>

      <h2>{t("Store")}</h2>
      <div className="inline">
        <input
          className="text-input"
          value={storeUrl}
          onChange={(event) => setStoreUrl(event.target.value)}
          placeholder={t("index.json URL or local path")}
          aria-label={t("Store URL")}
          spellCheck={false}
        />
        <button className="secondary" disabled={busy} onClick={() => void loadStore()}>
          {t("Load")}
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
                  t("Extension installed. Approve the permissions."),
                )
              }
            >
              {t("Install")}
            </button>
          </div>
        </div>
      ))}

      {message && <p className="settings-msg">{message}</p>}
    </section>
  );
}

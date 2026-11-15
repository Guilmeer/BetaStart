import { useCallback, useEffect, useMemo, useState } from "react";
import { api } from "./api";
import { useI18n } from "./i18n";
import type { ClipEntry } from "./types";

function preview(
  item: ClipEntry,
  t: (key: string) => string,
): string {
  if (item.kind === "image") return t("Copied image");
  const value = (item.text ?? "").replace(/\s+/g, " ").trim();
  if (!value) return t("(empty)");
  return value.length > 220 ? `${value.slice(0, 220)}…` : value;
}

function formatDate(timestamp: number, locale: string): string {
  return new Date(timestamp * 1000).toLocaleString(locale);
}

function ClipboardThumb({ id }: { id: number }) {
  const { t } = useI18n();
  const [src, setSrc] = useState<string | null>(null);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    if (!open || src) return;
    let active = true;
    api
      .clipboardImage(id)
      .then((value) => {
        if (active) setSrc(value);
      })
      .catch(() => {});
    return () => {
      active = false;
    };
  }, [id, open, src]);

  if (!open) {
    return (
      <button className="secondary" onClick={() => setOpen(true)}>
        {t("Show image")}
      </button>
    );
  }
  if (!src) return <span className="field-hint">{t("Loading image…")}</span>;
  return <img className="clipboard-image-thumb" src={src} alt={t("Clipboard image")} />;
}

export function Clipboard({
  onBack,
  onChanged,
}: {
  onBack: () => void;
  onChanged?: () => void;
}) {
  const { t, locale } = useI18n();
  const [items, setItems] = useState<ClipEntry[]>([]);
  const [query, setQuery] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    try {
      setItems(await api.listClipboard(300));
    } catch (error) {
      setMessage(String(error));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const filtered = useMemo(() => {
    const term = query.trim().toLowerCase();
    if (!term) return items;
    return items.filter((item) =>
      (item.text ?? "").toLowerCase().includes(term),
    );
  }, [items, query]);

  const run = useCallback(
    async (action: () => Promise<unknown>, success?: string) => {
      setBusy(true);
      setMessage(null);
      try {
        await action();
        await refresh();
        onChanged?.();
        if (success) setMessage(success);
      } catch (error) {
        setMessage(String(error));
      } finally {
        setBusy(false);
      }
    },
    [refresh, onChanged],
  );

  const copy = async (item: ClipEntry) => {
    try {
      await api.copyClipboard(item.id);
      setMessage(t("Copied to the clipboard."));
    } catch (error) {
      setMessage(String(error));
    }
  };

  const clearUnpinned = () => {
    const confirmed = window.confirm(
      t("Remove all unpinned items from history?"),
    );
    if (!confirmed) return;
    void run(
      () => api.clearUnpinnedClipboard(),
      t("Unpinned history removed."),
    );
  };

  return (
    <section className="clipboard-screen">
      <div className="ext-header">
        <h1>{t("Clipboard")}</h1>
        <div className="inline">
          <button
            className="secondary"
            disabled={busy}
            onClick={() => void refresh()}
          >
            {t("Refresh")}
          </button>
          <button className="secondary" onClick={onBack}>
            {t("Back")}
          </button>
        </div>
      </div>

      <div className="inline">
        <input
          className="text-input"
          placeholder={t("Filter history…")}
          aria-label={t("Filter history…")}
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          spellCheck={false}
        />
        <button
          className="secondary danger"
          disabled={busy}
          onClick={clearUnpinned}
        >
          {t("Clear unpinned")}
        </button>
      </div>

      <span className="field-hint">
        {t("{count} item(s) · pinned items stay at the top and are not removed by the automatic limit.", {
          count: filtered.length,
        })}
      </span>

      {filtered.length === 0 && (
        <p className="field-hint">{t("No items found.")}</p>
      )}

      {filtered.map((item) => (
        <div
          className={`ext-card${item.pinned ? " is-pinned" : ""}`}
          key={item.id}
        >
          <div className="ext-card-row">
            <div className="clipboard-text">
              {item.kind === "image" ? (
                <ClipboardThumb id={item.id} />
              ) : (
                <div className="clipboard-preview">{preview(item, t)}</div>
              )}
              <div className="field-hint">
                {t(item.kind)} · {formatDate(item.copiedAt, locale)}
                {item.pinned ? ` · ${t("pinned")}` : ""}
              </div>
            </div>
            <div className="ext-actions">
              <button className="secondary" onClick={() => void copy(item)}>
                {t("Copy")}
              </button>
              {item.imagePath && (
                <button
                  className="secondary"
                  onClick={() =>
                    void api
                      .openPath(item.imagePath ?? "")
                      .catch((error) => setMessage(String(error)))
                  }
                >
                  {t("Open")}
                </button>
              )}
              <button
                className="secondary"
                disabled={busy}
                onClick={() =>
                  void run(
                    () => api.pinClipboard(item.id, !item.pinned),
                    item.pinned ? t("Item unpinned.") : t("Item pinned."),
                  )
                }
              >
                {item.pinned ? t("Unpin") : t("Pin")}
              </button>
              <button
                className="secondary danger"
                disabled={busy}
                onClick={() =>
                  void run(
                    () => api.deleteClipboard(item.id),
                    t("Item removed."),
                  )
                }
              >
                {t("Delete")}
              </button>
            </div>
          </div>
        </div>
      ))}

      {message && <p className="settings-msg">{message}</p>}
    </section>
  );
}

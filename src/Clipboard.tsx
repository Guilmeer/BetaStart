import { useCallback, useEffect, useMemo, useState } from "react";
import { api } from "./api";
import type { ClipEntry } from "./types";

function preview(item: ClipEntry): string {
  if (item.kind === "image") return "Imagem copiada";
  const value = (item.text ?? "").replace(/\s+/g, " ").trim();
  if (!value) return "(vazio)";
  return value.length > 220 ? `${value.slice(0, 220)}…` : value;
}

function formatDate(timestamp: number): string {
  return new Date(timestamp * 1000).toLocaleString();
}

function ClipboardThumb({ id }: { id: number }) {
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
        Mostrar imagem
      </button>
    );
  }
  if (!src) return <span className="field-hint">Carregando imagem…</span>;
  return <img className="clipboard-image-thumb" src={src} alt="Clipboard" />;
}

export function Clipboard({
  onBack,
  onChanged,
}: {
  onBack: () => void;
  onChanged?: () => void;
}) {
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
      setMessage("Copiado para a área de transferência.");
    } catch (error) {
      setMessage(String(error));
    }
  };

  const clearUnpinned = () => {
    const confirmed = window.confirm(
      "Remover todos os itens não fixados do histórico?",
    );
    if (!confirmed) return;
    void run(
      () => api.clearUnpinnedClipboard(),
      "Histórico não fixado removido.",
    );
  };

  return (
    <section className="clipboard-screen">
      <div className="ext-header">
        <h1>Área de transferência</h1>
        <div className="inline">
          <button
            className="secondary"
            disabled={busy}
            onClick={() => void refresh()}
          >
            Atualizar
          </button>
          <button className="secondary" onClick={onBack}>
            Voltar
          </button>
        </div>
      </div>

      <div className="inline">
        <input
          className="text-input"
          placeholder="Filtrar histórico…"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          spellCheck={false}
        />
        <button
          className="secondary"
          disabled={busy}
          onClick={clearUnpinned}
        >
          Limpar não fixados
        </button>
      </div>

      <span className="field-hint">
        {filtered.length} item(ns) · itens fixados ficam no topo e não são
        removidos pelo limite automático.
      </span>

      {filtered.length === 0 && (
        <p className="field-hint">Nenhum item encontrado.</p>
      )}

      {filtered.map((item) => (
        <div className="ext-card" key={item.id}>
          <div className="ext-card-row">
            <div className="clipboard-text">
              {item.kind === "image" ? (
                <ClipboardThumb id={item.id} />
              ) : (
                <div className="clipboard-preview">{preview(item)}</div>
              )}
              <div className="field-hint">
                {item.kind} · {formatDate(item.copiedAt)}
                {item.pinned ? " · fixado" : ""}
              </div>
            </div>
            <div className="ext-actions">
              <button className="secondary" onClick={() => void copy(item)}>
                Copiar
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
                  Abrir
                </button>
              )}
              <button
                className="secondary"
                disabled={busy}
                onClick={() =>
                  void run(
                    () => api.pinClipboard(item.id, !item.pinned),
                    item.pinned ? "Item desafixado." : "Item fixado.",
                  )
                }
              >
                {item.pinned ? "Desafixar" : "Fixar"}
              </button>
              <button
                className="secondary"
                disabled={busy}
                onClick={() =>
                  void run(
                    () => api.deleteClipboard(item.id),
                    "Item removido.",
                  )
                }
              >
                Excluir
              </button>
            </div>
          </div>
        </div>
      ))}

      {message && <p className="settings-msg">{message}</p>}
    </section>
  );
}

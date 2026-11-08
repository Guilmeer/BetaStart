import { useCallback, useEffect, useState } from "react";
import { api } from "./api";
import type { McpClient, McpClientCreated, McpStatus } from "./types";

const SCOPES = [
  "memory",
  "tasks",
  "windows",
  "extensions",
  "search",
  "settings",
] as const;

const SCOPE_LABEL: Record<string, string> = {
  memory: "memória",
  tasks: "tasks",
  windows: "janelas",
  extensions: "extensões",
  search: "busca",
  settings: "settings",
};

export function Mcp({ onBack }: { onBack: () => void }) {
  const [status, setStatus] = useState<McpStatus | null>(null);
  const [clients, setClients] = useState<McpClient[]>([]);
  const [exePath, setExePath] = useState("");
  const [name, setName] = useState("");
  const [scopes, setScopes] = useState<string[]>([...SCOPES]);
  const [created, setCreated] = useState<McpClientCreated | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const refresh = useCallback(async () => {
    try {
      const [serverStatus, list, exe] = await Promise.all([
        api.mcpStatus(),
        api.mcpListClients(),
        api.mcpExePath(),
      ]);
      setStatus(serverStatus);
      setClients(list);
      setExePath(exe);
    } catch (error) {
      setMessage(String(error));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const toggleScope = (scope: string) =>
    setScopes((current) =>
      current.includes(scope)
        ? current.filter((value) => value !== scope)
        : [...current, scope],
    );

  const create = async () => {
    setBusy(true);
    setMessage(null);
    try {
      const result = await api.mcpCreateClient(
        name.trim() || "Cliente MCP",
        scopes,
      );
      setCreated(result);
      setName("");
      await refresh();
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  };

  const revoke = async (client: McpClient) => {
    setBusy(true);
    setMessage(null);
    try {
      await api.mcpRevokeClient(client.id);
      await refresh();
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
    }
  };

  const copy = async (text: string) => {
    try {
      await api.copyText(text);
      setMessage("Copiado para a área de transferência.");
    } catch (error) {
      setMessage(String(error));
    }
  };

  const snippet = created
    ? JSON.stringify(
        {
          mcpServers: {
            betastart: {
              command: exePath || "betastart.exe",
              args: ["mcp", "--token", created.token],
            },
          },
        },
        null,
        2,
      )
    : "";

  const httpUrl = status
    ? `http://${status.host}:${status.port}`
    : "http://127.0.0.1:19000";

  return (
    <section className="mcp">
      <div className="ext-header">
        <h1>MCP</h1>
        <button className="secondary" onClick={onBack}>
          Voltar
        </button>
      </div>

      <div className="ext-card">
        <div className="ext-card-row">
          <strong>Servidor</strong>
          <span className={status?.running ? "ok" : "warn"}>
            {status?.running ? "HTTP/SSE ativo" : "HTTP/SSE inativo"}
          </span>
        </div>
        <div className="field-hint">
          {status
            ? `${httpUrl} · ${status.clients} cliente(s) ativo(s)`
            : "carregando…"}
        </div>
        <div className="field-hint">
          stdio: <code>betastart mcp --token &lt;token&gt;</code>
        </div>
      </div>

      <h2>Criar cliente / token</h2>
      <div className="field">
        <input
          className="text-input"
          placeholder="Nome do cliente (ex.: Claude Desktop)"
          value={name}
          onChange={(event) => setName(event.target.value)}
        />
        <div className="scope-grid">
          {SCOPES.map((scope) => (
            <label className="scope" key={scope}>
              <input
                type="checkbox"
                checked={scopes.includes(scope)}
                onChange={() => toggleScope(scope)}
              />
              {SCOPE_LABEL[scope] ?? scope}
            </label>
          ))}
        </div>
        <button
          className="primary"
          disabled={busy || scopes.length === 0}
          onClick={() => void create()}
        >
          Gerar token
        </button>
      </div>

      {created && (
        <div className="ext-card">
          <div className="ext-card-row">
            <strong>Token de {created.client.name}</strong>
            <button
              className="secondary"
              onClick={() => void copy(created.token)}
            >
              Copiar
            </button>
          </div>
          <div className="field-hint">
            Guarde agora — o token não será exibido novamente.
          </div>
          <code className="token-box">{created.token}</code>

          <div className="ext-card-row">
            <strong>Config (stdio)</strong>
            <button className="secondary" onClick={() => void copy(snippet)}>
              Copiar
            </button>
          </div>
          <pre className="snippet">{snippet}</pre>
          <div className="field-hint">
            HTTP/SSE: {httpUrl}/sse?token=&lt;token&gt; · POST {httpUrl}/mcp
          </div>
        </div>
      )}

      <h2>Clientes</h2>
      {clients.length === 0 && (
        <p className="field-hint">Nenhum cliente criado.</p>
      )}
      {clients.map((client) => (
        <div className="ext-card" key={client.id}>
          <div className="ext-card-row">
            <div>
              <strong>{client.name}</strong>{" "}
              <span className={client.revoked ? "warn" : "ok"}>
                {client.revoked ? "revogado" : "ativo"}
              </span>
              <div className="field-hint">
                {client.scopes.length > 0
                  ? client.scopes.join(", ")
                  : "sem escopos"}
              </div>
              <div className="field-hint">
                criado {new Date(client.createdAt * 1000).toLocaleString()}
                {client.lastSeen
                  ? ` · visto ${new Date(client.lastSeen * 1000).toLocaleString()}`
                  : ""}
              </div>
            </div>
            {!client.revoked && (
              <button
                className="secondary"
                disabled={busy}
                onClick={() => void revoke(client)}
              >
                Revogar
              </button>
            )}
          </div>
        </div>
      ))}

      {message && <p className="settings-msg">{message}</p>}
    </section>
  );
}

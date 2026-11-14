import { useCallback, useEffect, useState } from "react";
import { api } from "./api";
import { useI18n } from "./i18n";
import type {
  McpAudit,
  McpClient,
  McpClientCreated,
  McpStatus,
  ProjectInfo,
} from "./types";

const SCOPES = [
  "memory",
  "tasks",
  "windows",
  "extensions",
  "search",
  "settings",
] as const;

const SCOPE_LABEL: Record<string, string> = {
  memory: "Memory",
  tasks: "Tasks",
  windows: "Windows",
  extensions: "Extensions",
  search: "Search",
  settings: "Settings",
};

export function Mcp({ onBack }: { onBack: () => void }) {
  const { t, locale } = useI18n();
  const [status, setStatus] = useState<McpStatus | null>(null);
  const [clients, setClients] = useState<McpClient[]>([]);
  const [audit, setAudit] = useState<McpAudit[]>([]);
  const [exePath, setExePath] = useState("");
  const [logPath, setLogPath] = useState("");
  const [name, setName] = useState("");
  const [scopes, setScopes] = useState<string[]>([...SCOPES]);
  const [created, setCreated] = useState<McpClientCreated | null>(null);
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const [projects, setProjects] = useState<ProjectInfo[]>([]);
  const [guidanceProject, setGuidanceProject] = useState("");
  const [guidancePath, setGuidancePath] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    try {
      const [serverStatus, list, exe, paths, recent, projectList] =
        await Promise.all([
          api.mcpStatus(),
          api.mcpListClients(),
          api.mcpExePath(),
          api.getPaths(),
          api.mcpAudit(40),
          api.listProjects(),
        ]);
      setStatus(serverStatus);
      setClients(list);
      setAudit(recent);
      setProjects(projectList);
      setExePath(exe);
      const separatorIndex = Math.max(
        paths.config.lastIndexOf("\\"),
        paths.config.lastIndexOf("/"),
      );
      const separator = separatorIndex >= 0 ? paths.config[separatorIndex] : "/";
      setLogPath(
        `${paths.config.slice(0, separatorIndex)}${separator}mcp-stdio.log`,
      );
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
        name.trim() || t("MCP client"),
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

  const openLog = async () => {
    try {
      await api.openPath(logPath);
    } catch (error) {
      setMessage(String(error));
    }
  };

  const copy = async (text: string) => {
    try {
      await api.copyText(text);
      setMessage(t("Copied to the clipboard."));
    } catch (error) {
      setMessage(String(error));
    }
  };

  const installGuidance = async () => {
    setBusy(true);
    setMessage(null);
    try {
      const path = await api.installAiGuidance(guidanceProject || null);
      setGuidancePath(path);
      setMessage(t("AI guidance written to AGENTS.md."));
    } catch (error) {
      setMessage(String(error));
    } finally {
      setBusy(false);
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
        <div className="inline">
          <button className="secondary" onClick={() => void refresh()}>
            {t("Refresh")}
          </button>
          <button className="secondary" onClick={onBack}>
            {t("Back")}
          </button>
        </div>
      </div>

      <div className="ext-card">
        <div className="ext-card-row">
          <strong>{t("Server")}</strong>
          <span className={status?.running ? "ok" : "warn"}>
            {status?.running ? t("HTTP/SSE active") : t("HTTP/SSE inactive")}
          </span>
        </div>
        <div className="field-hint">
          {status
            ? t("{url} · {count} active client(s)", {
                url: httpUrl,
                count: status.clients,
              })
            : t("loading…")}
        </div>
        <div className="field-hint">
          stdio: <code>betastart mcp --token &lt;token&gt;</code>
        </div>
      </div>

      <div className="ext-card">
        <div className="ext-card-row">
          <strong>{t("Diagnostic log")}</strong>
          <button
            className="secondary"
            disabled={!logPath}
            onClick={() => void openLog()}
          >
            {t("Open")}
          </button>
        </div>
        <div className="field-hint">
          <code>{logPath || t("loading…")}</code>
        </div>
        <div className="field-hint">
          {t("If a connection drops, the log shows whether the process exited on its own (EOF) or was terminated by the client. Set BETASTART_MCP_TRACE=1 to log every method.")}
        </div>
      </div>

      <div className="ext-card">
        <div className="ext-card-row">
          <strong>{t("AI guidance")}</strong>
        </div>
        <div className="field-hint">
          {t("Writes an AGENTS.md block that teaches your assistant to query this memory instead of re-reading files.")}
        </div>
        <div className="inline">
          <select
            className="text-input"
            value={guidanceProject}
            onChange={(event) => setGuidanceProject(event.target.value)}
          >
            <option value="">{t("Active project")}</option>
            {projects.map((project) => (
              <option key={project.slug} value={project.slug}>
                {project.title}
              </option>
            ))}
          </select>
          <button
            className="secondary"
            disabled={busy}
            onClick={() => void installGuidance()}
          >
            {t("Install guidance")}
          </button>
        </div>
        {guidancePath && <code className="token-box">{guidancePath}</code>}
      </div>

      <h2>{t("Create client / token")}</h2>
      <div className="field">
        <input
          className="text-input"
          placeholder={t("Client name (e.g. Claude Desktop)")}
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
          {t("Generate token")}
        </button>
      </div>

      {created && (
        <div className="ext-card">
          <div className="ext-card-row">
            <strong>{t("Token for {name}", { name: created.client.name })}</strong>
            <button
              className="secondary"
              onClick={() => void copy(created.token)}
            >
              {t("Copy")}
            </button>
          </div>
          <div className="field-hint">
            {t("Save it now — the token will not be shown again.")}
          </div>
          <code className="token-box">{created.token}</code>

          <div className="ext-card-row">
            <strong>{t("Config (stdio)")}</strong>
            <button className="secondary" onClick={() => void copy(snippet)}>
              {t("Copy")}
            </button>
          </div>
          <pre className="snippet">{snippet}</pre>
          <div className="field-hint">
            HTTP/SSE: {httpUrl}/sse?token=&lt;token&gt; · POST {httpUrl}/mcp
          </div>
        </div>
      )}

      <h2>{t("Clients")}</h2>
      {clients.length === 0 && (
        <p className="field-hint">{t("No clients created.")}</p>
      )}
      {clients.map((client) => (
        <div className="ext-card" key={client.id}>
          <div className="ext-card-row">
            <div>
              <strong>{client.name}</strong>{" "}
              <span className={client.revoked ? "warn" : "ok"}>
                {client.revoked ? t("revoked") : t("active")}
              </span>
              <div className="field-hint">
                {client.scopes.length > 0
                  ? client.scopes.map((scope) => t(SCOPE_LABEL[scope] ?? scope)).join(", ")
                  : t("no scopes")}
              </div>
              <div className="field-hint">
                {t("created {date}", {
                  date: new Date(client.createdAt * 1000).toLocaleString(locale),
                })}
                {client.lastSeen
                  ? ` · ${t("seen {date}", {
                      date: new Date(client.lastSeen * 1000).toLocaleString(locale),
                    })}`
                  : ""}
              </div>
            </div>
            {!client.revoked && (
              <button
                className="secondary"
                disabled={busy}
                onClick={() => void revoke(client)}
              >
                {t("Revoke")}
              </button>
            )}
          </div>
        </div>
      ))}

      <h2>{t("Recent calls")}</h2>
      {audit.length === 0 && (
        <p className="field-hint">{t("No calls recorded yet.")}</p>
      )}
      {audit.map((entry) => (
        <div className="ext-card" key={entry.id}>
          <div className="ext-card-row">
            <div>
              <strong>{entry.tool}</strong>{" "}
              <span className={entry.ok ? "ok" : "warn"}>
                {entry.ok ? "ok" : t("error")}
              </span>
              <div className="field-hint">
                {entry.clientName} ·{" "}
                {new Date(entry.createdAt * 1000).toLocaleString(locale)}
              </div>
              {entry.detail && (
                <div className="field-hint">{entry.detail}</div>
              )}
            </div>
          </div>
        </div>
      ))}

      {message && <p className="settings-msg">{message}</p>}
    </section>
  );
}

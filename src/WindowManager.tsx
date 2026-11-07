import { useCallback, useEffect, useState } from "react";
import { api } from "./api";
import { HotkeyInput } from "./HotkeyInput";
import type {
  MonitorInfo,
  WindowInfo,
  WindowLayout,
  WindowRule,
} from "./types";

export function WindowManager({ onBack }: { onBack: () => void }) {
  const [monitors, setMonitors] = useState<MonitorInfo[]>([]);
  const [layouts, setLayouts] = useState<WindowLayout[]>([]);
  const [rules, setRules] = useState<WindowRule[]>([]);
  const [windows, setWindows] = useState<WindowInfo[]>([]);
  const [message, setMessage] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  const [layoutName, setLayoutName] = useState("");
  const [layoutHotkey, setLayoutHotkey] = useState("");

  const [ruleName, setRuleName] = useState("");
  const [ruleApp, setRuleApp] = useState("");
  const [ruleField, setRuleField] = useState("process");
  const [ruleMonitor, setRuleMonitor] = useState(0);
  const [ruleX, setRuleX] = useState("0");
  const [ruleY, setRuleY] = useState("0");
  const [ruleW, setRuleW] = useState("1");
  const [ruleH, setRuleH] = useState("1");

  const refresh = useCallback(async () => {
    try {
      const [mons, lays, rls, wins] = await Promise.all([
        api.listMonitors(),
        api.listWindowLayouts(),
        api.listWindowRules(),
        api.listWindowsWithMonitor(),
      ]);
      setMonitors(mons);
      setLayouts(lays);
      setRules(rls);
      setWindows(wins);
    } catch (error) {
      setMessage(String(error));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const run = useCallback(
    async (action: () => Promise<unknown>) => {
      setBusy(true);
      setMessage(null);
      try {
        await action();
        await refresh();
      } catch (error) {
        setMessage(String(error));
      } finally {
        setBusy(false);
      }
    },
    [refresh],
  );

  const saveLayout = () =>
    run(async () => {
      const layout = await api.saveCurrentLayout(
        layoutName.trim() || "Layout",
        layoutHotkey.trim() || null,
      );
      setLayoutName("");
      setLayoutHotkey("");
      setMessage(
        `Layout "${layout.name}" salvo com ${layout.slots.length} janela(s).`,
      );
    });

  const applyLayout = (layout: WindowLayout) =>
    run(async () => {
      const result = await api.applyWindowLayout(layout.id);
      setMessage(
        `Layout aplicado: ${result.moved} janela(s)` +
          (result.missed.length
            ? `; não encontradas: ${result.missed.join(", ")}`
            : "."),
      );
    });

  const applyRules = () =>
    run(async () => {
      const result = await api.applyWindowRules();
      setMessage(
        `Regras aplicadas: ${result.moved} janela(s)` +
          (result.missed.length
            ? `; sem janela: ${result.missed.join(", ")}`
            : "."),
      );
    });

  const createRule = () =>
    run(async () => {
      await api.upsertWindowRule({
        id: 0,
        name: ruleName.trim() || ruleApp.trim(),
        appMatch: ruleApp.trim(),
        matchField: ruleField,
        monitor: ruleMonitor,
        x: Number(ruleX) || 0,
        y: Number(ruleY) || 0,
        w: Number(ruleW) || 1,
        h: Number(ruleH) || 1,
        enabled: true,
      });
      setRuleName("");
      setRuleApp("");
      setMessage("Regra salva.");
    });

  return (
    <section className="windows">
      <div className="ext-header">
        <h1>Gerenciador de janelas</h1>
        <button className="secondary" onClick={onBack}>
          Voltar
        </button>
      </div>

      <div className="ext-card">
        <div className="ext-card-row">
          <strong>Monitores</strong>
          <span className="field-hint">{windows.length} janelas abertas</span>
        </div>
        {monitors.map((monitor) => (
          <div className="monitor-row" key={monitor.index}>
            <span className="monitor-index">{monitor.index + 1}</span>
            <span>{monitor.name || `Monitor ${monitor.index + 1}`}</span>
            <span className="field-hint">
              {monitor.width}×{monitor.height}
              {monitor.primary ? " · principal" : ""}
            </span>
          </div>
        ))}
      </div>

      <h2>Layouts</h2>
      <div className="inline">
        <input
          className="text-input"
          placeholder="Nome do layout"
          value={layoutName}
          onChange={(event) => setLayoutName(event.target.value)}
        />
        <HotkeyInput
          value={layoutHotkey}
          onChange={(combo) => setLayoutHotkey(combo)}
          placeholder="Atalho (ex.: Win+Shift+F7)"
        />
        <button className="primary" disabled={busy} onClick={() => void saveLayout()}>
          Salvar atual
        </button>
      </div>
      <span className="field-hint">
        Organize as janelas manualmente (ex.: navegador 25% à direita, Discord 75%
        à esquerda) e salve. O atalho reaplica depois.
      </span>

      {layouts.map((layout) => (
        <div className="ext-card" key={layout.id}>
          <div className="ext-card-row">
            <div>
              <strong>{layout.name}</strong>{" "}
              <span className="ext-id">
                {layout.slots.length} janela(s)
                {layout.hotkey ? ` · ${layout.hotkey}` : ""}
              </span>
            </div>
            <div className="ext-actions">
              <button
                className="secondary"
                disabled={busy}
                onClick={() => void applyLayout(layout)}
              >
                Aplicar
              </button>
              <button
                className="secondary"
                disabled={busy}
                onClick={() =>
                  void run(() => api.deleteWindowLayout(layout.id))
                }
              >
                Excluir
              </button>
            </div>
          </div>
          <HotkeyInput
            value={layout.hotkey ?? ""}
            excludeLayoutId={layout.id}
            placeholder="Sem atalho — clique para gravar"
            onChange={async (combo) => {
              await api.setWindowLayoutHotkey(layout.id, combo || null);
              await refresh();
            }}
          />
        </div>
      ))}

      <h2>Regras por app</h2>
      <div className="rule-form">
        <div className="inline">
          <input
            className="text-input"
            placeholder="Nome da regra"
            value={ruleName}
            onChange={(event) => setRuleName(event.target.value)}
          />
          <input
            className="text-input"
            placeholder="Processo/título/classe (ex.: chrome)"
            value={ruleApp}
            onChange={(event) => setRuleApp(event.target.value)}
          />
        </div>
        <div className="inline">
          <select
            className="text-input"
            value={ruleField}
            onChange={(event) => setRuleField(event.target.value)}
          >
            <option value="process">processo</option>
            <option value="title">título</option>
            <option value="class">classe</option>
          </select>
          <select
            className="text-input"
            value={ruleMonitor}
            onChange={(event) => setRuleMonitor(Number(event.target.value))}
          >
            {monitors.map((monitor) => (
              <option key={monitor.index} value={monitor.index}>
                Monitor {monitor.index + 1}
              </option>
            ))}
          </select>
        </div>
        <div className="inline">
          <input
            className="text-input"
            placeholder="x (0–1)"
            value={ruleX}
            onChange={(event) => setRuleX(event.target.value)}
          />
          <input
            className="text-input"
            placeholder="y (0–1)"
            value={ruleY}
            onChange={(event) => setRuleY(event.target.value)}
          />
          <input
            className="text-input"
            placeholder="w (0–1)"
            value={ruleW}
            onChange={(event) => setRuleW(event.target.value)}
          />
          <input
            className="text-input"
            placeholder="h (0–1)"
            value={ruleH}
            onChange={(event) => setRuleH(event.target.value)}
          />
        </div>
        <div className="inline">
          <button
            className="primary"
            disabled={busy || !ruleApp.trim()}
            onClick={() => void createRule()}
          >
            Adicionar regra
          </button>
          <button
            className="secondary"
            disabled={busy}
            onClick={() => void applyRules()}
          >
            Aplicar regras agora
          </button>
        </div>
      </div>
      <span className="field-hint">
        Regras são aplicadas automaticamente quando uma janela nova do app
        aparece (polling ~1,2s). x/y/w/h são frações da área útil do monitor.
      </span>

      {rules.map((rule) => (
        <div className="ext-card" key={rule.id}>
          <div className="ext-card-row">
            <div>
              <strong>{rule.name}</strong>{" "}
              <span className="ext-id">
                {rule.appMatch} · {rule.matchField} · monitor{" "}
                {rule.monitor + 1} · {Math.round(rule.w * 100)}%×
                {Math.round(rule.h * 100)}%
              </span>
            </div>
            <button
              className="secondary"
              disabled={busy}
              onClick={() => void run(() => api.deleteWindowRule(rule.id))}
            >
              Excluir
            </button>
          </div>
        </div>
      ))}

      {message && <p className="settings-msg">{message}</p>}
    </section>
  );
}

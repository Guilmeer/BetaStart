import { useCallback, useEffect, useMemo, useState } from "react";
import { api } from "./api";
import { HotkeyInput } from "./HotkeyInput";
import { useI18n } from "./i18n";
import type {
  MonitorInfo,
  WindowInfo,
  WindowLayout,
  WindowRule,
} from "./types";

export function WindowManager({ onBack }: { onBack: () => void }) {
  const { t } = useI18n();
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

  const pickableWindows = useMemo(
    () =>
      windows
        .map((win, index) => ({ win, index }))
        .filter(
          ({ win }) =>
            win.title.trim() && win.process.toLowerCase() !== "betastart",
        ),
    [windows],
  );

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
        t('Layout "{name}" saved with {count} window(s).', {
          name: layout.name,
          count: layout.slots.length,
        }),
      );
    });

  const applyLayout = (layout: WindowLayout) =>
    run(async () => {
      const result = await api.applyWindowLayout(layout.id);
      setMessage(
        t("Layout applied: {moved} window(s)", { moved: result.moved }) +
          (result.missed.length
            ? `; ${t("not found: {list}", { list: result.missed.join(", ") })}`
            : "."),
      );
    });

  const applyRules = () =>
    run(async () => {
      const result = await api.applyWindowRules();
      setMessage(
        t("Rules applied: {moved} window(s)", { moved: result.moved }) +
          (result.missed.length
            ? `; ${t("no window: {list}", { list: result.missed.join(", ") })}`
            : "."),
      );
    });

  const pickWindow = (value: string) => {
    const win = windows[Number(value)];
    if (!win) return;
    setRuleApp(win.process);
    setRuleField("process");
    setRuleMonitor(win.monitor);
    if (!ruleName.trim()) {
      setRuleName(
        `${win.process}${win.title ? ` — ${win.title.slice(0, 40)}` : ""}`,
      );
    }
  };

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
      setMessage(t("Rule saved."));
    });

  return (
    <section className="windows">
      <div className="ext-header">
        <h1>{t("Window manager")}</h1>
        <button className="secondary" onClick={onBack}>
          {t("Back")}
        </button>
      </div>

      <div className="ext-card">
        <div className="ext-card-row">
          <strong>{t("Monitors")}</strong>
          <span className="field-hint">
            {t("{count} open windows", { count: windows.length })}
          </span>
        </div>
        {monitors.map((monitor) => (
          <div className="monitor-row" key={monitor.index}>
            <span className="monitor-index">{monitor.index + 1}</span>
            <span>{monitor.name || t("Monitor {number}", { number: monitor.index + 1 })}</span>
            <span className="field-hint">
              {monitor.width}×{monitor.height}
              {monitor.primary ? ` · ${t("primary")}` : ""}
            </span>
          </div>
        ))}
      </div>

      <h2>{t("Layouts")}</h2>
      <div className="inline">
        <input
          className="text-input"
          placeholder={t("Layout name")}
          value={layoutName}
          onChange={(event) => setLayoutName(event.target.value)}
        />
        <HotkeyInput
          value={layoutHotkey}
          onChange={(combo) => setLayoutHotkey(combo)}
          placeholder={t("Shortcut (e.g. Win+Shift+F7)")}
        />
        <button className="primary" disabled={busy} onClick={() => void saveLayout()}>
          {t("Save current")}
        </button>
      </div>
      <span className="field-hint">
        {t("Arrange the windows manually (e.g. browser 25% on the right, Discord 75% on the left) and save. The shortcut reapplies it later.")}
      </span>

      {layouts.map((layout) => (
        <div className="ext-card" key={layout.id}>
          <div className="ext-card-row">
            <div>
              <strong>{layout.name}</strong>{" "}
              <span className="ext-id">
                {t("{count} window(s)", { count: layout.slots.length })}
                {layout.hotkey ? ` · ${layout.hotkey}` : ""}
              </span>
            </div>
            <div className="ext-actions">
              <button
                className="secondary"
                disabled={busy}
                onClick={() => void applyLayout(layout)}
              >
                {t("Apply")}
              </button>
              <button
                className="secondary"
                disabled={busy}
                onClick={() =>
                  void run(() => api.deleteWindowLayout(layout.id))
                }
              >
                {t("Delete")}
              </button>
            </div>
          </div>
          <HotkeyInput
            value={layout.hotkey ?? ""}
            excludeLayoutId={layout.id}
            placeholder={t("No shortcut — click to record")}
            onChange={async (combo) => {
              await api.setWindowLayoutHotkey(layout.id, combo || null);
              await refresh();
            }}
          />
        </div>
      ))}

      <h2>{t("Per-app rules")}</h2>
      <div className="rule-form">
        <div className="inline">
          <select
            className="text-input"
            defaultValue=""
            onChange={(event) => pickWindow(event.target.value)}
          >
            <option value="">{t("Capture open window…")}</option>
            {pickableWindows.map(({ win, index }) => (
              <option key={`${win.handle}-${index}`} value={index}>
                {win.process} — {win.title.slice(0, 60)}
              </option>
            ))}
          </select>
          <button className="secondary" onClick={() => void refresh()}>
            {t("Refresh windows")}
          </button>
        </div>
        <div className="inline">
          <input
            className="text-input"
            placeholder={t("Rule name")}
            value={ruleName}
            onChange={(event) => setRuleName(event.target.value)}
          />
          <input
            className="text-input"
            placeholder={t("Process/title/class (e.g. chrome)")}
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
            <option value="process">{t("process")}</option>
            <option value="title">{t("title")}</option>
            <option value="class">{t("class")}</option>
          </select>
          <select
            className="text-input"
            value={ruleMonitor}
            onChange={(event) => setRuleMonitor(Number(event.target.value))}
          >
            {monitors.map((monitor) => (
              <option key={monitor.index} value={monitor.index}>
                {t("Monitor {number}", { number: monitor.index + 1 })}
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
            {t("Add rule")}
          </button>
          <button
            className="secondary"
            disabled={busy}
            onClick={() => void applyRules()}
          >
            {t("Apply rules now")}
          </button>
        </div>
      </div>
      <span className="field-hint">
        {t("Rules are applied automatically when a new window for the app appears (polling ~1.2s). x/y/w/h are fractions of the monitor work area.")}
      </span>

      {rules.map((rule) => (
        <div className="ext-card" key={rule.id}>
          <div className="ext-card-row">
            <div>
              <strong>{rule.name}</strong>{" "}
              <span className="ext-id">
                {rule.appMatch} · {t(rule.matchField)} ·{" "}
                {t("monitor {number}", { number: rule.monitor + 1 })} ·{" "}
                {Math.round(rule.w * 100)}%×{Math.round(rule.h * 100)}%
              </span>
            </div>
            <button
              className="secondary"
              disabled={busy}
              onClick={() => void run(() => api.deleteWindowRule(rule.id))}
            >
              {t("Delete")}
            </button>
          </div>
        </div>
      ))}

      {message && <p className="settings-msg">{message}</p>}
    </section>
  );
}

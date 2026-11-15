import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { PointerEvent as ReactPointerEvent } from "react";
import { api } from "./api";
import { HotkeyInput } from "./HotkeyInput";
import { Icon } from "./Icons";
import { Select } from "./Select";
import { useI18n } from "./i18n";
import type {
  MonitorInfo,
  WindowInfo,
  WindowLayout,
  WindowRule,
} from "./types";

interface Region {
  x: number;
  y: number;
  w: number;
  h: number;
}

const FULL_REGION: Region = { x: 0, y: 0, w: 1, h: 1 };

const REGION_PRESETS: Array<{ label: string; region: Region }> = [
  { label: "Full monitor", region: { x: 0, y: 0, w: 1, h: 1 } },
  { label: "Left half", region: { x: 0, y: 0, w: 0.5, h: 1 } },
  { label: "Right half", region: { x: 0.5, y: 0, w: 0.5, h: 1 } },
  { label: "Top half", region: { x: 0, y: 0, w: 1, h: 0.5 } },
  { label: "Bottom half", region: { x: 0, y: 0.5, w: 1, h: 0.5 } },
  { label: "Center", region: { x: 0.25, y: 0.25, w: 0.5, h: 0.5 } },
  { label: "Top left", region: { x: 0, y: 0, w: 0.5, h: 0.5 } },
  { label: "Bottom right", region: { x: 0.5, y: 0.5, w: 0.5, h: 0.5 } },
];

const clamp01 = (value: number) =>
  Number.isFinite(value) ? Math.min(1, Math.max(0, value)) : 0;

const roundCoord = (value: number) => Math.round(clamp01(value) * 1000) / 1000;

function clampRegion(region: Region): Region {
  const x = roundCoord(region.x);
  const y = roundCoord(region.y);
  return {
    x,
    y,
    w: roundCoord(Math.min(region.w, 1 - x)),
    h: roundCoord(Math.min(region.h, 1 - y)),
  };
}

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
  const [ruleX, setRuleX] = useState(0);
  const [ruleY, setRuleY] = useState(0);
  const [ruleW, setRuleW] = useState(1);
  const [ruleH, setRuleH] = useState(1);
  const [ruleEnabled, setRuleEnabled] = useState(true);
  const [ruleSource, setRuleSource] = useState("");
  const [editingRuleId, setEditingRuleId] = useState<number | null>(null);
  const [drag, setDrag] = useState<{
    startX: number;
    startY: number;
    origin: Region;
  } | null>(null);

  const ruleCanvasRef = useRef<HTMLDivElement>(null);
  const ruleBuilderRef = useRef<HTMLDivElement>(null);

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

  const monitorFor = (index: number) =>
    monitors.find((monitor) => monitor.index === index) ?? monitors[0];

  const ruleMonitorInfo = monitorFor(ruleMonitor);
  const ruleAspect = ruleMonitorInfo
    ? (ruleMonitorInfo.workWidth || ruleMonitorInfo.width) /
      (ruleMonitorInfo.workHeight || ruleMonitorInfo.height)
    : 16 / 9;

  const setRegion = (region: Region) => {
    const next = clampRegion(region);
    setRuleX(next.x);
    setRuleY(next.y);
    setRuleW(next.w);
    setRuleH(next.h);
  };

  const resetRuleForm = () => {
    setEditingRuleId(null);
    setRuleSource("");
    setRuleName("");
    setRuleApp("");
    setRuleField("process");
    setRuleMonitor(0);
    setRegion(FULL_REGION);
    setRuleEnabled(true);
    setDrag(null);
  };

  const startEditRule = (rule: WindowRule) => {
    setEditingRuleId(rule.id);
    setRuleSource("");
    setRuleName(rule.name);
    setRuleApp(rule.appMatch);
    setRuleField(rule.matchField);
    setRuleMonitor(rule.monitor);
    setRegion({ x: rule.x, y: rule.y, w: rule.w, h: rule.h });
    setRuleEnabled(rule.enabled);
    window.setTimeout(() => {
      ruleBuilderRef.current?.scrollIntoView({
        behavior: window.matchMedia("(prefers-reduced-motion: reduce)").matches
          ? "auto"
          : "smooth",
        block: "start",
      });
    }, 0);
  };

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

  useEffect(() => {
    if (!message) return;
    const timer = window.setTimeout(() => setMessage(null), 4500);
    return () => window.clearTimeout(timer);
  }, [message]);

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

  const saveRule = () => {
    const match = ruleApp.trim();
    if (!match || ruleW < 0.05 || ruleH < 0.05) return;
    const region = clampRegion({ x: ruleX, y: ruleY, w: ruleW, h: ruleH });
    void run(async () => {
      await api.upsertWindowRule({
        id: editingRuleId ?? 0,
        name: ruleName.trim() || match,
        appMatch: match,
        matchField: ruleField,
        monitor: ruleMonitor,
        x: region.x,
        y: region.y,
        w: region.w,
        h: region.h,
        enabled: ruleEnabled,
      });
      const wasEditing = editingRuleId !== null;
      resetRuleForm();
      setMessage(wasEditing ? t("Rule updated.") : t("Rule saved."));
    });
  };

  const toggleRule = (rule: WindowRule) => {
    void run(async () => {
      await api.upsertWindowRule({ ...rule, enabled: !rule.enabled });
      setMessage(rule.enabled ? t("Rule disabled.") : t("Rule enabled."));
    });
  };

  const pointInCanvas = (event: ReactPointerEvent<HTMLDivElement>) => {
    const rect = event.currentTarget.getBoundingClientRect();
    return {
      x:
        rect.width > 0
          ? clamp01((event.clientX - rect.left) / rect.width)
          : 0,
      y:
        rect.height > 0
          ? clamp01((event.clientY - rect.top) / rect.height)
          : 0,
    };
  };

  const startDraw = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (event.pointerType === "mouse" && event.button !== 0) return;
    event.preventDefault();
    const point = pointInCanvas(event);
    event.currentTarget.setPointerCapture(event.pointerId);
    setDrag({
      startX: point.x,
      startY: point.y,
      origin: { x: ruleX, y: ruleY, w: ruleW, h: ruleH },
    });
    setRegion({ x: point.x, y: point.y, w: 0, h: 0 });
  };

  const moveDraw = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (!drag) return;
    const point = pointInCanvas(event);
    setRegion({
      x: Math.min(drag.startX, point.x),
      y: Math.min(drag.startY, point.y),
      w: Math.abs(point.x - drag.startX),
      h: Math.abs(point.y - drag.startY),
    });
  };

  const endDraw = (event: ReactPointerEvent<HTMLDivElement>) => {
    if (!drag) return;
    const point = pointInCanvas(event);
    const candidate = clampRegion({
      x: Math.min(drag.startX, point.x),
      y: Math.min(drag.startY, point.y),
      w: Math.abs(point.x - drag.startX),
      h: Math.abs(point.y - drag.startY),
    });
    const tooSmall = candidate.w < 0.03 || candidate.h < 0.03;
    setRegion(tooSmall ? drag.origin : candidate);
    setDrag(null);
    try {
      event.currentTarget.releasePointerCapture(event.pointerId);
    } catch {
      // O ponteiro pode ter sido cancelado pelo sistema.
    }
  };

  const cancelDraw = () => {
    if (!drag) return;
    setRegion(drag.origin);
    setDrag(null);
  };

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
          aria-label={t("Layout name")}
          value={layoutName}
          onChange={(event) => setLayoutName(event.target.value)}
        />
        <HotkeyInput
          value={layoutHotkey}
          onChange={(combo) => setLayoutHotkey(combo)}
          label={t("Shortcut")}
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
                className="secondary danger"
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
            label={t("Shortcut")}
            placeholder={t("No shortcut — click to record")}
            onChange={async (combo) => {
              await api.setWindowLayoutHotkey(layout.id, combo || null);
              await refresh();
            }}
          />
        </div>
      ))}

      <div className="rules-head" ref={ruleBuilderRef}>
        <div>
          <h2>{t("Per-app rules")}</h2>
          <span className="field-hint">
            {t("Move matching windows automatically when they appear, or apply them on demand.")}
          </span>
        </div>
        <button
          className="secondary"
          disabled={busy}
          onClick={() => void applyRules()}
        >
          {t("Apply rules now")}
        </button>
      </div>

      <div
        className={`rule-builder${editingRuleId !== null ? " is-editing" : ""}`}
      >
        <div className="rule-builder-head">
          <div>
            <strong>
              {editingRuleId !== null ? t("Edit rule") : t("New rule")}
            </strong>
            <span className="field-hint">
              {t("Choose an app, then draw the target area on the monitor.")}
            </span>
          </div>
          {editingRuleId !== null && (
            <button
              className="secondary"
              type="button"
              onClick={resetRuleForm}
            >
              {t("Cancel")}
            </button>
          )}
        </div>

        <div className="rule-builder-grid">
          <div className="rule-source">
            <div className="field">
              <span className="field-label">
                {t("Capture an open window (optional)")}
              </span>
              <div className="inline">
                <Select
                  aria-label={t("Capture open window…")}
                  value={ruleSource}
                  onChange={(event) => {
                    setRuleSource(event.target.value);
                    pickWindow(event.target.value);
                  }}
                >
                  <option value="">{t("Choose a window…")}</option>
                  {pickableWindows.map(({ win, index }) => (
                    <option key={`${win.handle}-${index}`} value={index}>
                      {win.process} — {win.title.slice(0, 60)}
                    </option>
                  ))}
                </Select>
                <button
                  className="secondary"
                  type="button"
                  onClick={() => void refresh()}
                >
                  {t("Refresh")}
                </button>
              </div>
            </div>

            <div className="inline">
              <Select
                aria-label={t("Match field")}
                value={ruleField}
                onChange={(event) => setRuleField(event.target.value)}
              >
                <option value="process">{t("process")}</option>
                <option value="title">{t("title")}</option>
                <option value="class">{t("class")}</option>
              </Select>
              <input
                className="text-input"
                placeholder={t("e.g. chrome or Spotify")}
                aria-label={t("Match value")}
                value={ruleApp}
                onChange={(event) => setRuleApp(event.target.value)}
              />
            </div>

            <input
              className="text-input"
              placeholder={t("Rule name (optional)")}
              aria-label={t("Rule name")}
              value={ruleName}
              onChange={(event) => setRuleName(event.target.value)}
            />

            <label className="check-row">
              <input
                type="checkbox"
                checked={ruleEnabled}
                onChange={(event) => setRuleEnabled(event.target.checked)}
              />
              <span>{t("Rule enabled")}</span>
            </label>
            <span className="field-hint">
              {t("The rule moves any window whose process, title, or class matches this value. Picking an open window fills the fields for you.")}
            </span>
          </div>

          <div className="rule-target">
            <div className="rule-target-head">
              <span className="field-label">{t("Target area")}</span>
              <Select
                className="rule-monitor-select"
                aria-label={t("Monitor")}
                value={ruleMonitor}
                onChange={(event) =>
                  setRuleMonitor(Number(event.target.value))
                }
              >
                {monitors.map((monitor) => (
                  <option key={monitor.index} value={monitor.index}>
                    {monitor.name ||
                      t("Monitor {number}", { number: monitor.index + 1 })}
                    {" — "}
                    {monitor.width}×{monitor.height}
                  </option>
                ))}
              </Select>
            </div>
            <div
              className={`rule-canvas${drag ? " is-dragging" : ""}`}
              ref={ruleCanvasRef}
              style={{ aspectRatio: `${ruleAspect}` }}
              aria-hidden="true"
              onPointerDown={startDraw}
              onPointerMove={moveDraw}
              onPointerUp={endDraw}
              onPointerCancel={cancelDraw}
            >
              <div className="rule-canvas-grid" />
              <div
                className={`rule-region${
                  ruleW < 0.15 || ruleH < 0.15 ? " is-tiny" : ""
                }`}
                style={{
                  left: `${ruleX * 100}%`,
                  top: `${ruleY * 100}%`,
                  width: `${ruleW * 100}%`,
                  height: `${ruleH * 100}%`,
                }}
              >
                <span>
                  {Math.round(ruleW * 100)}% × {Math.round(ruleH * 100)}%
                </span>
              </div>
            </div>
            <div
              className="zone-presets"
              role="group"
              aria-label={t("Region presets")}
            >
              {REGION_PRESETS.map((preset) => (
                <button
                  key={preset.label}
                  type="button"
                  className="zone-chip"
                  onClick={() => setRegion(preset.region)}
                >
                  {t(preset.label)}
                </button>
              ))}
            </div>
            <span className="field-hint">
              {t("Drag on the monitor to draw the area, or pick a preset.")}
            </span>
          </div>
        </div>

        <details className="rule-advanced">
          <summary>{t("Advanced coordinates (0–1)")}</summary>
          <div className="rule-coords">
            <label className="field">
              <span className="field-label">X</span>
              <input
                type="number"
                className="text-input"
                min={0}
                max={1}
                step={0.05}
                value={ruleX}
                onChange={(event) =>
                  setRuleX(clamp01(Number(event.target.value)))
                }
              />
            </label>
            <label className="field">
              <span className="field-label">Y</span>
              <input
                type="number"
                className="text-input"
                min={0}
                max={1}
                step={0.05}
                value={ruleY}
                onChange={(event) =>
                  setRuleY(clamp01(Number(event.target.value)))
                }
              />
            </label>
            <label className="field">
              <span className="field-label">{t("Width")}</span>
              <input
                type="number"
                className="text-input"
                min={0}
                max={1}
                step={0.05}
                value={ruleW}
                onChange={(event) =>
                  setRuleW(clamp01(Number(event.target.value)))
                }
              />
            </label>
            <label className="field">
              <span className="field-label">{t("Height")}</span>
              <input
                type="number"
                className="text-input"
                min={0}
                max={1}
                step={0.05}
                value={ruleH}
                onChange={(event) =>
                  setRuleH(clamp01(Number(event.target.value)))
                }
              />
            </label>
          </div>
        </details>

        <div className="rule-actions">
          <button
            className="primary"
            type="button"
            disabled={
              busy || !ruleApp.trim() || ruleW < 0.05 || ruleH < 0.05
            }
            onClick={() => void saveRule()}
          >
            {editingRuleId !== null ? t("Update rule") : t("Add rule")}
          </button>
          <button
            className="secondary"
            type="button"
            onClick={resetRuleForm}
          >
            {editingRuleId !== null ? t("Cancel") : t("Reset")}
          </button>
        </div>
      </div>

      <div className="rules-list">
        {rules.length === 0 ? (
          <div className="rules-empty">
            <span className="rules-empty-icon" aria-hidden="true">
              <Icon name="windows" size={20} />
            </span>
            <div>
              <strong>{t("No rules yet")}</strong>
              <span className="field-hint">
                {t("Pick a window above and draw where it should go.")}
              </span>
            </div>
          </div>
        ) : (
          rules.map((rule) => {
            const monitor = monitorFor(rule.monitor);
            const ratio = monitor
              ? (monitor.workWidth || monitor.width) /
                (monitor.workHeight || monitor.height)
              : 16 / 9;
            return (
              <div
                className={`rule-item${
                  rule.enabled ? "" : " is-disabled"
                }${editingRuleId === rule.id ? " is-editing" : ""}`}
                key={rule.id}
              >
                <div
                  className="rule-mini"
                  style={{ aspectRatio: `${ratio}` }}
                  aria-hidden="true"
                >
                  <span
                    style={{
                      left: `${rule.x * 100}%`,
                      top: `${rule.y * 100}%`,
                      width: `${rule.w * 100}%`,
                      height: `${rule.h * 100}%`,
                    }}
                  />
                </div>
                <div className="rule-item-info">
                  <div className="rule-item-title">
                    <strong>{rule.name}</strong>
                    <span
                      className={`rule-state ${
                        rule.enabled ? "on" : "off"
                      }`}
                    >
                      {rule.enabled ? t("Enabled") : t("Disabled")}
                    </span>
                  </div>
                  <div className="rule-item-meta">
                    <span className="rule-chip">
                      <span className="rule-chip-key">
                        {t(rule.matchField)}
                      </span>
                      {rule.appMatch}
                    </span>
                    <span>
                      {t("Monitor {number}", { number: rule.monitor + 1 })}
                    </span>
                    <span>
                      {Math.round(rule.x * 100)}%,{" "}
                      {Math.round(rule.y * 100)}% ·{" "}
                      {Math.round(rule.w * 100)}%×{Math.round(rule.h * 100)}%
                    </span>
                  </div>
                </div>
                <div className="rule-item-actions">
                  <button
                    className="secondary"
                    type="button"
                    onClick={() => startEditRule(rule)}
                  >
                    {t("Edit")}
                  </button>
                  <button
                    className={`rule-switch${rule.enabled ? " is-on" : ""}`}
                    type="button"
                    role="switch"
                    aria-checked={rule.enabled}
                    aria-label={
                      rule.enabled ? t("Disable rule") : t("Enable rule")
                    }
                    title={
                      rule.enabled ? t("Disable rule") : t("Enable rule")
                    }
                    onClick={() => toggleRule(rule)}
                  >
                    <span className="rule-switch-knob" />
                  </button>
                  <button
                    className="secondary danger"
                    type="button"
                    disabled={busy}
                    onClick={() =>
                      void run(() => api.deleteWindowRule(rule.id))
                    }
                  >
                    {t("Delete")}
                  </button>
                </div>
              </div>
            );
          })
        )}
      </div>

      {message && (
        <div className="toast" role="status" aria-live="polite">
          {message}
        </div>
      )}
    </section>
  );
}

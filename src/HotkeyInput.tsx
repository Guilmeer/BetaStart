import { useCallback, useEffect, useState } from "react";
import { api } from "./api";
import { useI18n } from "./i18n";

interface HotkeyInputProps {
  value: string;
  onChange: (combo: string) => void | Promise<void>;
  excludeLayoutId?: number;
  excludeLauncher?: boolean;
  placeholder?: string;
  disabled?: boolean;
  label?: string;
}

function keyNameFromCode(code: string): string | null {
  if (code.startsWith("Key") && code.length === 4) return code.slice(3);
  if (code.startsWith("Digit") && code.length === 6) return code.slice(5);
  if (/^F\d{1,2}$/.test(code)) return code;

  switch (code) {
    case "Space":
      return "Space";
    case "Enter":
      return "Enter";
    case "NumpadEnter":
      return "NumpadEnter";
    case "Tab":
      return "Tab";
    case "Backspace":
      return "Backspace";
    case "Delete":
      return "Delete";
    case "Insert":
      return "Insert";
    case "Home":
      return "Home";
    case "End":
      return "End";
    case "PageUp":
      return "PageUp";
    case "PageDown":
      return "PageDown";
    case "ArrowUp":
      return "Up";
    case "ArrowDown":
      return "Down";
    case "ArrowLeft":
      return "Left";
    case "ArrowRight":
      return "Right";
    case "Minus":
      return "Minus";
    case "Equal":
      return "Equal";
    case "BracketLeft":
      return "BracketLeft";
    case "BracketRight":
      return "BracketRight";
    case "Backslash":
      return "Backslash";
    case "Semicolon":
      return "Semicolon";
    case "Quote":
      return "Quote";
    case "Comma":
      return "Comma";
    case "Period":
      return "Period";
    case "Slash":
      return "Slash";
    case "Backquote":
      return "Backquote";
    default:
      if (code.startsWith("Numpad")) return code;
      return null;
  }
}

export function HotkeyInput({
  value,
  onChange,
  excludeLayoutId,
  excludeLauncher,
  placeholder,
  disabled,
  label,
}: HotkeyInputProps) {
  const { t } = useI18n();
  const [listening, setListening] = useState(false);
  const [draft, setDraft] = useState(value);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setDraft(value);
  }, [value]);

  const stop = useCallback((resume: boolean) => {
    setListening(false);
    delete document.body.dataset.recordingHotkey;
    if (resume) void api.resumeHotkeys().catch(() => {});
  }, []);

  const start = useCallback(async () => {
    if (disabled) return;
    setError(null);
    setDraft("");
    document.body.dataset.recordingHotkey = "true";
    try {
      await api.suspendHotkeys();
      setListening(true);
    } catch (error) {
      delete document.body.dataset.recordingHotkey;
      setError(String(error));
    }
  }, [disabled]);

  const finalize = useCallback(
    async (combo: string) => {
      setDraft(combo);
      setListening(false);
      delete document.body.dataset.recordingHotkey;
      try {
        const check = await api.validateHotkey(
          combo,
          excludeLayoutId ?? null,
          Boolean(excludeLauncher),
        );
        if (!check.valid) {
          setError(check.conflict ?? t("invalid shortcut"));
          return;
        }
        setError(null);
        await onChange(combo);
      } catch (error) {
        setError(String(error));
      } finally {
        void api.resumeHotkeys().catch(() => {});
      }
    },
    [excludeLayoutId, excludeLauncher, onChange, t],
  );

  useEffect(() => {
    if (!listening) return;

    const onKey = (event: KeyboardEvent) => {
      event.preventDefault();
      event.stopPropagation();
      setError(null);

      if (event.key === "Escape") {
        stop(true);
        setDraft(value);
        return;
      }

      const mods: string[] = [];
      if (event.ctrlKey) mods.push("Ctrl");
      if (event.altKey) mods.push("Alt");
      if (event.shiftKey) mods.push("Shift");
      if (event.metaKey) mods.push("Win");

      const key = keyNameFromCode(event.code);
      if (!key) return; // só modificador; continua ouvindo

      if (mods.length === 0 && !key.startsWith("F")) {
        setError(t("Use at least one modifier (Ctrl, Alt, Shift, or Win)."));
        return;
      }

      void finalize([...mods, key].join("+"));
    };

    const onBlur = () => {
      stop(true);
      setDraft(value);
    };

    window.addEventListener("keydown", onKey, true);
    window.addEventListener("blur", onBlur);
    return () => {
      window.removeEventListener("keydown", onKey, true);
      window.removeEventListener("blur", onBlur);
    };
  }, [listening, value, finalize, stop, t]);

  useEffect(
    () => () => {
      if (listening) {
        delete document.body.dataset.recordingHotkey;
        void api.resumeHotkeys().catch(() => {});
      }
    },
    [listening],
  );

  return (
    <div className="hotkey-field">
      <button
        type="button"
        className={`hotkey-input ${listening ? "listening" : ""} ${
          error ? "conflict" : ""
        }`}
        onClick={() => void (listening ? stop(true) : start())}
        disabled={disabled}
        aria-label={
          label
            ? `${label}: ${draft || placeholder || t("Click to record")}`
            : undefined
        }
        title={error ?? t("Click and press the shortcut")}
      >
        {listening
          ? t("Press the shortcut… (Esc cancels)")
          : draft || placeholder || t("Click to record")}
      </button>
      {error && <span className="hotkey-error">{error}</span>}
    </div>
  );
}

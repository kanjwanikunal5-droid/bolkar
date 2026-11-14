import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import { api } from "./api";
import { Icon } from "./icons";

/* ---------- Theme ---------- */

export type ThemeChoice = "system" | "light" | "dark";

const THEME_KEY = "theme-cache";

/** Applies the theme to <html data-theme> and caches it so the next start has no flash. */
export function applyTheme(choice: ThemeChoice) {
  const dark = choice === "dark" || (choice === "system" && window.matchMedia("(prefers-color-scheme: dark)").matches);
  document.documentElement.dataset.theme = dark ? "dark" : "light";
  try {
    localStorage.setItem(THEME_KEY, choice);
  } catch {
    /* storage unavailable: the setting in the app database still applies */
  }
}

/** Follows Windows' light/dark switch while the choice is "system". */
export function useTheme(choice: ThemeChoice) {
  useEffect(() => {
    applyTheme(choice);
    if (choice !== "system") return;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const on = () => applyTheme("system");
    mq.addEventListener("change", on);
    return () => mq.removeEventListener("change", on);
  }, [choice]);
}

/* ---------- Toasts ---------- */

interface ToastItem {
  id: number;
  text: string;
  error: boolean;
}

const ToastCtx = createContext<(text: string, error?: boolean) => void>(() => {});

export function ToastProvider({ children }: { children: ReactNode }) {
  const [items, setItems] = useState<ToastItem[]>([]);
  const next = useRef(1);
  const show = useCallback((text: string, error = false) => {
    const id = next.current++;
    setItems((all) => [...all.slice(-2), { id, text, error }]);
    setTimeout(() => setItems((all) => all.filter((t) => t.id !== id)), error ? 5000 : 2200);
  }, []);
  return (
    <ToastCtx.Provider value={show}>
      {children}
      <div className="toasts" role="status" aria-live="polite">
        {items.map((t) => (
          <div key={t.id} className={`toast ${t.error ? "error" : ""}`}>
            {t.error ? <Icon.Info size={16} /> : <Icon.Check size={16} />}
            {t.text}
          </div>
        ))}
      </div>
    </ToastCtx.Provider>
  );
}

export const useToast = () => useContext(ToastCtx);

/* ---------- Controls ---------- */

export function Switch({
  checked,
  onChange,
  label,
  disabled,
}: {
  checked: boolean;
  onChange: (on: boolean) => void;
  label: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      className="switch"
      aria-checked={checked}
      aria-label={label}
      disabled={disabled}
      onClick={() => onChange(!checked)}
    />
  );
}

export function Segmented<T extends string>({
  value,
  options,
  onChange,
  label,
}: {
  value: T;
  options: [T, string][];
  onChange: (v: T) => void;
  label: string;
}) {
  return (
    <div className="segmented" role="group" aria-label={label}>
      {options.map(([v, text]) => (
        <button key={v} type="button" aria-pressed={value === v} onClick={() => onChange(v)}>
          {text}
        </button>
      ))}
    </div>
  );
}

export function SettingRow({ title, help, children }: { title: string; help?: ReactNode; children: ReactNode }) {
  return (
    <div className="setting">
      <div className="what">
        <strong>{title}</strong>
        {help && <span>{help}</span>}
      </div>
      <div className="row">{children}</div>
    </div>
  );
}

export function PageHead({ title, sub, children }: { title: string; sub?: ReactNode; children?: ReactNode }) {
  return (
    <div className="page-head">
      <div>
        <h1>{title}</h1>
        {sub && <p>{sub}</p>}
      </div>
      {children && <div className="row">{children}</div>}
    </div>
  );
}

/** "Ctrl + Win" -> keycaps. Words that are not keys ("Hold", "Double-tap") stay as text. */
export function Keys({ combo }: { combo: string }) {
  const KEY = /^(Ctrl|Win|Alt|Shift|Esc|Enter|Tab|[A-Z0-9])$/;
  const parts = combo.split(/\s*\+\s*/);
  return (
    <span className="keys">
      {parts.map((p, i) => {
        const words = p.split(" ");
        const key = words[words.length - 1];
        const lead = words.slice(0, -1).join(" ");
        return (
          <span key={i} className="keys">
            {i > 0 && <span className="plus">+</span>}
            {lead && <span className="dim">{lead}</span>}
            {KEY.test(key) ? <kbd>{key}</kbd> : <span>{key}</span>}
          </span>
        );
      })}
    </span>
  );
}

export function ProgressBar({ value, max }: { value: number; max: number }) {
  const pct = max > 0 ? Math.min(100, (value / max) * 100) : 0;
  return (
    <div className="progress-bar" role="progressbar" aria-valuemin={0} aria-valuemax={max} aria-valuenow={value}>
      <span style={{ width: `${pct}%` }} />
    </div>
  );
}

/** Animates a number from 0 to its value once (skipped when the user prefers less motion). */
export function CountUp({ value, decimals = 0 }: { value: number; decimals?: number }) {
  const [shown, setShown] = useState(value);
  useEffect(() => {
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches || value === 0) {
      setShown(value);
      return;
    }
    let frame = 0;
    const start = performance.now();
    const step = (t: number) => {
      const k = Math.min(1, (t - start) / 700);
      setShown(value * (1 - Math.pow(1 - k, 3)));
      if (k < 1) frame = requestAnimationFrame(step);
    };
    frame = requestAnimationFrame(step);
    return () => cancelAnimationFrame(frame);
  }, [value]);
  return <>{shown.toLocaleString(undefined, { maximumFractionDigits: decimals, minimumFractionDigits: decimals })}</>;
}

/** Live microphone bars. The microphone only opens when dictation keys are pressed. */
export function MicLevel({ bars = 14 }: { bars?: number }) {
  const [mic, setMic] = useState<{ open: boolean; level: number } | null>(null);
  useEffect(() => {
    let alive = true;
    const tick = () => void api.micStatus().then((m) => alive && setMic(m), () => {});
    tick();
    const t = setInterval(tick, 120);
    return () => {
      alive = false;
      clearInterval(t);
    };
  }, []);
  const level = mic ? Math.min(1, Math.sqrt(mic.level) * 2.2) : 0;
  return (
    <span className="row" style={{ gap: "0.6rem" }}>
      <span className="level" aria-hidden="true">
        {Array.from({ length: bars }, (_, i) => {
          const shape = 0.35 + 0.65 * Math.sin(((i + 1) / (bars + 1)) * Math.PI);
          const h = 12 + Math.round(88 * level * shape);
          return <span key={i} style={{ height: `${h}%`, opacity: mic?.open ? 1 : 0.35 }} />;
        })}
      </span>
      <span className="dim" style={{ fontSize: "0.72rem" }}>
        {!mic ? "" : mic.open ? (level > 0.06 ? "Hearing you" : "Microphone ready") : "Mic opens when you press the keys"}
      </span>
    </span>
  );
}

export function EmptyState({ icon, title, children }: { icon: ReactNode; title: string; children?: ReactNode }) {
  return (
    <div className="empty-state">
      {icon}
      <h3 style={{ marginTop: "0.6rem" }}>{title}</h3>
      {children && <p style={{ margin: "0.3rem auto 0", maxWidth: "28rem" }}>{children}</p>}
    </div>
  );
}

/* ---------- Formatting ---------- */

export function greeting(now = new Date()) {
  const h = now.getHours();
  return h < 5 ? "Working late" : h < 12 ? "Good morning" : h < 17 ? "Good afternoon" : "Good evening";
}

export function timeOf(iso: string) {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? "" : d.toLocaleTimeString([], { hour: "numeric", minute: "2-digit" });
}

export function dayLabel(iso: string) {
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return "";
  const today = new Date();
  const y = new Date();
  y.setDate(today.getDate() - 1);
  if (d.toDateString() === today.toDateString()) return "Today";
  if (d.toDateString() === y.toDateString()) return "Yesterday";
  return d.toLocaleDateString([], { weekday: "long", day: "numeric", month: "short" });
}

export function appName(process: string) {
  const base = process.replace(/\.exe$/i, "");
  const known: Record<string, string> = {
    winword: "Word",
    excel: "Excel",
    powerpnt: "PowerPoint",
    outlook: "Outlook",
    chrome: "Chrome",
    msedge: "Edge",
    firefox: "Firefox",
    whatsapp: "WhatsApp",
    slack: "Slack",
    "ms-teams": "Teams",
    code: "VS Code",
    notepad: "Notepad",
    windowsterminal: "Terminal",
    "app-desktop": "This app",
  };
  return known[base.toLowerCase()] ?? (base ? base[0].toUpperCase() + base.slice(1) : "");
}

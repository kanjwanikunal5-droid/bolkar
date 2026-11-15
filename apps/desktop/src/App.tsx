import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";
import { api, type AiStatus, type Dictation, type Insights, type Model, type ModelProgress, type Settings, type Status, type UpdateInfo } from "./api";
import { APP_NAME } from "./brand";
import { GuidePage } from "./Guide";
import { HistoryPage } from "./History";
import { HomePage } from "./Home";
import { Icon } from "./icons";
import { InsightsPage } from "./Insights";
import { MeetingsPage } from "./Meetings";
import { DictionaryPage, SnippetsPage } from "./Pages";
import { ScratchpadPage } from "./Scratchpad";
import { SettingsPage } from "./SettingsPage";
import { StylePage } from "./Style";
import { ToastProvider, applyTheme, useTheme, useToast, type ThemeChoice } from "./ui";

type Page = "home" | "insights" | "history" | "scratchpad" | "meetings" | "dictionary" | "snippets" | "style" | "guide" | "settings";

const NAV: { group: string; items: [Page, string, JSX.Element][] }[] = [
  {
    group: "",
    items: [
      ["home", "Home", <Icon.Home />],
      ["insights", "Insights", <Icon.Chart />],
      ["history", "History", <Icon.Clock />],
    ],
  },
  {
    group: "Write",
    items: [
      ["scratchpad", "Scratchpad", <Icon.Note />],
      ["meetings", "Meeting notes", <Icon.Meeting />],
    ],
  },
  {
    group: "Personalise",
    items: [
      ["dictionary", "Dictionary", <Icon.Book />],
      ["snippets", "Snippets", <Icon.Quote />],
      ["style", "Style", <Icon.Pen />],
    ],
  },
];

function cachedTheme(): ThemeChoice {
  try {
    return (localStorage.getItem("theme-cache") as ThemeChoice) || "system";
  } catch {
    return "system";
  }
}

/** First run: one button that downloads the recommended speech model and selects it. */
function FirstRun({ gpu, onReady, openSettings }: { gpu: boolean; onReady: () => void; openSettings: () => void }) {
  const id = gpu ? "large-v3-turbo" : "small";
  const [model, setModel] = useState<Model | null>(null);
  const [progress, setProgress] = useState<ModelProgress | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void api.models().then((all) => setModel(all.find((m) => m.id === id) ?? null));
    const off = listen<ModelProgress>("model-progress", (e) => {
      if (e.payload.id !== id) return;
      if (e.payload.finished) {
        setProgress(null);
        if (e.payload.error) setError(e.payload.error);
        else void api.setSetting("model", id).then(onReady);
      } else setProgress(e.payload);
    });
    return () => void off.then((f) => f());
  }, [id, onReady]);

  const percent = progress?.total ? Math.round((progress.done / progress.total) * 100) : 0;
  return (
    <div className="card first-run" role="region" aria-label="Set up">
      <div className="row" style={{ alignItems: "flex-start", gap: "1rem" }}>
        <span className="stat-icon pulse" style={{ marginBottom: 0 }}>
          <Icon.Download size={18} />
        </span>
        <div style={{ flex: 1 }}>
          <h2 style={{ margin: 0 }}>One step before you can dictate</h2>
          <p className="dim" style={{ margin: "0.3rem 0 0.8rem" }}>
            {APP_NAME} recognises speech on this computer, so it needs a speech model once
            {model ? ` (${model.label}, ${model.size_mb} MB)` : ""}. After that it also works offline.
          </p>
          {error && (
            <p className="bad" role="alert">
              Download failed: {error}. Check the internet connection and try again.
            </p>
          )}
          {progress ? (
            <div className="row">
              <progress value={progress.done} max={progress.total || 1} aria-label="Downloading the speech model" />
              <span aria-live="polite">{percent}%</span>
            </div>
          ) : (
            <div className="row">
              <button
                className="btn primary"
                onClick={() => {
                  setError(null);
                  void api.downloadModel(id).catch((e) => setError(String(e)));
                }}
              >
                <Icon.Download size={16} /> Download and set up ({model?.size_mb ?? "..."} MB)
              </button>
              <button className="btn ghost" onClick={openSettings}>
                Choose another model
              </button>
            </div>
          )}
        </div>
      </div>
    </div>
  );
}

function Shell() {
  const toast = useToast();
  const [page, setPage] = useState<Page>("home");
  const [section, setSection] = useState("general");
  const [status, setStatus] = useState<Status | null>(null);
  const [settings, setSettings] = useState<Settings>({});
  const [dictations, setDictations] = useState<Dictation[]>([]);
  const [insights, setInsights] = useState<Insights | null>(null);
  const [ai, setAi] = useState<AiStatus | null>(null);
  const [fixId, setFixId] = useState<string | null>(null);
  const [scratchFocus, setScratchFocus] = useState(0);
  const [update, setUpdate] = useState<UpdateInfo | null>(null);

  const refreshAi = useCallback(() => {
    void api.ai().then(setAi);
    void api.status().then(setStatus);
  }, []);
  const refresh = useCallback(() => {
    void api.status().then(setStatus);
    void api.settings().then(setSettings);
    void api.dictations().then(setDictations);
    void api.insights().then(setInsights, () => {});
    void api.ai().then(setAi);
  }, []);

  const theme = ((settings.theme as ThemeChoice) || cachedTheme()) as ThemeChoice;
  useTheme(theme);

  useEffect(() => {
    refresh();
    const offs = [
      listen("dictation-added", refresh),
      listen("fix-last", () => {
        void api.lastDictationId().then((id) => {
          refresh();
          setPage("history");
          setFixId(id);
        });
      }),
      listen("open-scratchpad", () => {
        setPage("scratchpad");
        setScratchFocus((n) => n + 1);
      }),
      listen<UpdateInfo>("update-available", (e) => setUpdate(e.payload)),
    ];
    return () => offs.forEach((o) => void o.then((f) => f()));
  }, [refresh]);

  // Text size: 100% = 16px. Default 125% because the owner sits far from the screen.
  useEffect(() => {
    const scale = Number(settings.ui_scale ?? "125");
    document.documentElement.style.fontSize = `${(16 * (Number.isFinite(scale) ? scale : 125)) / 100}px`;
  }, [settings.ui_scale]);

  useEffect(() => {
    document.title = APP_NAME;
  }, []);

  /** Saves one setting immediately and confirms with a short notice. */
  const set = useCallback(
    (key: string, value: string, note = "Saved") => {
      setSettings((s) => ({ ...s, [key]: value }));
      api.setSetting(key, value).then(
        () => {
          toast(note);
          void api.status().then(setStatus);
        },
        (e) => toast(String(e), true),
      );
    },
    [toast],
  );

  const go = useCallback((p: string, s?: string) => {
    setPage(p as Page);
    if (s) setSection(s);
  }, []);

  return (
    <div className="app">
      <aside className="sidebar" aria-label="Main">
        <div className="brand">
          <span className="brand-mark">
            <Icon.Logo size={18} />
          </span>
          <span className="brand-name">{APP_NAME}</span>
        </div>
        {NAV.map((g) => (
          <div key={g.group || "main"}>
            {g.group && <div className="nav-label">{g.group}</div>}
            {g.items.map(([id, label, icon]) => (
              <button key={id} className="nav-item" aria-current={page === id ? "page" : undefined} onClick={() => setPage(id)}>
                {icon}
                {label}
              </button>
            ))}
          </div>
        ))}
        <div className="nav-label">Help</div>
        <button className="nav-item" aria-current={page === "guide" ? "page" : undefined} onClick={() => setPage("guide")}>
          <Icon.Compass />
          How to use
        </button>
        <button className="nav-item" aria-current={page === "settings" ? "page" : undefined} onClick={() => setPage("settings")}>
          <Icon.Gear />
          Settings
        </button>

        <div className="sidebar-foot">
          <button
            className="status-chip"
            style={{ border: 0, cursor: "pointer", textAlign: "left" }}
            onClick={() => go("settings", status?.model_installed ? "groq" : "speech")}
            title="Speech recognition"
          >
            <span className={`dot ${status?.hook_error || !status?.model_installed || (status?.gpu_present && !status?.gpu_on) ? "warn" : "on"}`} />
            <span>
              <strong style={{ color: "var(--text)", display: "block" }}>
                {status?.hook_error
                  ? "Shortcuts off"
                  : !status?.model_installed
                    ? "Setup needed"
                    : status?.gpu_present && !status?.gpu_on
                      ? "Slow: graphics card off"
                      : "Ready to dictate"}
              </strong>
              {status?.cloud_speech ? "Groq Whisper" : "On this computer"}
              {ai?.active ? " · AI clean-up" : ""}
            </span>
          </button>
          <div className="theme-switch" role="group" aria-label="Appearance">
            {(
              [
                ["light", <Icon.Sun size={16} />, "Light"],
                ["system", <Icon.Monitor size={16} />, "Follow Windows"],
                ["dark", <Icon.Moon size={16} />, "Dark"],
              ] as [ThemeChoice, JSX.Element, string][]
            ).map(([v, icon, label]) => (
              <button
                key={v}
                aria-pressed={theme === v}
                aria-label={label}
                title={label}
                onClick={() => {
                  applyTheme(v);
                  set("theme", v, `${label} theme`);
                }}
              >
                {icon}
              </button>
            ))}
          </div>
        </div>
      </aside>

      <main>
        <div className="page" style={{ animation: "none" }}>
          {status?.hook_error && (
            <div className="banner" role="alert">
              Shortcuts are not working: {status.hook_error}
            </div>
          )}
          {update?.newer && (
            <div className="banner info" role="status">
              <Icon.Download size={18} /> {update.message}
              <button className="btn small primary" onClick={() => void api.openReleasePage(update.url)}>
                Get it
              </button>
            </div>
          )}
          {status && !status.model_installed && page !== "settings" && (
            <FirstRun gpu={status.gpu_build} onReady={refresh} openSettings={() => go("settings", "speech")} />
          )}
        </div>

        {page === "home" && <HomePage status={status} settings={settings} dictations={dictations} insights={insights} ai={ai} go={go} />}
        {page === "insights" && <InsightsPage data={insights} />}
        {page === "history" && (
          <HistoryPage
            dictations={dictations}
            fixId={fixId}
            onChanged={() => {
              setFixId(null);
              refresh();
            }}
          />
        )}
        {page === "scratchpad" && settings.scratchpad !== undefined && (
          <div className="page">
            <ScratchpadPage initial={settings.scratchpad} focusSignal={scratchFocus} />
          </div>
        )}
        {page === "meetings" && (
          <div className="page">
            <MeetingsPage />
          </div>
        )}
        {page === "dictionary" && <DictionaryPage />}
        {page === "snippets" && <SnippetsPage />}
        {page === "style" && <StylePage settings={settings} ai={ai} set={set} go={go} />}
        {page === "guide" && <GuidePage />}
        {page === "settings" && (
          <SettingsPage settings={settings} status={status} ai={ai} section={section} setSection={setSection} set={set} refreshAi={refreshAi} />
        )}
      </main>
    </div>
  );
}

export function App() {
  return (
    <ToastProvider>
      <Shell />
    </ToastProvider>
  );
}

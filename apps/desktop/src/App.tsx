import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useState } from "react";
import { api, type Dictation, type Model, type ModelProgress, type Settings, type Status, type UpdateInfo } from "./api";
import { APP_NAME } from "./brand";
import { MeetingsPage } from "./Meetings";
import { AiSettings, DictionaryPage, FixBox, MicMeter, SnippetsPage, UploadsCard } from "./Pages";
import { ScratchpadPage } from "./Scratchpad";

type Page = "home" | "history" | "scratchpad" | "meetings" | "dictionary" | "snippets" | "settings";

const LANGUAGES: [string, string][] = [
  ["en-hinglish", "English + Hinglish (automatic, Hinglish in Roman letters)"],
  ["en", "English only"],
  ["hinglish", "Hinglish only (Roman letters)"],
  ["auto", "Any language (detect automatically)"],
  ["hi", "Hindi"],
  ["mr", "Marathi"],
  ["ta", "Tamil"],
  ["gu", "Gujarati"],
  ["bn", "Bengali"],
  ["te", "Telugu"],
  ["kn", "Kannada"],
  ["ml", "Malayalam"],
  ["pa", "Punjabi"],
  ["ur", "Urdu"],
  ["es", "Spanish"],
  ["ar", "Arabic"],
];

function when(iso: string) {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleString();
}

function DictationRow({ d, onChanged, startFixing = false }: { d: Dictation; onChanged: () => void; startFixing?: boolean }) {
  const [showRaw, setShowRaw] = useState(false);
  const [copied, setCopied] = useState(false);
  const [fixing, setFixing] = useState(startFixing);
  useEffect(() => setFixing(startFixing), [startFixing]);
  if (fixing) {
    return (
      <div className="dictation">
        <p className="dim">Correct the text, then save. Your original words stay in history.</p>
        <FixBox
          id={d.id}
          text={d.final_text}
          onDone={() => {
            setFixing(false);
            onChanged();
          }}
        />
      </div>
    );
  }
  return (
    <div className="dictation">
      <div className="row spread">
        <span className="dim">
          {when(d.created_at)} · {d.app_process || "no app"} · {d.word_count} words
        </span>
        <span className="row">
          <span className={`tag ${d.inserted ? "ok" : ""}`}>{d.inserted ? "inserted" : d.insert_method || "not inserted"}</span>
          <span className="tag">{d.engine}</span>
          {d.cleanup && <span className="tag" title={d.cleanup}>{d.cleanup.startsWith("rules") ? (d.cleanup === "rules" ? "rules" : "rules (AI skipped)") : `AI: ${d.cleanup}`}</span>}
        </span>
      </div>
      <p>{d.final_text}</p>
      {showRaw && (
        <p className="raw">
          <span className="dim">What the recogniser heard: </span>
          {d.raw_text}
        </p>
      )}
      {d.error && <p className="bad">{d.error}</p>}
      <div className="row">
        <button
          className="btn"
          onClick={() =>
            void api.copy(d.final_text).then(() => {
              setCopied(true);
              setTimeout(() => setCopied(false), 1500);
            })
          }
        >
          {copied ? "Copied" : "Copy"}
        </button>
        <button className="btn" onClick={() => setFixing(true)}>
          Fix
        </button>
        <button className="btn" onClick={() => setShowRaw((v) => !v)} aria-expanded={showRaw}>
          {showRaw ? "Hide original" : "Show original"}
        </button>
        <button className="btn" onClick={() => void api.deleteDictation(d.id).then(onChanged)}>
          Delete
        </button>
      </div>
    </div>
  );
}

/** Settings card: version, manual update check, opt-in check at start-up. */
function UpdatesCard({ version, auto, setAuto }: { version: string; auto: boolean; setAuto: (on: boolean) => void }) {
  const [info, setInfo] = useState<UpdateInfo | null>(null);
  const [busy, setBusy] = useState(false);
  return (
    <div className="card">
      <h3>Updates</h3>
      <p>
        You have version <strong>{version}</strong>.
      </p>
      <div className="row">
        <button
          className="btn"
          disabled={busy}
          onClick={() => {
            setBusy(true);
            void api.checkUpdate().then((i) => {
              setInfo(i);
              setBusy(false);
            });
          }}
        >
          {busy ? "Checking..." : "Check for updates"}
        </button>
        {info?.newer && (
          <button className="btn primary" onClick={() => void api.openReleasePage(info.url)}>
            Open the download page
          </button>
        )}
      </div>
      {info && <p aria-live="polite">{info.message}</p>}
      <label className="row">
        <input type="checkbox" checked={auto} onChange={(e) => setAuto(e.target.checked)} />
        Check automatically when the app starts (asks GitHub; nothing is installed without you)
      </label>
    </div>
  );
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
      } else {
        setProgress(e.payload);
      }
    });
    return () => {
      void off.then((f) => f());
    };
  }, [id, onReady]);

  const percent = progress?.total ? Math.round((progress.done / progress.total) * 100) : 0;
  return (
    <div className="card first-run" role="region" aria-label="Set up">
      <h3>One step before you can dictate</h3>
      <p>
        {APP_NAME} recognises speech on this computer, so it needs a speech model once
        {model ? ` (${model.label}, ${model.size_mb} MB)` : ""}. After that it works offline.
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
            Download and set up ({model?.size_mb ?? "..."} MB)
          </button>
          <button className="btn" onClick={openSettings}>
            Choose another model
          </button>
        </div>
      )}
    </div>
  );
}

function ModelList({ selected, onSelect }: { selected: string; onSelect: (id: string) => void }) {
  const [models, setModels] = useState<Model[]>([]);
  const [progress, setProgress] = useState<Record<string, ModelProgress>>({});
  const [error, setError] = useState<string | null>(null);
  const refresh = useCallback(() => void api.models().then(setModels), []);

  useEffect(() => {
    refresh();
    const off = listen<ModelProgress>("model-progress", (e) => {
      const p = e.payload;
      if (p.finished) {
        setProgress((all) => {
          const next = { ...all };
          delete next[p.id];
          return next;
        });
        setError(p.error);
        refresh();
      } else {
        setProgress((all) => ({ ...all, [p.id]: p }));
      }
    });
    return () => {
      void off.then((f) => f());
    };
  }, [refresh]);

  return (
    <div>
      {error && (
        <div className="banner" role="alert">
          {error}
        </div>
      )}
      {models.map((m) => {
        const p = progress[m.id];
        return (
          <div className="dictation" key={m.id}>
            <div className="row spread">
              <label className="row">
                <input
                  type="radio"
                  name="model"
                  checked={selected === m.id}
                  disabled={!m.installed}
                  onChange={() => onSelect(m.id)}
                />
                <strong>{m.label}</strong>
              </label>
              <span className="row">
                {p ? (
                  <>
                    <progress value={p.done} max={p.total || 1} aria-label={`Downloading ${m.label}`} />
                    <span className="dim">{p.total ? Math.round((p.done / p.total) * 100) : 0}%</span>
                  </>
                ) : m.installed ? (
                  <button className="btn" disabled={selected === m.id} onClick={() => void api.deleteModel(m.id).then(refresh, (e) => setError(String(e)))}>
                    Delete
                  </button>
                ) : (
                  <button className="btn primary" onClick={() => void api.downloadModel(m.id)}>
                    Download ({m.size_mb} MB)
                  </button>
                )}
              </span>
            </div>
            <div className="dim">
              {m.size_mb} MB on disk · needs about {m.ram_mb} MB of memory{m.note ? ` · ${m.note}` : ""}
            </div>
          </div>
        );
      })}
    </div>
  );
}

export function App() {
  const [page, setPage] = useState<Page>("home");
  const [status, setStatus] = useState<Status | null>(null);
  const [settings, setSettings] = useState<Settings>({});
  const [dictations, setDictations] = useState<Dictation[]>([]);

  const refresh = useCallback(() => {
    void api.status().then(setStatus);
    void api.settings().then(setSettings);
    void api.dictations().then(setDictations);
  }, []);

  const [fixId, setFixId] = useState<string | null>(null);
  const [scratchFocus, setScratchFocus] = useState(0);
  const [update, setUpdate] = useState<UpdateInfo | null>(null);

  useEffect(() => {
    refresh();
    const off = listen("dictation-added", refresh);
    // Alt + Shift + F: open the last dictation for correcting.
    const offFix = listen("fix-last", () => {
      void api.lastDictationId().then((id) => {
        refresh();
        setPage("history");
        setFixId(id);
      });
    });
    // Alt + Shift + N: open the scratchpad, cursor at the end.
    const offScratch = listen("open-scratchpad", () => {
      setPage("scratchpad");
      setScratchFocus((n) => n + 1);
    });
    const offUpdate = listen<UpdateInfo>("update-available", (e) => setUpdate(e.payload));
    return () => {
      void offUpdate.then((f) => f());
      void off.then((f) => f());
      void offFix.then((f) => f());
      void offScratch.then((f) => f());
    };
  }, [refresh]);

  // Text size: 100% = 16px. Default is 125% because the owner sits far from the screen.
  useEffect(() => {
    const scale = Number(settings.ui_scale ?? "125");
    document.documentElement.style.fontSize = `${(16 * (Number.isFinite(scale) ? scale : 125)) / 100}px`;
  }, [settings.ui_scale]);

  useEffect(() => {
    document.title = APP_NAME;
  }, []);

  const set = (key: string, value: string) => {
    setSettings((s) => ({ ...s, [key]: value }));
    void api.setSetting(key, value).then(refresh);
  };

  const pages: [Page, string][] = [
    ["home", "Home"],
    ["history", "Dictation history"],
    ["scratchpad", "Scratchpad"],
    ["meetings", "Meeting notes"],
    ["dictionary", "Dictionary"],
    ["snippets", "Snippets"],
    ["settings", "Settings"],
  ];

  return (
    <div className="app">
      <nav aria-label="Main">
        <h1>{APP_NAME}</h1>
        {pages.map(([id, label]) => (
          <button key={id} aria-current={page === id ? "page" : undefined} onClick={() => setPage(id)}>
            {label}
          </button>
        ))}
      </nav>
      <main>
        {status?.hook_error && (
          <div className="banner" role="alert">
            Shortcuts are not working: {status.hook_error}
          </div>
        )}
        {update?.newer && (
          <div className="banner" role="status">
            {update.message}{" "}
            <button className="btn" onClick={() => void api.openReleasePage(update.url)}>
              Open the download page
            </button>
          </div>
        )}
        {status && !status.model_installed && page !== "settings" && (
          <FirstRun gpu={status.gpu_build} onReady={refresh} openSettings={() => setPage("settings")} />
        )}

        {page === "home" && (
          <>
            <h2>Home</h2>
            <div className="card">
              <h3>How to dictate</h3>
              <table className="keys">
                <tbody>
                  {status?.shortcuts.map(([keys, what]) => (
                    <tr key={keys}>
                      <td>
                        <kbd>{keys}</kbd>
                      </td>
                      <td>{what}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
              <p className="dim">
                Click where you want the text, hold the keys, speak, then let go. The text is typed where your cursor is.
              </p>
            </div>
            <div className="card">
              <h3>Try it here</h3>
              <textarea className="practice" aria-label="Practice box" placeholder="Click here, hold Ctrl + Win and speak." />
            </div>
            <div className="card row spread">
              <span>
                <strong>{status?.words_this_week ?? 0}</strong> words dictated in the last 7 days
              </span>
              <span className="dim">
                {status?.cloud_speech
                  ? "Speech recognition: Groq (cloud, your key) - audio is sent to Groq"
                  : `Speech recognition: on this device${status?.gpu_build && settings.use_gpu === "true" ? " (graphics card)" : ""} - audio never leaves this computer`}
              </span>
            </div>
            <div className="card">
              <h3>Recent</h3>
              {dictations.length === 0 && <p className="dim">Nothing dictated yet.</p>}
              {dictations.slice(0, 3).map((d) => (
                <DictationRow key={d.id} d={d} onChanged={refresh} />
              ))}
            </div>
          </>
        )}

        {page === "history" && (
          <>
            <h2>Dictation history</h2>
            <p className="dim">
              Every dictation is kept here, even if it could not be inserted. The original words from the recogniser are stored
              separately from the cleaned text.
            </p>
            <div className="card">
              {dictations.length === 0 && <p className="dim">Nothing dictated yet.</p>}
              {dictations.map((d) => (
                <DictationRow
                  key={d.id}
                  d={d}
                  startFixing={d.id === fixId}
                  onChanged={() => {
                    setFixId(null);
                    refresh();
                  }}
                />
              ))}
            </div>
          </>
        )}

        {page === "scratchpad" && settings.scratchpad !== undefined && (
          <ScratchpadPage initial={settings.scratchpad} focusSignal={scratchFocus} />
        )}
        {page === "meetings" && <MeetingsPage />}
        {page === "dictionary" && <DictionaryPage />}
        {page === "snippets" && <SnippetsPage />}

        {page === "settings" && (
          <>
            <h2>Settings</h2>
            <div className="card">
              <h3>Speech model</h3>
              <ModelList selected={settings.model ?? ""} onSelect={(id) => set("model", id)} />
              {status?.gpu_build && (
                <label className="row" style={{ marginTop: "0.75rem" }}>
                  <input type="checkbox" checked={settings.use_gpu === "true"} onChange={(e) => set("use_gpu", String(e.target.checked))} />
                  Use the graphics card (much faster)
                </label>
              )}
            </div>
            <UpdatesCard
              version={status?.version ?? ""}
              auto={settings.check_updates === "on"}
              setAuto={(on) => set("check_updates", on ? "on" : "off")}
            />
            <div className="card">
              <h3>Dictation</h3>
              <MicMeter />
              <div className="field">
                <label htmlFor="language">Language</label>
                <select id="language" value={settings.language ?? "auto"} onChange={(e) => set("language", e.target.value)}>
                  {LANGUAGES.map(([code, name]) => (
                    <option key={code} value={code}>
                      {name}
                    </option>
                  ))}
                </select>
              </div>
              <div className="field">
                <label htmlFor="level">Clean-up</label>
                <select id="level" value={settings.level ?? "LIGHT"} onChange={(e) => set("level", e.target.value)}>
                  <option value="LITERAL">Literal - keep every word</option>
                  <option value="LIGHT">Light - remove "um", fix self-corrections, format lists</option>
                  <option value="PROFESSIONAL">Professional - Light plus grammar polish (needs AI clean-up)</option>
                </select>
              </div>
              <label className="row">
                <input
                  type="checkbox"
                  checked={settings.spoken_commands === "true"}
                  onChange={(e) => set("spoken_commands", String(e.target.checked))}
                />
                Spoken commands ("new line", "full stop", "bullet point")
              </label>
              <label className="row">
                <input
                  type="checkbox"
                  checked={settings.keep_mic_open === "true"}
                  onChange={(e) => set("keep_mic_open", String(e.target.checked))}
                />
                Keep the microphone ready, so your first word is not cut off. Only the last 0.3 seconds are held in memory
                (never saved or sent). Windows shows the microphone as in use while this is on. If you turn it off, the
                microphone takes up to about 1-2 seconds to start after you press the keys.
              </label>
            </div>
            <AiSettings />
            <UploadsCard />
            <div className="card">
              <h3>Privacy</h3>
              <label className="row">
                <input type="checkbox" checked={settings.save_audio === "true"} onChange={(e) => set("save_audio", String(e.target.checked))} />
                Keep the recording of each dictation on this computer for {settings.audio_retention_days ?? "7"} days
              </label>
              <p className="dim">
                Audio never leaves this computer. Text is sent only if you turn on AI clean-up above, and every such upload is
                listed under "Data that left this computer". Dictations are kept out of Windows clipboard history.
              </p>
            </div>
            <div className="card">
              <h3>Text size</h3>
              <div className="field">
                <label htmlFor="scale">Size</label>
                <select id="scale" value={settings.ui_scale ?? "125"} onChange={(e) => set("ui_scale", e.target.value)}>
                  {["100", "125", "150", "175", "200"].map((s) => (
                    <option key={s} value={s}>
                      {s}%
                    </option>
                  ))}
                </select>
              </div>
            </div>
            <p className="dim">
              {APP_NAME} {status?.version}
              {status?.running_as_admin ? " · running as administrator" : ""}
            </p>
          </>
        )}
      </main>
    </div>
  );
}

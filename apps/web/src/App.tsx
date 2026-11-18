import { useEffect, useRef, useState } from "react";
import { startRecording, type Recording } from "./audio";
import { chat, resetModelChoice, SPEECH_MODEL, transcribe } from "./groq";
import { addHistory, clearHistory, forgetKey, keyKind, loadHistory, saveKey, saveKeyOnDevice, unlockKey, unlockOnDevice, type HistoryItem } from "./keystore";
import * as core from "./wasm/app_wasm.js";

type Mode = "en-hinglish" | "en" | "hinglish";
const MODES: [Mode, string][] = [
  ["en-hinglish", "English + Hinglish"],
  ["en", "English"],
  ["hinglish", "Hinglish"],
];
const DATA_NOTE =
  "Your audio and the text of each dictation are sent to Groq (with your own free key) to be recognised and cleaned up. Free accounts have daily limits. Read Groq's privacy policy before agreeing. Your key is stored encrypted in this browser only.";

function vocabulary() {
  return JSON.stringify({ terms: JSON.parse(core.default_terms()) });
}

async function dictate(key: string, wav: Blob, mode: Mode): Promise<{ raw: string; text: string; how: string }> {
  const vocab = vocabulary();
  const language = mode === "en" ? "en" : mode === "hinglish" ? "hi" : null;
  let raw = await transcribe(key, wav, language, mode === "hinglish" ? core.roman_hinglish_example() : core.recogniser_hint(vocab));
  const how: string[] = [SPEECH_MODEL];
  const hadDevanagari = core.has_devanagari(raw);
  if (mode !== "en" && hadDevanagari) {
    try {
      const answer = await chat(key, core.transliterate_prompt(), JSON.stringify({ text: raw }), 4000);
      const roman = core.accept_transliteration(answer, raw);
      if (roman) {
        raw = roman;
        how.push("Roman Hinglish");
      } else how.push("Roman conversion rejected");
    } catch (e) {
      how.push(`Roman conversion failed: ${(e as Error).message}`);
    }
  }
  // Hinglish spelling (bahut, karunga...) and obvious mis-hearings; checked against rewrites.
  let toClean = raw;
  const hinglish = mode === "hinglish" || (mode === "en-hinglish" && hadDevanagari);
  if (hinglish && raw && !core.has_devanagari(raw)) {
    toClean = core.hinglish_spelling(raw);
    try {
      const answer = await chat(key, core.polish_prompt(), JSON.stringify({ text: toClean }), 4000);
      const polished = core.accept_polish(answer, toClean);
      if (polished) {
        toClean = polished;
        how.push("Hinglish spelling");
      } else how.push("spelling polish rejected");
    } catch (e) {
      how.push(`spelling polish failed: ${(e as Error).message}`);
    }
  }
  const langTag = hinglish ? "hi-en" : "en";
  let text = core.clean(toClean, "LIGHT", true, vocab);
  try {
    const req = JSON.parse(core.cleanup_request(toClean, "LIGHT", true, vocab, langTag)) as { system: string; user: string };
    const answer = await chat(key, req.system, req.user, 3000);
    const verdict = JSON.parse(core.accept_cleanup(answer, toClean, "LIGHT", true, vocab)) as { ok: boolean; text: string; why?: string };
    text = verdict.text;
    how.push(verdict.ok ? "AI clean-up" : `rules (AI answer rejected: ${verdict.why})`);
  } catch (e) {
    how.push(`rules (AI unavailable: ${(e as Error).message})`);
  }
  return { raw, text, how: how.join(" + ") };
}

type Theme = "system" | "light" | "dark";

function applyTheme(t: Theme) {
  const dark = t === "dark" || (t === "system" && matchMedia("(prefers-color-scheme: dark)").matches);
  document.documentElement.dataset.theme = dark ? "dark" : "light";
  document.querySelector('meta[name="theme-color"]')?.setAttribute("content", dark ? "#0b0e12" : "#0a7a72");
}

function readTheme(): Theme {
  try {
    return (localStorage.getItem("theme") as Theme) || "system";
  } catch {
    return "system";
  }
}

const Logo = () => (
  <svg width="22" height="22" viewBox="0 0 24 24" aria-hidden="true">
    <g fill="currentColor">
      <rect x="3" y="9" width="2.4" height="6" rx="1.2" />
      <rect x="7.2" y="5.5" width="2.4" height="13" rx="1.2" />
      <rect x="11.4" y="2.5" width="2.4" height="19" rx="1.2" />
      <rect x="15.6" y="6.5" width="2.4" height="11" rx="1.2" />
      <rect x="19.8" y="9.5" width="2.4" height="5" rx="1.2" />
    </g>
  </svg>
);

const MicIcon = () => (
  <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="1.8" strokeLinecap="round" aria-hidden="true">
    <rect x="9" y="3" width="6" height="11" rx="3" />
    <path d="M5.5 11a6.5 6.5 0 0 0 13 0M12 17.5V21" />
  </svg>
);

const StopIcon = () => (
  <svg width="34" height="34" viewBox="0 0 24 24" aria-hidden="true">
    <rect x="6" y="6" width="12" height="12" rx="3" fill="currentColor" />
  </svg>
);

function KeySetup({ onDone }: { onDone: (key: string) => void }) {
  const [key, setKey] = useState("");
  const [usePass, setUsePass] = useState(false);
  const [pass, setPass] = useState("");
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const save = () => {
    setBusy(true);
    const k = key.trim();
    (usePass ? saveKey(k, pass) : saveKeyOnDevice(k)).then(
      () => onDone(k),
      (e) => {
        setError(String(e));
        setBusy(false);
      },
    );
  };
  return (
    <section className="card setup">
      <h2>Connect once, then just speak</h2>
      <p className="note">
        Uses your own free Groq key: create one at <strong>console.groq.com/keys</strong>, copy it, paste it here. You will not be
        asked again on this device.
      </p>
      <input type="password" autoComplete="off" aria-label="Groq API key" placeholder="Paste your Groq key (gsk_...)" value={key} onChange={(e) => setKey(e.target.value)} />
      <label className="check">
        <input type="checkbox" checked={usePass} onChange={(e) => setUsePass(e.target.checked)} />
        Shared device: lock the key with a passphrase
      </label>
      {usePass && (
        <input type="password" autoComplete="new-password" aria-label="Passphrase" placeholder="Passphrase (4+ characters)" value={pass} onChange={(e) => setPass(e.target.value)} />
      )}
      <p className="note">{DATA_NOTE} Pressing Connect means you agree.</p>
      {error && <p className="bad">{error}</p>}
      <button className="primary big" disabled={key.trim().length < 20 || (usePass && pass.length < 4) || busy} onClick={save}>
        {busy ? "Saving..." : "Connect"}
      </button>
    </section>
  );
}

function Unlock({ onDone, onReset }: { onDone: (key: string) => void; onReset: () => void }) {
  const [pass, setPass] = useState("");
  const [error, setError] = useState("");
  const go = () => void unlockKey(pass).then(onDone, (e: Error) => setError(e.message));
  return (
    <section className="card setup">
      <h2>Unlock</h2>
      <input type="password" aria-label="Passphrase" placeholder="Passphrase" value={pass} onChange={(e) => setPass(e.target.value)} onKeyDown={(e) => e.key === "Enter" && go()} />
      {error && <p className="bad">{error}</p>}
      <div className="row">
        <button className="primary" onClick={go}>
          Unlock
        </button>
        <button onClick={() => void forgetKey().then(() => { resetModelChoice(); onReset(); })}>Remove key</button>
      </div>
    </section>
  );
}

function Wave({ level, live }: { level: number; live: boolean }) {
  const n = 15;
  const v = Math.min(1, Math.sqrt(level) * 2.4);
  return (
    <div className={`wave ${live ? "live" : ""}`} aria-hidden="true">
      {Array.from({ length: n }, (_, i) => {
        const shape = 0.3 + 0.7 * Math.sin(((i + 1) / (n + 1)) * Math.PI);
        return <span key={i} style={{ height: `${10 + 90 * v * shape}%` }} />;
      })}
    </div>
  );
}

function clock(s: number) {
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;
}

export function App() {
  const [stage, setStage] = useState<"loading" | "setup" | "locked" | "ready">("loading");
  const [key, setKey] = useState("");
  const [mode, setMode] = useState<Mode>(() => {
    try {
      return (localStorage.getItem("mode") as Mode) || "en-hinglish";
    } catch {
      return "en-hinglish";
    }
  });
  const [theme, setTheme] = useState<Theme>(readTheme);
  const [rec, setRec] = useState<Recording | null>(null);
  const [level, setLevel] = useState(0);
  const [seconds, setSeconds] = useState(0);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<{ raw: string; text: string; how: string } | null>(null);
  const [error, setError] = useState("");
  const [toast, setToast] = useState("");
  const [history, setHistory] = useState<HistoryItem[]>([]);
  const [query, setQuery] = useState("");
  const [menu, setMenu] = useState(false);
  const timer = useRef<number>();
  const started = useRef(0);

  useEffect(() => {
    void keyKind().then((kind) => {
      if (kind === "device") void unlockOnDevice().then(ready, () => setStage("setup"));
      else setStage(kind === "passphrase" ? "locked" : "setup");
    });
    void loadHistory().then(setHistory);
  }, []);
  useEffect(() => {
    try {
      localStorage.setItem("mode", mode);
    } catch {
      /* private mode */
    }
  }, [mode]);
  useEffect(() => {
    applyTheme(theme);
    try {
      localStorage.setItem("theme", theme);
    } catch {
      /* private mode */
    }
    if (theme !== "system") return;
    const mq = matchMedia("(prefers-color-scheme: dark)");
    const on = () => applyTheme("system");
    mq.addEventListener("change", on);
    return () => mq.removeEventListener("change", on);
  }, [theme]);

  function ready(k: string) {
    setKey(k);
    setStage("ready");
  }

  const flash = (t: string) => {
    setToast(t);
    setTimeout(() => setToast(""), 1800);
  };

  const start = async () => {
    setError("");
    setResult(null);
    try {
      const r = await startRecording();
      setRec(r);
      started.current = Date.now();
      setSeconds(0);
      timer.current = window.setInterval(() => {
        setLevel(r.level());
        setSeconds(Math.floor((Date.now() - started.current) / 1000));
      }, 80);
      navigator.vibrate?.(15);
    } catch {
      setError("Microphone not available. Allow microphone access for this site.");
    }
  };
  const stop = async () => {
    if (!rec) return;
    window.clearInterval(timer.current);
    setRec(null);
    setLevel(0);
    setBusy(true);
    navigator.vibrate?.(10);
    try {
      const wav = await rec.stop();
      const r = await dictate(key, wav, mode);
      setResult(r);
      if (r.text) {
        await addHistory({ at: new Date().toISOString(), ...r });
        setHistory(await loadHistory());
        // Copy straight away so it is ready to paste (some browsers need a tap: then use Copy).
        navigator.clipboard.writeText(r.text).then(
          () => flash("Copied, ready to paste"),
          () => {},
        );
      } else setError("Didn't hear anything. Tap and try again.");
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  const cancel = () => {
    window.clearInterval(timer.current);
    rec?.cancel();
    setRec(null);
    setLevel(0);
  };
  const copy = (text: string) => void navigator.clipboard.writeText(text).then(() => flash("Copied"), () => flash("Copy failed"));
  const share = (text: string) => {
    if (navigator.share) void navigator.share({ text }).catch(() => {});
    else window.open(`https://wa.me/?text=${encodeURIComponent(text)}`, "_blank", "noopener");
  };

  const today = new Date().toDateString();
  const words = (t: string) => t.split(/\s+/).filter(Boolean).length;
  const todayWords = history.filter((h) => new Date(h.at).toDateString() === today).reduce((n, h) => n + words(h.text), 0);
  const totalWords = history.reduce((n, h) => n + words(h.text), 0);
  const shown = history.filter((h) => !query.trim() || h.text.toLowerCase().includes(query.trim().toLowerCase()));

  return (
    <div className="shell">
      <header className="top">
        <span className="brand">
          <span className="brand-mark">
            <Logo />
          </span>
          {__APP_NAME__}
        </span>
        <div className="seg" role="group" aria-label="Appearance">
          {(["light", "system", "dark"] as Theme[]).map((t) => (
            <button key={t} aria-pressed={theme === t} onClick={() => setTheme(t)} aria-label={`${t} theme`}>
              {t === "light" ? "☀" : t === "dark" ? "☾" : "A"}
            </button>
          ))}
        </div>
      </header>

      <main>
        {stage === "loading" && <div className="card skeleton" style={{ height: 220 }} />}
        {stage === "setup" && <KeySetup onDone={ready} />}
        {stage === "locked" && <Unlock onDone={ready} onReset={() => setStage("setup")} />}
        {stage === "ready" && (
          <>
            <section className={`hero ${rec ? "recording" : ""}`}>
              <div className="modes" role="radiogroup" aria-label="Language">
                {MODES.map(([m, label]) => (
                  <button key={m} role="radio" aria-checked={mode === m} className={mode === m ? "on" : ""} onClick={() => setMode(m)} disabled={!!rec || busy}>
                    {label}
                  </button>
                ))}
              </div>
              <button
                className={`mic ${rec ? "live" : ""} ${busy ? "busy" : ""}`}
                disabled={busy}
                aria-label={rec ? "Stop and transcribe" : "Start dictating"}
                onClick={() => void (rec ? stop() : start())}
                style={rec ? { boxShadow: `0 0 0 ${10 + Math.min(46, Math.sqrt(level) * 140)}px rgba(255,255,255,0.14)` } : undefined}
              >
                {busy ? <span className="spinner" /> : rec ? <StopIcon /> : <MicIcon />}
              </button>
              <Wave level={level} live={!!rec} />
              <p className="hint" aria-live="polite">
                {busy ? "Writing it down..." : rec ? `Listening ${clock(seconds)} · tap to finish` : "Tap and speak in English or Hinglish"}
              </p>
              {rec && (
                <button className="ghost-light" onClick={cancel}>
                  Cancel
                </button>
              )}
            </section>

            {error && <p className="card bad">{error}</p>}

            {result && result.text && (
              <section className="card result-card">
                <p className="result">{result.text}</p>
                <div className="row">
                  <button className="primary" onClick={() => copy(result.text)}>
                    Copy
                  </button>
                  <button onClick={() => share(result.text)}>Share</button>
                </div>
                <details>
                  <summary className="note">What was heard</summary>
                  <p className="note">{result.raw}</p>
                  <p className="note">{result.how}</p>
                </details>
              </section>
            )}

            <section className="stats">
              <div className="stat">
                <strong>{todayWords.toLocaleString()}</strong>
                <span>words today</span>
              </div>
              <div className="stat">
                <strong>{totalWords.toLocaleString()}</strong>
                <span>words on this phone</span>
              </div>
              <div className="stat">
                <strong>{history.length}</strong>
                <span>dictations</span>
              </div>
            </section>

            {history.length > 0 && (
              <section className="card">
                <div className="row spread">
                  <h2>History</h2>
                  <span className="note">This device only</span>
                </div>
                <input type="text" aria-label="Search history" placeholder="Search" value={query} onChange={(e) => setQuery(e.target.value)} />
                {shown.slice(0, 40).map((h) => (
                  <div key={h.at} className="item">
                    <div>
                      <p>{h.text}</p>
                      <span className="note">{new Date(h.at).toLocaleString([], { day: "numeric", month: "short", hour: "numeric", minute: "2-digit" })}</span>
                    </div>
                    <div className="item-actions">
                      <button onClick={() => copy(h.text)}>Copy</button>
                      <button onClick={() => share(h.text)}>Share</button>
                    </div>
                  </div>
                ))}
              </section>
            )}

            <section className="card">
              <button className="link" onClick={() => setMenu((m) => !m)} aria-expanded={menu}>
                Settings
              </button>
              {menu && (
                <div className="stack">
                  <p className="note">Groq key: saved on this device. Speech is recognised by Groq Whisper with your key.</p>
                  <div className="row">
                    <button onClick={() => void clearHistory().then(() => setHistory([]))}>Clear history</button>
                    <button
                      onClick={() =>
                        void forgetKey().then(() => {
                          resetModelChoice();
                          setKey("");
                          setStage("setup");
                        })
                      }
                    >
                      Remove key
                    </button>
                  </div>
                </div>
              )}
            </section>
          </>
        )}
        <footer className="note">Tip: Share sends the text straight to WhatsApp. On Android, the {__APP_NAME__} keyboard types into any app.</footer>
      </main>
      {toast && (
        <div className="toast" role="status">
          ✓ {toast}
        </div>
      )}
    </div>
  );
}

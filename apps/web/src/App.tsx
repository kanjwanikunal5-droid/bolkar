import { useEffect, useRef, useState } from "react";
import { startRecording, type Recording } from "./audio";
import { chat, SPEECH_MODEL, transcribe } from "./groq";
import { addHistory, clearHistory, forgetKey, hasKey, loadHistory, saveKey, unlockKey, type HistoryItem } from "./keystore";
import * as core from "./wasm/app_wasm.js";

type Mode = "en-hinglish" | "en" | "hinglish";
const MODES: [Mode, string][] = [
  ["en-hinglish", "English + Hinglish"],
  ["en", "English"],
  ["hinglish", "Hinglish"],
];
const DATA_NOTE =
  "Your audio and the text of each dictation are sent to Groq (with your own free key) to be recognised and cleaned up. Free accounts have daily limits. Read Groq's privacy policy before agreeing. Your key is encrypted with your passphrase and stays in this browser.";

function vocabulary() {
  return JSON.stringify({ terms: JSON.parse(core.default_terms()) });
}

async function dictate(key: string, wav: Blob, mode: Mode): Promise<{ raw: string; text: string; how: string }> {
  const vocab = vocabulary();
  const language = mode === "en" ? "en" : mode === "hinglish" ? "hi" : null;
  let raw = await transcribe(key, wav, language, mode === "hinglish" ? "" : core.recogniser_hint(vocab));
  const how: string[] = [SPEECH_MODEL];
  if (mode !== "en" && core.has_devanagari(raw)) {
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
  const langTag = mode === "en" ? "en" : "hi-en";
  let text = core.clean(raw, "LIGHT", true, vocab);
  try {
    const req = JSON.parse(core.cleanup_request(raw, "LIGHT", true, vocab, langTag)) as { system: string; user: string };
    const answer = await chat(key, req.system, req.user, 3000);
    const verdict = JSON.parse(core.accept_cleanup(answer, raw, "LIGHT", true, vocab)) as { ok: boolean; text: string; why?: string };
    text = verdict.text;
    how.push(verdict.ok ? "AI clean-up" : `rules (AI answer rejected: ${verdict.why})`);
  } catch (e) {
    how.push(`rules (AI unavailable: ${(e as Error).message})`);
  }
  return { raw, text, how: how.join(" + ") };
}

function KeySetup({ onDone }: { onDone: (key: string) => void }) {
  const [key, setKey] = useState("");
  const [pass, setPass] = useState("");
  const [agree, setAgree] = useState(false);
  const [error, setError] = useState("");
  return (
    <section className="card">
      <h2>Set up</h2>
      <p>This web app uses your own free Groq key. Create one at console.groq.com/keys.</p>
      <label>
        Groq API key
        <input type="password" autoComplete="off" value={key} onChange={(e) => setKey(e.target.value)} />
      </label>
      <label>
        Passphrase (to lock the key on this device)
        <input type="password" autoComplete="new-password" value={pass} onChange={(e) => setPass(e.target.value)} />
      </label>
      <p className="note">{DATA_NOTE}</p>
      <label className="check">
        <input type="checkbox" checked={agree} onChange={(e) => setAgree(e.target.checked)} />I understand and agree.
      </label>
      {error && <p className="bad">{error}</p>}
      <button
        className="primary"
        disabled={!key.trim() || pass.length < 4 || !agree}
        onClick={() => void saveKey(key.trim(), pass).then(() => onDone(key.trim()), (e) => setError(String(e)))}
      >
        Save
      </button>
    </section>
  );
}

function Unlock({ onDone, onReset }: { onDone: (key: string) => void; onReset: () => void }) {
  const [pass, setPass] = useState("");
  const [error, setError] = useState("");
  const go = () => void unlockKey(pass).then(onDone, (e: Error) => setError(e.message));
  return (
    <section className="card">
      <h2>Unlock</h2>
      <label>
        Passphrase
        <input type="password" value={pass} onChange={(e) => setPass(e.target.value)} onKeyDown={(e) => e.key === "Enter" && go()} />
      </label>
      {error && <p className="bad">{error}</p>}
      <div className="row">
        <button className="primary" onClick={go}>
          Unlock
        </button>
        <button onClick={() => void forgetKey().then(onReset)}>Remove key</button>
      </div>
    </section>
  );
}

export function App() {
  const [stage, setStage] = useState<"loading" | "setup" | "locked" | "ready">("loading");
  const [key, setKey] = useState("");
  const [mode, setMode] = useState<Mode>(() => (localStorage.getItem("mode") as Mode) || "en-hinglish");
  const [rec, setRec] = useState<Recording | null>(null);
  const [level, setLevel] = useState(0);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<{ raw: string; text: string; how: string } | null>(null);
  const [error, setError] = useState("");
  const [copied, setCopied] = useState(false);
  const [history, setHistory] = useState<HistoryItem[]>([]);
  const timer = useRef<number>();

  useEffect(() => {
    void hasKey().then((has) => setStage(has ? "locked" : "setup"));
    void loadHistory().then(setHistory);
  }, []);
  useEffect(() => {
    try {
      localStorage.setItem("mode", mode);
    } catch {
      /* private mode */
    }
  }, [mode]);

  const ready = (k: string) => {
    setKey(k);
    setStage("ready");
  };

  const start = async () => {
    setError("");
    setResult(null);
    try {
      const r = await startRecording();
      setRec(r);
      timer.current = window.setInterval(() => setLevel(r.level()), 80);
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
    try {
      const wav = await rec.stop();
      const r = await dictate(key, wav, mode);
      setResult(r);
      if (r.text) {
        await addHistory({ at: new Date().toISOString(), ...r });
        setHistory(await loadHistory());
      } else setError("Didn't hear anything.");
    } catch (e) {
      setError((e as Error).message);
    } finally {
      setBusy(false);
    }
  };
  const copy = (text: string) =>
    void navigator.clipboard.writeText(text).then(() => {
      setCopied(true);
      setTimeout(() => setCopied(false), 1500);
    });

  return (
    <main>
      <header>
        <h1>Bolkar</h1>
        <span className="note">Speak, then copy the clean text</span>
      </header>
      {stage === "setup" && <KeySetup onDone={ready} />}
      {stage === "locked" && <Unlock onDone={ready} onReset={() => setStage("setup")} />}
      {stage === "ready" && (
        <>
          <div className="modes" role="radiogroup" aria-label="Language">
            {MODES.map(([m, label]) => (
              <button key={m} role="radio" aria-checked={mode === m} className={mode === m ? "on" : ""} onClick={() => setMode(m)}>
                {label}
              </button>
            ))}
          </div>
          <button
            className={`mic ${rec ? "live" : ""}`}
            disabled={busy}
            aria-label={rec ? "Stop and transcribe" : "Start dictating"}
            onClick={() => void (rec ? stop() : start())}
            style={rec ? { boxShadow: `0 0 0 ${6 + Math.min(40, Math.sqrt(level) * 120)}px rgba(15,163,163,0.25)` } : undefined}
          >
            {busy ? "..." : rec ? "Stop" : "Speak"}
          </button>
          {rec && (
            <button className="link" onClick={() => { window.clearInterval(timer.current); rec.cancel(); setRec(null); }}>
              Cancel
            </button>
          )}
          {error && <p className="bad">{error}</p>}
          {result && result.text && (
            <section className="card">
              <p className="result">{result.text}</p>
              <div className="row">
                <button className="primary" onClick={() => copy(result.text)}>
                  {copied ? "Copied" : "Copy"}
                </button>
              </div>
              <p className="note">{result.how}</p>
            </section>
          )}
          {history.length > 0 && (
            <section className="card">
              <div className="row spread">
                <h2>History (this device only)</h2>
                <button onClick={() => void clearHistory().then(() => setHistory([]))}>Clear</button>
              </div>
              {history.slice(0, 30).map((h) => (
                <div key={h.at} className="item">
                  <p>{h.text}</p>
                  <button onClick={() => copy(h.text)}>Copy</button>
                </div>
              ))}
            </section>
          )}
        </>
      )}
      <footer className="note">Works on iPhone and Android in the browser. On iPhone it cannot type into other apps: copy and paste instead.</footer>
    </main>
  );
}

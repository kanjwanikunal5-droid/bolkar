import { useEffect, useRef, useState } from "react";
import { api, errorText, meetings, type MeetingLive, type MeetingResult } from "./api";
import { Icon } from "./icons";
import { PageHead, SettingRow, Switch } from "./ui";

function clock(s: number) {
  const h = Math.floor(s / 3600);
  const m = Math.floor(s / 60) % 60;
  const sec = s % 60;
  return `${h > 0 ? h + ":" : ""}${String(m).padStart(2, "0")}:${String(sec).padStart(2, "0")}`;
}

function Level({ label, value, on }: { label: string; value: number; on: boolean }) {
  const pct = Math.min(100, Math.round(Math.sqrt(value) * 220));
  return (
    <div className="field">
      <span>{label}</span>
      <span className="row">
        <meter min={0} max={100} value={on ? pct : 0} aria-label={`${label} level`} style={{ width: "12rem", height: "1.2rem" }} />
        <span className="dim">{on ? "" : "not recorded"}</span>
      </span>
    </div>
  );
}

/** Renders the notes Markdown as simple, readable blocks (headings, bullets, table rows). */
function Notes({ markdown }: { markdown: string }) {
  return (
    <div>
      {markdown.split("\n").map((line, i) => {
        if (line.startsWith("# ")) return <h2 key={i}>{line.slice(2)}</h2>;
        if (line.startsWith("## ")) return <h3 key={i} style={{ marginTop: "1rem" }}>{line.slice(3)}</h3>;
        if (line.startsWith("### ")) return <h4 key={i}>{line.slice(4)}</h4>;
        if (line.startsWith("|---")) return null;
        if (line.startsWith("|")) {
          const cells = line.split("|").slice(1, -1).map((c) => c.trim());
          return (
            <div key={i} className="row" style={{ borderTop: "1px solid var(--border)", padding: "0.25rem 0" }}>
              {cells.map((c, j) => (
                <span key={j} style={{ flex: j === 0 ? 3 : 1 }}>
                  {c}
                </span>
              ))}
            </div>
          );
        }
        if (line.trim() === "") return null;
        return (
          <p key={i} style={{ margin: "0.2rem 0" }}>
            {line.replace(/\*\*/g, "")}
          </p>
        );
      })}
    </div>
  );
}

export function MeetingsPage() {
  const [title, setTitle] = useState("");
  const [mic, setMic] = useState(true);
  const [system, setSystem] = useState(true);
  const [consent, setConsent] = useState(false);
  const [live, setLive] = useState<MeetingLive | null>(null);
  const [thought, setThought] = useState("");
  const [thoughts, setThoughts] = useState<[number, string][]>([]);
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<MeetingResult | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const seconds = useRef(0);

  useEffect(() => {
    const t = setInterval(() => {
      void meetings.status().then((s) => {
        setLive(s);
        if (s) seconds.current = s.seconds;
      });
    }, 500);
    return () => clearInterval(t);
  }, []);

  const recording = !!live?.recording;
  const start = () => {
    setError(null);
    setResult(null);
    setThoughts([]);
    void meetings.start(mic, system, consent).then(() => meetings.status().then(setLive), (e) => setError(errorText(e)));
  };
  const stop = () => {
    setBusy(true);
    setError(null);
    void meetings
      .stop(title, thoughts)
      .then(setResult, (e) => setError(errorText(e)))
      .finally(() => {
        setBusy(false);
        setConsent(false);
      });
  };
  const addThought = () => {
    if (!thought.trim()) return;
    setThoughts((t) => [...t, [seconds.current * 1000, thought.trim()]]);
    setThought("");
  };

  return (
    <>
      <PageHead
        title="Meeting notes"
        sub="Records your microphone and the call audio, transcribes on this computer, and writes a summary with decisions and action items."
      />
      {error && (
        <div className="banner" role="alert">
          {error}
        </div>
      )}

      {!recording && !busy && (
        <div className="card">
          <SettingRow title="Title" help="Shown at the top of the notes.">
            <input id="mtitle" type="text" aria-label="Meeting title" value={title} placeholder="e.g. Weekly finance review" onChange={(e) => setTitle(e.target.value)} />
          </SettingRow>
          <SettingRow title="My microphone" help="Your voice, labelled “Me”.">
            <Switch label="Record my microphone" checked={mic} onChange={setMic} />
          </SettingRow>
          <SettingRow title="Computer sound" help="Zoom, Google Meet, Teams: the other side, labelled “Others”. Use headphones if you can.">
            <Switch label="Record computer sound" checked={system} onChange={setSystem} />
          </SettingRow>
          <SettingRow title="Permission" help="I have told the participants and I am allowed to record this meeting.">
            <Switch label="I am allowed to record this meeting" checked={consent} onChange={setConsent} />
          </SettingRow>
          <div className="row" style={{ marginTop: "0.6rem" }}>
            <button className="btn primary" disabled={!consent || (!mic && !system)} onClick={start}>
              <Icon.Mic size={16} /> Start recording
            </button>
            {!consent && <span className="dim">Turn on Permission to start.</span>}
          </div>
        </div>
      )}

      {recording && (
        <div className="card">
          <div className="row spread">
            <strong className="bad row" role="status" style={{ gap: "0.5rem" }}>
              <span className="dot pulse" style={{ background: "var(--danger)" }} />
              Recording {clock(live?.seconds ?? 0)}
            </strong>
            <button className="btn primary" onClick={stop}>
              Stop and make notes
            </button>
          </div>
          {live?.problems.map((p) => (
            <div className="banner" role="alert" key={p}>
              {p} Recording continues with the other source.
            </div>
          ))}
          <Level label="Microphone" value={live?.mic_level ?? 0} on={mic} />
          <Level label="Computer sound" value={live?.system_level ?? 0} on={system} />
          <h3 style={{ marginTop: "1rem" }}>My thoughts</h3>
          <p className="dim">Your own notes. They are kept exactly as you write them and never changed by the AI.</p>
          <div className="row">
            <input
              type="text"
              aria-label="Add a note"
              style={{ flex: 1 }}
              value={thought}
              onChange={(e) => setThought(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && addThought()}
            />
            <button className="btn" onClick={addThought}>
              Add note
            </button>
          </div>
          {thoughts.map(([ms, t], i) => (
            <p key={i}>
              <span className="dim">[{clock(Math.floor(ms / 1000))}]</span> {t}
            </p>
          ))}
        </div>
      )}

      {busy && (
        <div className="card" role="status">
          Transcribing on this computer and writing the notes... this takes about a minute per hour of meeting.
        </div>
      )}

      {result && (
        <div className="card">
          <div className="row spread">
            <strong>{result.segments.length} transcript segments</strong>
            <button
              className="btn primary"
              onClick={() =>
                void api.copy(result.markdown).then(() => {
                  setCopied(true);
                  setTimeout(() => setCopied(false), 1500);
                })
              }
            >
              {copied ? "Copied" : "Copy notes"}
            </button>
          </div>
          {result.note && <p className="dim">{result.note}</p>}
          {result.dropped.length > 0 && (
            <details>
              <summary className="dim">{result.dropped.length} item(s) removed because the transcript did not support them</summary>
              {result.dropped.map((d, i) => (
                <p className="dim" key={i}>
                  {d}
                </p>
              ))}
            </details>
          )}
          <Notes markdown={result.markdown} />
        </div>
      )}
    </>
  );
}

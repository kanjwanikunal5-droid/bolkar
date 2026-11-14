import { useState } from "react";
import { api, type AiStatus, type Dictation, type Insights, type Settings, type Status } from "./api";
import { Icon } from "./icons";
import { CountUp, EmptyState, MicLevel, ProgressBar, appName, dayLabel, greeting, timeOf, useToast } from "./ui";

export type Go = (page: string, section?: string) => void;

/** Decorative sound waves for the hero. */
function Waves() {
  const bars = [18, 34, 58, 88, 64, 110, 76, 130, 92, 60, 104, 70, 44, 82, 52, 30, 20];
  return (
    <svg className="waves" width="360" height="160" viewBox="0 0 360 160" aria-hidden="true">
      {bars.map((h, i) => (
        <rect key={i} x={i * 21} y={80 - h / 2} width="10" height={h} rx="5" fill="#fff" />
      ))}
    </svg>
  );
}

function Hero({ name, insights }: { name: string; insights: Insights | null }) {
  const streak = insights?.current_streak ?? 0;
  return (
    <section className="hero" aria-label="Welcome">
      <Waves />
      <h1>
        {greeting()}
        {name ? `, ${name}` : ""}
      </h1>
      <p>
        Hold <kbd>Ctrl</kbd> + <kbd>Win</kbd> and speak into any app. Let go, and it types.
      </p>
      <div className="chips">
        <span className="chip" title="Days in a row with at least one dictation">
          <Icon.Flame size={16} />
          {streak > 0 ? `${streak}-day streak` : "Start a streak today"}
        </span>
        <span className="chip">
          <Icon.Words size={16} />
          <CountUp value={insights?.words_this_week ?? 0} /> words this week
        </span>
        {insights?.speaking_wpm != null && (
          <span className="chip" title="Your speaking speed while dictating">
            <Icon.Gauge size={16} />
            <CountUp value={Math.round(insights.speaking_wpm)} /> words per minute
          </span>
        )}
      </div>
    </section>
  );
}

function Recent({ dictations, go }: { dictations: Dictation[]; go: Go }) {
  const toast = useToast();
  const items = dictations.filter((d) => d.final_text).slice(0, 6);
  return (
    <div className="card">
      <div className="card-title">
        <h2>Recent</h2>
        <button className="btn ghost small" onClick={() => go("history")}>
          See all <Icon.Chevron size={14} />
        </button>
      </div>
      {items.length === 0 ? (
        <EmptyState icon={<Icon.Mic size={36} />} title="Nothing dictated yet">
          Click into any text box, hold <kbd>Ctrl</kbd> + <kbd>Win</kbd> and say a sentence. It will appear here.
        </EmptyState>
      ) : (
        items.map((d, i) => {
          const showDay = i === 0 || dayLabel(d.created_at) !== dayLabel(items[i - 1].created_at);
          return (
            <div key={d.id}>
              {showDay && <div className="day-head">{dayLabel(d.created_at)}</div>}
              <div className="timeline-item">
                <span className="time">{timeOf(d.created_at)}</span>
                <div style={{ minWidth: 0 }}>
                  <p className="text">{d.final_text}</p>
                  <div className="meta">
                    {appName(d.app_process) || "No app"} · {d.word_count} words
                    {d.level === "COMMAND" && <span className="badge accent" style={{ marginLeft: "0.4rem" }}>Command</span>}
                  </div>
                </div>
                <span className="actions row" style={{ gap: "0.2rem" }}>
                  <button
                    className="btn ghost small"
                    aria-label="Copy"
                    title="Copy"
                    onClick={() => void api.copy(d.final_text).then(() => toast("Copied"))}
                  >
                    <Icon.Copy size={16} />
                  </button>
                </span>
              </div>
            </div>
          );
        })
      )}
    </div>
  );
}

interface Step {
  done: boolean;
  label: string;
  hint: string;
  go: () => void;
}

function Checklist({ steps }: { steps: Step[] }) {
  const done = steps.filter((s) => s.done).length;
  if (done === steps.length) return null;
  return (
    <div className="card">
      <div className="card-title">
        <h2>Get the most out of it</h2>
        <span className="badge accent">
          {done} of {steps.length}
        </span>
      </div>
      <ProgressBar value={done} max={steps.length} />
      <div className="stack" style={{ marginTop: "0.6rem", gap: "0.1rem" }}>
        {steps.map((s) => (
          <button key={s.label} className={`check-item ${s.done ? "done" : ""}`} onClick={s.go}>
            <span className="tick">
              <Icon.Check size={13} />
            </span>
            <span>
              <span className="label" style={{ display: "block", fontWeight: 600 }}>
                {s.label}
              </span>
              {!s.done && (
                <span className="dim" style={{ fontSize: "0.72rem" }}>
                  {s.hint}
                </span>
              )}
            </span>
            <span className="chev">
              <Icon.Chevron size={16} />
            </span>
          </button>
        ))}
      </div>
    </div>
  );
}

function TryIt() {
  const [text, setText] = useState("");
  return (
    <div className="card">
      <div className="card-title">
        <h2>Try it here</h2>
        <MicLevel />
      </div>
      <textarea
        className="practice"
        aria-label="Practice box"
        value={text}
        onChange={(e) => setText(e.target.value)}
        placeholder="Click here, hold Ctrl + Win and speak. Try: “Let's meet at 2 PM... actually make that 3 PM.”"
      />
    </div>
  );
}

function SetupCard({ status, ai, settings, go }: { status: Status | null; ai: AiStatus | null; settings: Settings; go: Go }) {
  const lang: Record<string, string> = { "en-hinglish": "English + Hinglish", en: "English", hinglish: "Hinglish" };
  const rows: [string, string, boolean, string][] = [
    ["Speech recognition", status?.cloud_speech ? "Groq Whisper (cloud)" : "On this computer", true, "speech"],
    ["AI clean-up", ai?.active ? "On" : "Off", ai?.active ?? false, "groq"],
    ["Language", lang[settings.language ?? ""] ?? settings.language ?? "", true, "speech"],
  ];
  return (
    <div className="card">
      <div className="card-title">
        <h2>Your setup</h2>
        <button className="btn ghost small" onClick={() => go("settings", "groq")}>
          Change
        </button>
      </div>
      {rows.map(([k, v, on, section]) => (
        <button key={k} className="check-item" onClick={() => go("settings", section)}>
          <span className={`dot ${on ? "on" : "warn"}`} />
          <span style={{ flex: 1 }}>
            <span className="dim" style={{ fontSize: "0.72rem", display: "block" }}>
              {k}
            </span>
            <strong>{v}</strong>
          </span>
          <span className="chev">
            <Icon.Chevron size={16} />
          </span>
        </button>
      ))}
    </div>
  );
}

export function HomePage({
  status,
  settings,
  dictations,
  insights,
  ai,
  go,
}: {
  status: Status | null;
  settings: Settings;
  dictations: Dictation[];
  insights: Insights | null;
  ai: AiStatus | null;
  go: Go;
}) {
  const steps: Step[] = [
    {
      done: !!status?.model_installed,
      label: "Install a speech model",
      hint: "One download, then dictation works offline.",
      go: () => go("settings", "speech"),
    },
    {
      done: !!ai?.groq_key_hint && !!ai?.active,
      label: "Connect Groq for the best accuracy",
      hint: "Free key. Best measured English and Hinglish.",
      go: () => go("settings", "groq"),
    },
    {
      done: (insights?.total_dictations ?? 0) > 0,
      label: "Dictate your first sentence",
      hint: "Hold Ctrl + Win in any app and speak.",
      go: () => go("guide"),
    },
    {
      done: (insights?.commands ?? 0) > 0,
      label: "Edit text by voice",
      hint: "Select text, press Alt + Shift + C, say “make this a bulleted list”.",
      go: () => go("guide"),
    },
    {
      done: (insights?.your_words ?? 0) + (insights?.learned_words ?? 0) > 0,
      label: "Teach it a word",
      hint: "Names and terms you use, written your way.",
      go: () => go("dictionary"),
    },
    {
      done: !!settings.scratchpad,
      label: "Jot something in the Scratchpad",
      hint: "Alt + Shift + N from anywhere.",
      go: () => go("scratchpad"),
    },
  ];
  return (
    <div className="page">
      <Hero name={settings.user_name ?? ""} insights={insights} />
      <div className="grid home">
        <Recent dictations={dictations} go={go} />
        <div>
          <Checklist steps={steps} />
          <TryIt />
          <SetupCard status={status} ai={ai} settings={settings} go={go} />
        </div>
      </div>
    </div>
  );
}

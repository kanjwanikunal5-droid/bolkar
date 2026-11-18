import { useEffect, useState } from "react";
import { api, type AiStatus, type Dictation, type Insights, type Settings, type Status } from "./api";
import { Icon } from "./icons";
import photoEdit from "./assets/banner-edit.jpg";
import photoHinglish from "./assets/banner-hinglish.jpg";
import photoPoints from "./assets/banner-points.jpg";
import photoStyle from "./assets/banner-style.jpg";
import { CountUp, EmptyState, MicLevel, ProgressBar, appName, dayLabel, greeting, timeOf, useToast } from "./ui";

export type Go = (page: string, section?: string) => void;

/* ---------- Greeting row with stat pills ---------- */

function Greeting({ name, insights }: { name: string; insights: Insights | null }) {
  const streak = insights?.current_streak ?? 0;
  return (
    <div className="greet">
      <div>
        <h1>
          {greeting()}
          {name ? `, ${name}` : ""}
        </h1>
        <p className="dim">
          Hold <kbd>Ctrl</kbd> + <kbd>Win</kbd> and speak into any app.
        </p>
      </div>
      <div className="pills" aria-label="Your numbers">
        <span className="pill-stat" title="Days in a row with at least one dictation">
          <span className="pill-ico amber">
            <Icon.Flame size={15} />
          </span>
          {streak > 0 ? `${streak} day${streak === 1 ? "" : "s"}` : "No streak yet"}
        </span>
        <span className="pill-stat" title="Words dictated in the last 7 days">
          <span className="pill-ico">
            <Icon.Words size={15} />
          </span>
          <CountUp value={insights?.words_this_week ?? 0} /> words
        </span>
        <span className="pill-stat" title="Your speaking speed while dictating">
          <span className="pill-ico">
            <Icon.Gauge size={15} />
          </span>
          {insights?.speaking_wpm ? (
            <>
              <CountUp value={Math.round(insights.speaking_wpm)} /> WPM
            </>
          ) : (
            "– WPM"
          )}
        </span>
      </div>
    </div>
  );
}

/* ---------- Daily goal ---------- */

function Confetti() {
  const colors = ["#22b3a5", "#f6a623", "#6366f1", "#ef4444", "#5bdacd"];
  return (
    <span className="confetti" aria-hidden="true">
      {Array.from({ length: 28 }, (_, i) => (
        <i
          key={i}
          style={{
            left: `${(i * 37) % 100}%`,
            background: colors[i % colors.length],
            animationDelay: `${(i % 7) * 60}ms`,
            transform: `rotate(${i * 29}deg)`,
          }}
        />
      ))}
    </span>
  );
}

function DailyGoal({ insights, goal, go }: { insights: Insights | null; goal: number; go: Go }) {
  const today = insights?.words_today ?? 0;
  const done = today >= goal;
  const [celebrate, setCelebrate] = useState(false);
  useEffect(() => {
    if (!done) return;
    const key = `goal-celebrated-${new Date().toDateString()}`;
    try {
      if (localStorage.getItem(key)) return;
      localStorage.setItem(key, "1");
    } catch {
      /* no storage: celebrate anyway */
    }
    setCelebrate(true);
    const t = setTimeout(() => setCelebrate(false), 2600);
    return () => clearTimeout(t);
  }, [done]);
  return (
    <section className={`goal card ${done ? "done" : ""}`} aria-label="Daily goal">
      {celebrate && <Confetti />}
      <div className="goal-body">
        <h2>{done ? "Goal reached for today" : `${goal} words a day`}</h2>
        <p className="dim">
          {done
            ? `You dictated ${today.toLocaleString()} words today. Keep the streak going tomorrow.`
            : today === 0
              ? "Speak your first words today to start."
              : `${(goal - today).toLocaleString()} words to go. Every dictation counts.`}
        </p>
        <div className="goal-bar">
          <ProgressBar value={Math.min(today, goal)} max={goal} />
          <span className="goal-end" aria-hidden="true">
            {done ? "🏆" : "🎯"}
          </span>
        </div>
        <span className="goal-count">
          {Math.min(today, goal).toLocaleString()}/{goal.toLocaleString()} words
        </span>
      </div>
      <button className="goal-tile" onClick={() => go("insights")} aria-label="Open Insights" title="Your Insights">
        <Icon.Logo size={34} />
      </button>
    </section>
  );
}

/* ---------- Feature banner (rotating) ---------- */

interface Slide {
  title: JSX.Element;
  text: string;
  cta: string;
  photo: string;
  /** Mirror the photo so the person faces the text. */
  flip?: boolean;
  go: () => void;
}

function Banner({ go }: { go: Go }) {
  const slides: Slide[] = [
    {
      title: (
        <>
          Speak Hinglish, <em>type</em> it like WhatsApp
        </>
      ),
      text: "English or Hinglish, decided for every sentence. Hinglish comes out in Roman letters, spelled the way you text.",
      cta: "Try it now",
      photo: photoHinglish,
      go: () => go("guide"),
    },
    {
      title: (
        <>
          Edit by voice, <em>anywhere</em>
        </>
      ),
      text: "Select text, press Alt + Shift + C and say “make this a bulleted list”. Word and Google Docs get a real list.",
      cta: "See how",
      photo: photoEdit,
      go: () => go("guide"),
    },
    {
      title: (
        <>
          A style for <em>every</em> app
        </>
      ),
      text: "Casual in WhatsApp, formal in email, clear everywhere else. Choose once; it is used every time.",
      cta: "Choose styles",
      photo: photoStyle,
      flip: true,
      go: () => go("style"),
    },
    {
      title: (
        <>
          Speak in <em>points</em>
        </>
      ),
      text: "Normal speech stays a natural paragraph. Say “first thing…, second thing…”, “pehli baat…” or “bullet point…” and you get a tidy list.",
      cta: "List settings",
      photo: photoPoints,
      go: () => go("settings", "writing"),
    },
  ];
  const [i, setI] = useState(0);
  const [paused, setPaused] = useState(false);
  useEffect(() => {
    if (paused || window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    const t = setInterval(() => setI((n) => (n + 1) % slides.length), 7000);
    return () => clearInterval(t);
  }, [paused, slides.length]);
  const s = slides[i];
  return (
    <section
      className="banner-hero"
      aria-roledescription="carousel"
      aria-label="What you can do"
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
      onFocus={() => setPaused(true)}
      onBlur={() => setPaused(false)}
    >
      <div className="banner-copy" key={i}>
        <h2>{s.title}</h2>
        <p>{s.text}</p>
        <button className="btn on-hero" onClick={s.go}>
          {s.cta} <Icon.Chevron size={15} />
        </button>
      </div>
      <div className="banner-photo" key={`a${i}`} aria-hidden="true">
        <img src={s.photo} alt="" className={s.flip ? "flip" : undefined} draggable={false} />
      </div>
      <div className="banner-dots" role="tablist" aria-label="Choose a slide">
        {slides.map((_, n) => (
          <button key={n} role="tab" aria-selected={n === i} aria-label={`Slide ${n + 1}`} onClick={() => setI(n)} />
        ))}
      </div>
    </section>
  );
}

/* ---------- Today ---------- */

function Recent({ dictations, go }: { dictations: Dictation[]; go: Go }) {
  const toast = useToast();
  const items = dictations.filter((d) => d.final_text).slice(0, 8);
  return (
    <section>
      <div className="section-head">
        <span className="day-head" style={{ margin: 0 }}>
          Recent dictations
        </span>
        <button className="btn ghost small" onClick={() => go("history")}>
          See all <Icon.Chevron size={14} />
        </button>
      </div>
      <div className="card">
        {items.length === 0 ? (
          <EmptyState icon={<Icon.Mic size={36} />} title="Nothing dictated yet">
            Click into any text box, hold <kbd>Ctrl</kbd> + <kbd>Win</kbd> and say a sentence. It will appear here.
          </EmptyState>
        ) : (
          items.map((d, n) => {
            const showDay = n === 0 || dayLabel(d.created_at) !== dayLabel(items[n - 1].created_at);
            return (
              <div key={d.id}>
                {showDay && <div className="day-head">{dayLabel(d.created_at)}</div>}
                <div className="timeline-item">
                  <span className="time">{timeOf(d.created_at)}</span>
                  <div style={{ minWidth: 0 }}>
                    <p className="text">{d.final_text}</p>
                    <div className="meta">
                      {appName(d.app_process) || "No app"} · {d.word_count} words
                      {d.level === "COMMAND" && (
                        <span className="badge accent" style={{ marginLeft: "0.4rem" }}>
                          Command
                        </span>
                      )}
                    </div>
                  </div>
                  <span className="actions row" style={{ gap: "0.2rem" }}>
                    <button className="btn ghost small" aria-label="Copy" title="Copy" onClick={() => void api.copy(d.final_text).then(() => toast("Copied"))}>
                      <Icon.Copy size={16} />
                    </button>
                  </span>
                </div>
              </div>
            );
          })
        )}
      </div>
    </section>
  );
}

/* ---------- Side cards ---------- */

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
        <MicLevel bars={10} />
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
    ["Speech recognition", status?.cloud_speech ? "Groq Whisper (cloud)" : "On this computer", true, "groq"],
    ["Graphics card", status?.gpu_on ? "On (fast)" : status?.gpu_present ? "Off: slow" : "Not available", !!status?.gpu_on || !status?.gpu_present, "speech"],
    ["AI clean-up", ai?.active ? "On" : "Off", ai?.active ?? false, "groq"],
    ["Language", lang[settings.language ?? ""] ?? settings.language ?? "", true, "speech"],
  ];
  return (
    <div className="card">
      <div className="card-title">
        <h2>Your setup</h2>
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
  const goal = Math.max(10, Number(settings.daily_goal) || 100);
  const steps: Step[] = [
    { done: !!status?.model_installed, label: "Install a speech model", hint: "One download, then it also works offline.", go: () => go("settings", "speech") },
    { done: !!ai?.groq_key_hint && !!ai?.active, label: "Connect Groq for the best accuracy", hint: "Free key. Best measured English and Hinglish.", go: () => go("settings", "groq") },
    { done: (insights?.total_dictations ?? 0) > 0, label: "Dictate your first sentence", hint: "Hold Ctrl + Win in any app and speak.", go: () => go("guide") },
    { done: (insights?.commands ?? 0) > 0, label: "Edit text by voice", hint: "Select text, Alt + Shift + C, “make this a bulleted list”.", go: () => go("guide") },
    { done: (insights?.your_words ?? 0) + (insights?.learned_words ?? 0) > 0, label: "Teach it a word", hint: "Names and terms you use, written your way.", go: () => go("dictionary") },
    { done: !!settings.scratchpad, label: "Jot something in the Scratchpad", hint: "Alt + Shift + N from anywhere.", go: () => go("scratchpad") },
  ];
  return (
    <div className="page">
      <Greeting name={settings.user_name ?? ""} insights={insights} />
      {status?.gpu_present && !status.gpu_on && (
        <div className="banner" role="alert">
          <Icon.Info size={18} /> The graphics card is switched off, so speech recognition on this computer is very slow.
          <button className="btn small primary" onClick={() => go("settings", "speech")}>
            Fix it
          </button>
        </div>
      )}
      {(status?.other_dictation_apps?.length ?? 0) > 0 && (
        <div className="banner" role="alert">
          <Icon.Info size={18} /> {status?.other_dictation_apps.join(" and ")} {status?.other_dictation_apps.length === 1 ? "is" : "are"} also running. If
          Ctrl + Win does nothing or types twice, quit {status?.other_dictation_apps.length === 1 ? "it" : "them"} from the system tray.
        </div>
      )}
      <DailyGoal insights={insights} goal={goal} go={go} />
      <Banner go={go} />
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

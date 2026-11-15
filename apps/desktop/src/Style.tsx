import type { AiStatus, Settings } from "./api";
import { Icon } from "./icons";
import { PageHead } from "./ui";

const GROUPS: { key: string; title: string; apps: string; icon: JSX.Element }[] = [
  { key: "style_chat", title: "Personal messages", apps: "WhatsApp, Telegram, Slack, Teams chat", icon: <Icon.Quote size={18} /> },
  { key: "style_email", title: "Email", apps: "Outlook, Gmail and other mail", icon: <Icon.Note size={18} /> },
  { key: "style_docs", title: "Documents", apps: "Word, Google Docs, Notion", icon: <Icon.Book size={18} /> },
  { key: "style_other", title: "Everything else", apps: "Browsers, notes and other apps", icon: <Icon.Globe size={18} /> },
];

/** Example output of each style for the same spoken sentence (illustration, not a promise). */
const OPTIONS: [string, string, string][] = [
  ["casual", "Casual", "hey, running 10 mins late, start without me"],
  ["clear", "Clear", "Hey, I'm running 10 minutes late. Please start without me."],
  ["formal", "Formal", "Hello, I am running ten minutes late. Please begin without me."],
];

export function StylePage({
  settings,
  ai,
  set,
  go,
}: {
  settings: Settings;
  ai: AiStatus | null;
  set: (key: string, value: string, note?: string) => void;
  go: (page: string, section?: string) => void;
}) {
  return (
    <div className="page">
      <PageHead
        title="Style"
        sub="How your dictation is written, by kind of app. The app you are typing in decides which style is used."
      />
      {!ai?.active && (
        <div className="banner info">
          <Icon.Sparkles size={18} /> Styles are applied by AI clean-up, which is off. Without it, dictation is tidied by the built-in rules
          only.
          <button className="btn small primary" onClick={() => go("settings", "groq")}>
            Turn on
          </button>
        </div>
      )}
      <p className="dim" style={{ marginTop: 0 }}>
        Example: you say “hey running ten minutes late start without me”.
      </p>
      {GROUPS.map((g) => {
        const current = settings[g.key] || "clear";
        return (
          <div className="card" key={g.key}>
            <div className="card-title">
              <h2 className="row" style={{ gap: "0.6rem" }}>
                <span className="stat-icon" style={{ marginBottom: 0 }}>
                  {g.icon}
                </span>
                <span>
                  {g.title}
                  <span className="dim" style={{ display: "block", fontSize: "0.72rem", fontWeight: 500 }}>
                    {g.apps}
                  </span>
                </span>
              </h2>
            </div>
            <div className="style-grid" role="radiogroup" aria-label={`${g.title} style`}>
              {OPTIONS.map(([id, label, sample]) => (
                <button
                  key={id}
                  role="radio"
                  aria-checked={current === id}
                  className="style-option"
                  onClick={() => set(g.key, id, `${g.title}: ${label}`)}
                >
                  <strong>
                    {label}
                    {current === id && (
                      <span className="badge ok">
                        <Icon.Check size={12} /> In use
                      </span>
                    )}
                  </strong>
                  <span className="sample">{sample}</span>
                </button>
              ))}
            </div>
          </div>
        );
      })}
    </div>
  );
}

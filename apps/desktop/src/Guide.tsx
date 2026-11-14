import { useState } from "react";
import { Icon } from "./icons";
import { Keys, MicLevel, PageHead } from "./ui";

const GROUPS: { title: string; icon: JSX.Element; items: [string, string][] }[] = [
  {
    title: "Dictate",
    icon: <Icon.Mic size={18} />,
    items: [
      ["Hold Ctrl + Win", "Speak while holding, let go to type the text where your cursor is."],
      ["Double-tap Ctrl + Win", "Hands-free on. Speak as long as you like (up to 20 minutes)."],
      ["Ctrl + Win + H", "Hands-free on or off."],
      ["Esc", "Cancel while recording. Nothing is typed."],
    ],
  },
  {
    title: "Edit and reuse",
    icon: <Icon.Wand size={18} />,
    items: [
      ["Alt + Shift + C", "Command Mode: select text, press, say what to do, press again."],
      ["Alt + Shift + F", "Fix the last dictation. Your fix teaches the dictionary."],
      ["Alt + Shift + Z", "Paste the last dictation again."],
      ["Alt + Shift + X", "Copy the last dictation."],
    ],
  },
  {
    title: "More",
    icon: <Icon.Sparkles size={18} />,
    items: [
      ["Alt + Shift + H", "Switch language: English + Hinglish, English only, Hinglish only."],
      ["Alt + Shift + N", "Open the Scratchpad."],
    ],
  },
];

const SPOKEN: [string, string][] = [
  ["“new line” / “new paragraph”", "Starts a new line or paragraph."],
  ["“full stop”, “comma”, “question mark”", "Punctuation, when you want it exactly."],
  ["“Let's meet at 2... actually make that 3”", "Self-corrections: you get “Let's meet at 3.”"],
  ["“First... second... third...”", "Becomes a numbered list."],
];

const COMMANDS = [
  "make this a bulleted list",
  "make it a numbered list",
  "all caps",
  "lowercase",
  "title case",
  "make it one paragraph",
  "clean this up",
  "bullet points bana do",
  "make it more formal (AI)",
  "make it shorter (AI)",
  "translate to English (AI)",
];

export function GuidePage() {
  const [text, setText] = useState("");
  return (
    <div className="page">
      <PageHead title="How to use" sub="Everything works in any app: Word, Gmail, WhatsApp, Chrome, Excel, VS Code." />

      <div className="grid cols-3" style={{ marginBottom: "1rem" }}>
        {[
          ["1", "Click where the text should go", "Any text box in any app."],
          ["2", "Hold Ctrl + Win and speak", "Naturally, in English or Hinglish."],
          ["3", "Let go", "Clean text is typed for you in about a second."],
        ].map(([n, t, s]) => (
          <div className="card lift" key={n}>
            <span className="stat-icon" style={{ fontWeight: 800 }}>
              {n}
            </span>
            <h3>{t}</h3>
            <p className="dim" style={{ margin: 0 }}>
              {s}
            </p>
          </div>
        ))}
      </div>

      <div className="card">
        <div className="card-title">
          <h2>Try it now</h2>
          <MicLevel />
        </div>
        <textarea
          className="practice"
          aria-label="Practice box"
          value={text}
          onChange={(e) => setText(e.target.value)}
          placeholder="Click here, hold Ctrl + Win, say “Bhai kal meeting chaar baje hai, please time pe aa jana.”"
        />
      </div>

      <div className="grid">
        {GROUPS.map((g) => (
          <div className="card" key={g.title}>
            <div className="card-title">
              <h2 className="row" style={{ gap: "0.5rem" }}>
                <span className="stat-icon" style={{ marginBottom: 0 }}>
                  {g.icon}
                </span>
                {g.title}
              </h2>
            </div>
            {g.items.map(([k, what]) => (
              <div className="shortcut" key={k}>
                <Keys combo={k} />
                <span>{what}</span>
              </div>
            ))}
          </div>
        ))}

        <div className="card">
          <div className="card-title">
            <h2>Say it while dictating</h2>
          </div>
          {SPOKEN.map(([say, what]) => (
            <div className="shortcut" key={say}>
              <strong>{say}</strong>
              <span className="dim">{what}</span>
            </div>
          ))}
        </div>
      </div>

      <div className="card">
        <div className="card-title">
          <h2>Command Mode: things you can say</h2>
          <Keys combo="Alt + Shift + C" />
        </div>
        <div className="row">
          {COMMANDS.map((c) => (
            <span key={c} className={`badge ${c.endsWith("(AI)") ? "" : "accent"}`} style={{ fontSize: "0.72rem", padding: "0.3rem 0.7rem" }}>
              “{c.replace(" (AI)", "")}”{c.endsWith("(AI)") ? " · needs AI" : ""}
            </span>
          ))}
        </div>
        <p className="dim" style={{ marginBottom: 0 }}>
          Lists become real lists in Word and Google Docs. Undo with Ctrl + Z, as usual.
        </p>
      </div>
    </div>
  );
}

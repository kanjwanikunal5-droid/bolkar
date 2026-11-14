import { useEffect, useRef, useState } from "react";
import { api } from "./api";

/** Notes you can dictate into (Alt + Shift + N). Saved on this computer as you type. */
export function ScratchpadPage({ initial, focusSignal }: { initial: string; focusSignal: number }) {
  const [text, setText] = useState(initial);
  const [saved, setSaved] = useState(true);
  const box = useRef<HTMLTextAreaElement>(null);
  const timer = useRef<number | undefined>(undefined);

  useEffect(() => {
    box.current?.focus();
    const end = box.current?.value.length ?? 0;
    box.current?.setSelectionRange(end, end);
  }, [focusSignal]);

  const change = (value: string) => {
    setText(value);
    setSaved(false);
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => {
      void api.setSetting("scratchpad", value).then(() => setSaved(true));
    }, 600);
  };

  return (
    <>
      <h2>Scratchpad</h2>
      <p className="dim">
        Quick notes. Press <kbd>Alt</kbd> + <kbd>Shift</kbd> + <kbd>N</kbd> from any app to come here, then hold{" "}
        <kbd>Ctrl</kbd> + <kbd>Win</kbd> and speak. Select text and press <kbd>Alt</kbd> + <kbd>Shift</kbd> + <kbd>C</kbd>{" "}
        to edit it by voice (for example "make this a bulleted list").
      </p>
      <div className="card">
        <textarea
          ref={box}
          className="practice scratchpad"
          aria-label="Scratchpad"
          value={text}
          onChange={(e) => change(e.target.value)}
          placeholder="Click here, hold Ctrl + Win and speak."
        />
        <div className="row spread">
          <span className="dim" aria-live="polite">
            {saved ? "Saved on this computer" : "Saving..."}
          </span>
          <span>
            <button className="btn" onClick={() => void navigator.clipboard.writeText(text)}>
              Copy all
            </button>{" "}
            <button
              className="btn"
              onClick={() => {
                if (text && window.confirm("Clear the scratchpad? This cannot be undone.")) change("");
              }}
            >
              Clear
            </button>
          </span>
        </div>
      </div>
    </>
  );
}

import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import type { PillState } from "./api";
import "./styles.css";

const HIDDEN: PillState = { state: "hidden", message: "", level: 0, seconds: 0, hands_free: false, engine: "" };

function clock(seconds: number) {
  const m = Math.floor(seconds / 60);
  const s = seconds % 60;
  return `${m}:${s.toString().padStart(2, "0")}`;
}

/** Five bars whose height follows the microphone level. */
function Bars({ level }: { level: number }) {
  // Speech RMS is small (about 0.02 to 0.2), so use a square-root scale to make it visible.
  const strength = Math.min(1, Math.sqrt(level) * 2.2);
  const shape = [0.55, 0.8, 1, 0.8, 0.55];
  return (
    <div className="bars" aria-hidden="true">
      {shape.map((f, i) => (
        <span key={i} style={{ height: `${4 + strength * f * 22}px` }} />
      ))}
    </div>
  );
}

function Pill() {
  const [pill, setPill] = useState<PillState>(HIDDEN);
  useEffect(() => {
    const off = listen<PillState>("pill", (e) => setPill(e.payload));
    return () => {
      void off.then((f) => f());
    };
  }, []);

  if (pill.state === "hidden") return null;
  const listening = pill.state === "listening";
  const text = listening
    ? pill.message || (pill.hands_free ? "Listening (hands-free)" : "Listening")
    : pill.state === "done"
      ? `${pill.message} ✓`
      : pill.message;

  return (
    <div className={`pill ${pill.state}`} role="status" aria-live="polite">
      {listening && <span className="dot" aria-hidden="true" />}
      {listening && <Bars level={pill.level} />}
      <span className="msg">{text}</span>
      {listening && <span>{clock(pill.seconds)}</span>}
      {pill.engine && <span className="engine">{pill.engine}</span>}
    </div>
  );
}

createRoot(document.getElementById("root")!).render(<Pill />);

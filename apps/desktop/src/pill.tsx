import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import type { PillState } from "./api";
import "./styles.css";

const HIDDEN: PillState = { state: "hidden", message: "", level: 0, seconds: 0, hands_free: false, engine: "", sounds: false };

/** Short, quiet cues made with the Web Audio API (no sound files). */
let audio: AudioContext | null = null;
function cue(kind: "start" | "done" | "error") {
  try {
    audio ??= new AudioContext();
    const notes = kind === "start" ? [523.25, 783.99] : kind === "done" ? [880, 1174.66] : [311.13, 233.08];
    notes.forEach((f, i) => {
      const t = audio!.currentTime + i * 0.09;
      const osc = audio!.createOscillator();
      const gain = audio!.createGain();
      osc.type = "sine";
      osc.frequency.value = f;
      gain.gain.setValueAtTime(0, t);
      gain.gain.linearRampToValueAtTime(0.06, t + 0.015);
      gain.gain.exponentialRampToValueAtTime(0.0001, t + 0.16);
      osc.connect(gain).connect(audio!.destination);
      osc.start(t);
      osc.stop(t + 0.18);
    });
  } catch {
    /* no audio device: stay silent */
  }
}
const BARS = 9;

/**
 * Live waveform: each bar follows the microphone level with its own small offset, so the
 * shape moves like speech instead of pulsing as one block. Falls back to a gentle idle shimmer.
 */
function Wave({ level, busy }: { level: number; busy: boolean }) {
  const [heights, setHeights] = useState<number[]>(() => Array(BARS).fill(0.15));
  const levelRef = useRef(level);
  levelRef.current = level;
  useEffect(() => {
    let frame = 0;
    let t = 0;
    const tick = () => {
      t += 1;
      // Speech RMS is small (about 0.01 to 0.2): square-root scale makes it visible.
      const strength = busy ? 0.35 : Math.min(1, Math.sqrt(levelRef.current) * 2.6);
      setHeights((prev) =>
        prev.map((h, i) => {
          const centre = 1 - Math.abs(i - (BARS - 1) / 2) / ((BARS - 1) / 2 + 1);
          const wobble = 0.55 + 0.45 * Math.sin(t / (busy ? 6 : 3.2) + i * 1.3);
          const target = Math.max(0.12, strength * (0.35 + 0.65 * centre) * wobble);
          return h + (target - h) * 0.35;
        }),
      );
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(frame);
  }, [busy]);
  return (
    <div className={`wave ${busy ? "busy" : ""}`} aria-hidden="true">
      {heights.map((h, i) => (
        <span key={i} style={{ height: `${Math.round(4 + h * 22)}px` }} />
      ))}
    </div>
  );
}

function Pill() {
  const [pill, setPill] = useState<PillState>(HIDDEN);
  useEffect(() => {
    let last = "hidden";
    const off = listen<PillState>("pill", (e) => {
      const next = e.payload;
      if (next.sounds && next.state !== last) {
        if (next.state === "listening") cue("start");
        else if (next.state === "done") cue("done");
        else if (next.state === "error") cue("error");
      } else if (next.sounds && next.state === "listening") {
        // The microphone had to start: the start cue comes once it really hears.
        cue("start");
      }
      last = next.state;
      setPill(next);
    });
    return () => {
      void off.then((f) => f());
    };
  }, []);

  if (pill.state === "hidden") return null;
  const listening = pill.state === "listening";
  const busy = pill.state === "transcribing" || pill.state === "polishing";
  const act = (action: "cancel" | "finish") => void invoke("pill_action", { action });

  // Results and errors: a short message in the same capsule.
  if (!listening && !busy) {
    return (
      <div className="capsule-row" role="status" aria-live="polite">
        <div className={`capsule message ${pill.state}`}>
          {pill.state === "done" && <span className="tick">{"✓"}</span>}
          <span>{pill.message}</span>
        </div>
      </div>
    );
  }

  return (
    <div className="capsule-row" role="status" aria-live="polite" aria-label={listening ? "Listening" : "Working on your text"}>
      {listening && (
        <button className="round cancel" title="Cancel (Esc)" aria-label="Cancel" onClick={() => act("cancel")}>
          {"✕"}
        </button>
      )}
      <div className={`capsule ${busy ? "working" : ""}`}>
        <Wave level={pill.level} busy={busy} />
        {listening && pill.message && <span className="warn">{pill.message}</span>}
      </div>
      {listening && (
        <button className="round finish" title="Finish" aria-label="Finish" onClick={() => act("finish")}>
          {"✓"}
        </button>
      )}
    </div>
  );
}

createRoot(document.getElementById("root")!).render(<Pill />);

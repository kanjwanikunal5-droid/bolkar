import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { api, errorText, meetings, type Check, type Devices, type MeetingLive, type MeetingRow, type MeetingView, type Point, type Segment, type Summary } from "./api";
import { Icon } from "./icons";
import { EmptyState, PageHead, Segmented, SettingRow, Switch, useToast } from "./ui";

function clock(s: number) {
  const h = Math.floor(s / 3600);
  const m = Math.floor(s / 60) % 60;
  const sec = s % 60;
  return `${h > 0 ? h + ":" : ""}${String(m).padStart(2, "0")}:${String(sec).padStart(2, "0")}`;
}

const stamp = (ms: number) => clock(Math.floor(ms / 1000));

function when(iso: string) {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? "" : d.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}

function Level({ label, value, on, device }: { label: string; value: number; on: boolean; device: string }) {
  const pct = Math.min(100, Math.round(Math.sqrt(value) * 220));
  return (
    <div className="field">
      <span>
        {label}
        {on && device && <span className="dim"> · {device}</span>}
      </span>
      <span className="row">
        <meter min={0} max={100} value={on ? pct : 0} aria-label={`${label} level`} style={{ width: "12rem", height: "1.2rem" }} />
        <span className="dim">{on ? "" : "not recorded"}</span>
      </span>
    </div>
  );
}

/* ---------- Notes (rendered from the checked summary, not from Markdown) ---------- */

function CheckMark({ check, note }: { check?: Check; note?: string | null }) {
  if (check === "needs_review")
    return (
      <span className="badge amber" title={note ?? "Something did not match the transcript"} style={{ marginLeft: "0.35rem" }}>
        Check this
      </span>
    );
  if (check === "quoted")
    return (
      <span className="badge ok" title="The supporting words were found in the transcript" style={{ marginLeft: "0.35rem" }}>
        Quote found
      </span>
    );
  return null;
}

function Cites({ ids, at, jump }: { ids: string[]; at: Map<string, number>; jump: (id: string) => void }) {
  const known = ids.filter((id) => at.has(id));
  if (!known.length) return null;
  return (
    <span className="dim" style={{ marginLeft: "0.35rem", fontSize: "0.8em" }}>
      {known.map((id) => (
        <button key={id} className="btn ghost small" style={{ padding: "0 0.25rem", minHeight: 0 }} title="Show in the transcript" onClick={() => jump(id)}>
          [{stamp(at.get(id) ?? 0)}]
        </button>
      ))}
    </span>
  );
}

function PointList({ items, at, jump }: { items: Point[]; at: Map<string, number>; jump: (id: string) => void }) {
  return (
    <ul style={{ marginTop: "0.3rem" }}>
      {items.map((p, i) => (
        <li key={i}>
          {p.text}
          <Cites ids={p.evidence} at={at} jump={jump} />
          <CheckMark check={p.check} note={p.note} />
        </li>
      ))}
    </ul>
  );
}

function Section({ title, children }: { title: string; children: ReactNode }) {
  return (
    <section style={{ marginTop: "1rem" }}>
      <h3 style={{ marginBottom: "0.3rem" }}>{title}</h3>
      {children}
    </section>
  );
}

function SummaryView({ s, segments, jump }: { s: Summary; segments: Segment[]; jump: (id: string) => void }) {
  const at = useMemo(() => new Map(segments.map((x) => [x.id, x.start_ms])), [segments]);
  return (
    <div>
      <p className="dim" style={{ fontSize: "0.75rem" }}>
        Times link each item to the transcript. “Quote found”: its exact words are in the transcript. “Check this”: a number, quote or word did not match.
        Links, quotes, numbers, names and dates are checked automatically; the meaning is not, so read important items against the transcript.
      </p>
      {s.overview.purpose && <p>{s.overview.purpose}</p>}
      {s.executive_summary.length > 0 && (
        <Section title="Summary">
          <PointList items={s.executive_summary} at={at} jump={jump} />
        </Section>
      )}
      {s.topics.length > 0 && (
        <Section title="Discussion">
          {s.topics.map((t, i) => (
            <div key={i} style={{ marginBottom: "0.8rem" }}>
              <h4 style={{ margin: "0.6rem 0 0.2rem" }}>{t.title}</h4>
              {t.narrative && (
                <p style={{ margin: "0.2rem 0" }}>
                  {t.narrative.text}
                  <Cites ids={t.narrative.evidence} at={at} jump={jump} />
                  <CheckMark check={t.narrative.check} note={t.narrative.note} />
                </p>
              )}
              {t.points.length > 0 && <PointList items={t.points} at={at} jump={jump} />}
            </div>
          ))}
        </Section>
      )}
      {s.decisions.length > 0 && (
        <Section title="Decisions">
          <ul>
            {s.decisions.map((d, i) => (
              <li key={i}>
                <span className={`badge ${d.status === "confirmed" ? "ok" : ""}`} style={{ marginRight: "0.4rem" }}>
                  {d.status === "confirmed" ? "Confirmed" : "Proposed"}
                </span>
                {d.text}
                <Cites ids={d.evidence} at={at} jump={jump} />
                <CheckMark check={d.check} note={d.note} />
              </li>
            ))}
          </ul>
        </Section>
      )}
      {s.action_items.length > 0 && (
        <Section title="Action items">
          <div className="table" role="table" aria-label="Action items">
            <div className="row" role="row" style={{ fontWeight: 650, padding: "0.25rem 0" }}>
              <span style={{ flex: 3 }}>Action</span>
              <span style={{ flex: 1 }}>Owner</span>
              <span style={{ flex: 1 }}>Deadline</span>
            </div>
            {s.action_items.map((a, i) => (
              <div key={i} className="row" role="row" style={{ borderTop: "1px solid var(--border)", padding: "0.3rem 0", alignItems: "flex-start" }}>
                <span style={{ flex: 3 }}>
                  {a.action}
                  <Cites ids={a.evidence} at={at} jump={jump} />
                  <CheckMark check={a.check} note={a.note} />
                </span>
                <span style={{ flex: 1 }} className={a.owner ? "" : "dim"}>
                  {a.owner ?? "Not specified"}
                </span>
                <span style={{ flex: 1 }} className={a.deadline ? "" : "dim"}>
                  {a.deadline ? `${a.deadline}${a.deadline_tentative ? " (tentative)" : ""}` : "Not specified"}
                </span>
              </div>
            ))}
          </div>
        </Section>
      )}
      {s.risks.length > 0 && (
        <Section title="Risks and challenges">
          <PointList items={s.risks} at={at} jump={jump} />
        </Section>
      )}
      {s.open_questions.length > 0 && (
        <Section title="Open questions">
          <PointList items={s.open_questions} at={at} jump={jump} />
        </Section>
      )}
      {s.numbers.length > 0 && (
        <Section title="Important numbers">
          <ul>
            {s.numbers.map((n, i) => (
              <li key={i}>
                <strong>{n.value}</strong> {n.context}
                <Cites ids={n.evidence} at={at} jump={jump} />
              </li>
            ))}
          </ul>
        </Section>
      )}
      {s.next_steps.length > 0 && (
        <Section title="Next steps">
          <PointList items={s.next_steps} at={at} jump={jump} />
        </Section>
      )}
      {s.takeaway && (
        <Section title="Final takeaway">
          <p>
            {s.takeaway.text}
            <Cites ids={s.takeaway.evidence} at={at} jump={jump} />
            <CheckMark check={s.takeaway.check} note={s.takeaway.note} />
          </p>
        </Section>
      )}
    </div>
  );
}

/* ---------- One saved meeting ---------- */

function TranscriptLine({ seg, onSave }: { seg: MeetingView["segments"][number]; onSave: (text: string) => Promise<void> }) {
  const [editing, setEditing] = useState(false);
  const [text, setText] = useState(seg.text);
  useEffect(() => setText(seg.text), [seg.text]);
  return (
    <div id={`seg-${seg.id}`} className="timeline-item" style={{ alignItems: "flex-start" }}>
      <span className="time">{stamp(seg.start_ms)}</span>
      <div style={{ minWidth: 0, flex: 1 }}>
        <div className="meta">
          {seg.speaker}
          {seg.edited && (
            <span className="badge accent" style={{ marginLeft: "0.4rem" }} title={`Recognised as: ${seg.original}`}>
              Corrected
            </span>
          )}
        </div>
        {editing ? (
          <div className="row" style={{ marginTop: "0.2rem" }}>
            <input type="text" aria-label="Correct this line" value={text} onChange={(e) => setText(e.target.value)} style={{ flex: 1 }} />
            <button
              className="btn small primary"
              onClick={() =>
                void onSave(text).then(() => {
                  setEditing(false);
                })
              }
            >
              Save
            </button>
            <button className="btn small ghost" onClick={() => setEditing(false)}>
              Cancel
            </button>
          </div>
        ) : (
          <p className="text" style={{ margin: "0.1rem 0" }}>
            {seg.text}
          </p>
        )}
      </div>
      {!editing && (
        <button className="btn ghost small" aria-label="Correct this line" title="Correct this line (the original is kept)" onClick={() => setEditing(true)}>
          <Icon.Edit size={15} />
        </button>
      )}
    </div>
  );
}

function MeetingDetail({ view, setView, onBack, onDeleted }: { view: MeetingView; setView: (v: MeetingView) => void; onBack: () => void; onDeleted: () => void }) {
  const toast = useToast();
  const [title, setTitle] = useState(view.title);
  const [busy, setBusy] = useState<string | null>(null);
  const [confirmDelete, setConfirmDelete] = useState(false);
  useEffect(() => setTitle(view.title), [view.title]);
  const jump = useCallback((id: string) => document.getElementById(`seg-${id}`)?.scrollIntoView({ behavior: "smooth", block: "center" }), []);
  const run = (label: string, p: Promise<MeetingView>, done: string) => {
    setBusy(label);
    void p
      .then(
        (v) => {
          setView(v);
          toast(done);
        },
        (e) => toast(errorText(e), true),
      )
      .finally(() => setBusy(null));
  };
  const exportAs = (format: "md" | "txt") =>
    void meetings.exportNotes(view.id, format).then(
      (path) => {
        toast(`Saved in Downloads: ${path.split(/[\\/]/).pop()}`);
        void api.revealFile(path);
      },
      (e) => toast(errorText(e), true),
    );
  return (
    <>
      <div className="row" style={{ marginBottom: "0.6rem" }}>
        <button className="btn ghost small" onClick={onBack}>
          ← Past meetings
        </button>
      </div>
      <div className="card">
        <div className="row" style={{ flexWrap: "wrap" }}>
          <input type="text" aria-label="Meeting title" value={title} onChange={(e) => setTitle(e.target.value)} style={{ flex: 1, minWidth: "14rem", fontWeight: 650 }} />
          {title.trim() && title.trim() !== view.title && (
            <button className="btn small primary" onClick={() => run("rename", meetings.rename(view.id, title), "Title saved")}>
              Save title
            </button>
          )}
        </div>
        <p className="dim" style={{ margin: "0.4rem 0" }}>
          {when(view.started_at)} · {clock(Math.round(view.duration_ms / 1000))} · {view.segments.length} transcript lines
          {view.has_audio ? " · audio kept" : " · audio deleted"}
        </p>
        <div className="row" style={{ flexWrap: "wrap", gap: "0.4rem" }}>
          <button className="btn small" onClick={() => void api.copy(view.markdown).then(() => toast("Notes copied"))}>
            <Icon.Copy size={15} /> Copy notes
          </button>
          <button className="btn small" onClick={() => exportAs("md")}>
            <Icon.Download size={15} /> Save as Markdown
          </button>
          <button className="btn small" onClick={() => exportAs("txt")}>
            <Icon.Download size={15} /> Save as text
          </button>
          {view.segments.length > 0 && (
            <button className="btn small" disabled={!!busy} onClick={() => run("notes", meetings.rewriteNotes(view.id), "Notes written again")}>
              <Icon.Sparkles size={15} /> {busy === "notes" ? "Writing notes..." : "Write notes again"}
            </button>
          )}
          {view.has_audio && (
            <button className="btn small" disabled={!!busy} onClick={() => run("transcribe", meetings.transcribeAgain(view.id), "Transcribed")}>
              <Icon.Refresh size={15} /> {busy === "transcribe" ? "Transcribing..." : view.status === "interrupted" ? "Transcribe recording" : "Transcribe again"}
            </button>
          )}
          {!confirmDelete ? (
            <button className="btn ghost small danger" onClick={() => setConfirmDelete(true)}>
              <Icon.Trash size={15} /> Delete
            </button>
          ) : (
            <span className="row" role="alert">
              <span>Delete this meeting{view.has_audio ? ", its audio," : ""} its transcript and notes?</span>
              <button
                className="btn small danger"
                onClick={() =>
                  void meetings.remove(view.id).then(
                    () => {
                      toast("Meeting deleted");
                      onDeleted();
                    },
                    (e) => toast(errorText(e), true),
                  )
                }
              >
                Delete
              </button>
              <button className="btn small ghost" onClick={() => setConfirmDelete(false)}>
                Keep
              </button>
            </span>
          )}
        </div>
        {view.note && (
          <p className="dim" role="status">
            {view.note}
          </p>
        )}
        {view.problems.map((p) => (
          <div className="banner" role="alert" key={p}>
            {p}
          </div>
        ))}
        {view.flags.length > 0 && (
          <details>
            <summary className="dim">{view.flags.length} item(s) marked “Check this”</summary>
            {view.flags.map((d, i) => (
              <p className="dim" key={i}>
                {d}
              </p>
            ))}
          </details>
        )}
        {view.dropped.length > 0 && (
          <details>
            <summary className="dim">{view.dropped.length} item(s) removed or blanked by the checks</summary>
            {view.dropped.map((d, i) => (
              <p className="dim" key={i}>
                {d}
              </p>
            ))}
          </details>
        )}
        {view.summary && <SummaryView s={view.summary} segments={view.segments} jump={jump} />}
      </div>
      {view.thoughts.length > 0 && (
        <div className="card">
          <h3 style={{ marginTop: 0 }}>My thoughts</h3>
          {view.thoughts.map(([ms, t], i) => (
            <p key={i}>
              <span className="dim">[{stamp(ms)}]</span> {t}
            </p>
          ))}
        </div>
      )}
      <div className="card">
        <h3 style={{ marginTop: 0 }}>Transcript</h3>
        <p className="dim" style={{ marginTop: 0, fontSize: "0.75rem" }}>
          “Microphone” and “System audio” are the two sound sources, not people. Corrections are kept next to the original recognised text.
        </p>
        {view.segments.length === 0 ? (
          <p className="dim">No transcript yet.</p>
        ) : (
          view.segments.map((s) => (
            <TranscriptLine
              key={s.id}
              seg={s}
              onSave={(text) =>
                meetings.editLine(view.id, s.id, text).then(
                  (v) => {
                    setView(v);
                    toast("Line corrected");
                  },
                  (e) => toast(errorText(e), true),
                )
              }
            />
          ))
        )}
      </div>
    </>
  );
}

/* ---------- Past meetings ---------- */

function Library({ open }: { open: (id: string) => void }) {
  const toast = useToast();
  const [query, setQuery] = useState("");
  const [rows, setRows] = useState<MeetingRow[] | null>(null);
  const [confirmAll, setConfirmAll] = useState(false);
  const load = useCallback(() => void meetings.list(query).then(setRows, (e) => toast(errorText(e), true)), [query, toast]);
  useEffect(() => {
    const t = setTimeout(load, 200);
    return () => clearTimeout(t);
  }, [load]);
  return (
    <div className="card">
      <div className="row" style={{ marginBottom: "0.6rem" }}>
        <Icon.Search size={16} />
        <input type="search" aria-label="Search meetings" placeholder="Search titles, transcripts and notes" value={query} onChange={(e) => setQuery(e.target.value)} style={{ flex: 1 }} />
      </div>
      {rows === null ? (
        <p className="dim">Loading...</p>
      ) : rows.length === 0 ? (
        <EmptyState icon={<Icon.Meeting size={36} />} title={query ? "No meeting matches" : "No meetings yet"}>
          {query ? "Try another word." : "Recorded meetings appear here with their transcript and notes."}
        </EmptyState>
      ) : (
        rows.map((m) => (
          <button key={m.id} className="check-item" onClick={() => open(m.id)} style={{ width: "100%" }}>
            <span style={{ flex: 1, textAlign: "left" }}>
              <strong style={{ display: "block" }}>{m.title || "Meeting"}</strong>
              <span className="dim" style={{ fontSize: "0.75rem" }}>
                {when(m.started_at)} · {clock(Math.round(m.duration_ms / 1000))} · {m.segments} lines
              </span>
            </span>
            {m.status === "interrupted" && <span className="badge amber">Interrupted</span>}
            {m.has_summary && <span className="badge ok">Notes</span>}
            {!m.has_audio && <span className="badge">No audio</span>}
            <span className="chev">
              <Icon.Chevron size={16} />
            </span>
          </button>
        ))
      )}
      {rows && rows.length > 0 && !query && (
        <div className="row" style={{ marginTop: "0.8rem" }}>
          {!confirmAll ? (
            <button className="btn ghost small danger" onClick={() => setConfirmAll(true)}>
              <Icon.Trash size={15} /> Delete all meetings
            </button>
          ) : (
            <span className="row" role="alert">
              <span>Delete all {rows.length} meetings with their audio, transcripts and notes? This cannot be undone.</span>
              <button
                className="btn small danger"
                onClick={() =>
                  void meetings
                    .removeAll()
                    .then(
                      (n) => toast(`${n} meeting(s) deleted`),
                      (e) => toast(errorText(e), true),
                    )
                    .finally(() => {
                      setConfirmAll(false);
                      load();
                    })
                }
              >
                Delete all
              </button>
              <button className="btn small ghost" onClick={() => setConfirmAll(false)}>
                Keep
              </button>
            </span>
          )}
        </div>
      )}
    </div>
  );
}

/* ---------- Recording ---------- */

function DevicePick({ label, value, onChange, names, fallback }: { label: string; value: string; onChange: (v: string) => void; names: string[]; fallback: string | null }) {
  return (
    <select aria-label={label} value={names.includes(value) ? value : ""} onChange={(e) => onChange(e.target.value)} style={{ maxWidth: "18rem" }}>
      <option value="">Windows default{fallback ? ` (${fallback})` : ""}</option>
      {names.map((n) => (
        <option key={n} value={n}>
          {n}
        </option>
      ))}
    </select>
  );
}

/** Kept outside the component so notes survive switching pages while recording. */
let keptThoughts: [number, string][] = [];

function Recorder({ onSaved }: { onSaved: (v: MeetingView) => void }) {
  const [title, setTitle] = useState("");
  const [mic, setMic] = useState(true);
  const [system, setSystem] = useState(true);
  const [consent, setConsent] = useState(false);
  const [devices, setDevices] = useState<Devices | null>(null);
  const [micDevice, setMicDevice] = useState("");
  const [outDevice, setOutDevice] = useState("");
  const [live, setLive] = useState<MeetingLive | null>(null);
  const [thought, setThought] = useState("");
  const [thoughts, setThoughtsState] = useState<[number, string][]>(keptThoughts);
  const setThoughts = (next: [number, string][] | ((t: [number, string][]) => [number, string][])) =>
    setThoughtsState((cur) => {
      keptThoughts = typeof next === "function" ? next(cur) : next;
      return keptThoughts;
    });
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const seconds = useRef(0);

  useEffect(() => {
    void meetings.devices().then(setDevices, () => setDevices(null));
    void api.settings().then((s) => {
      setMicDevice(s.meeting_mic_device ?? "");
      setOutDevice(s.meeting_output_device ?? "");
    });
  }, []);
  useEffect(() => {
    const t = setInterval(() => {
      void meetings.status().then((s) => {
        setLive(s);
        if (s) seconds.current = s.seconds;
      });
    }, 1000);
    return () => clearInterval(t);
  }, []);

  const recording = !!live?.recording;
  const start = () => {
    setError(null);
    setThoughts([]);
    void api.setSetting("meeting_mic_device", micDevice);
    void api.setSetting("meeting_output_device", outDevice);
    void meetings.start(mic, system, consent, micDevice, outDevice).then(() => meetings.status().then(setLive), (e) => setError(errorText(e)));
  };
  const stop = () => {
    setBusy(true);
    setError(null);
    void meetings
      .stop(title, thoughts)
      .then(
        (v) => {
          setThoughts([]);
          onSaved(v);
        },
        (e) => setError(errorText(e)),
      )
      .finally(() => {
        setBusy(false);
        setConsent(false);
        setTitle("");
      });
  };
  const addThought = () => {
    if (!thought.trim()) return;
    setThoughts((t) => [...t, [seconds.current * 1000, thought.trim()]]);
    setThought("");
  };
  const lines = live?.live.segments ?? [];

  return (
    <>
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
          <SettingRow title="Microphone" help="Your voice, and anyone else in the room.">
            <span className="row">
              <DevicePick label="Microphone device" value={micDevice} onChange={setMicDevice} names={devices?.inputs ?? []} fallback={devices?.default_input ?? null} />
              <Switch label="Record the microphone" checked={mic} onChange={setMic} />
            </span>
          </SettingRow>
          <SettingRow title="System audio" help="Everything the computer plays: Zoom, Google Meet, Teams. Use headphones so it is not picked up twice.">
            <span className="row">
              <DevicePick label="Speakers or headphones" value={outDevice} onChange={setOutDevice} names={devices?.outputs ?? []} fallback={devices?.default_output ?? null} />
              <Switch label="Record system audio" checked={system} onChange={setSystem} />
            </span>
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
              {p}
            </div>
          ))}
          <Level label="Microphone" value={live?.mic_level ?? 0} on={mic} device={live?.mic_device ?? ""} />
          <Level label="System audio" value={live?.system_level ?? 0} on={system} device={live?.system_device ?? ""} />

          <h3 style={{ marginTop: "1rem" }}>Transcript so far</h3>
          <p className="dim" style={{ marginTop: 0 }}>
            {live?.live.mode === "after"
              ? "No graphics card in use: the transcript is made when you stop, so the computer stays fast during the call."
              : live?.live.mode === "live"
                ? "New lines appear about every 30 seconds. They are provisional and tidied when you stop."
                : "The first lines appear after about 30 seconds of speech."}
          </p>
          {live?.live.error && (
            <p className="dim" role="status">
              {live.live.error}
            </p>
          )}
          <div style={{ maxHeight: "16rem", overflowY: "auto" }} aria-live="polite">
            {lines.map((s, i) => (
              <p key={i} style={{ margin: "0.2rem 0" }}>
                <span className="dim">
                  [{stamp(s.start_ms)}] {s.speaker}:
                </span>{" "}
                {s.text}
              </p>
            ))}
          </div>

          <h3 style={{ marginTop: "1rem" }}>My thoughts</h3>
          <p className="dim">Your own notes. They are kept exactly as you write them and never changed by the AI.</p>
          <div className="row">
            <input type="text" aria-label="Add a note" style={{ flex: 1 }} value={thought} onChange={(e) => setThought(e.target.value)} onKeyDown={(e) => e.key === "Enter" && addThought()} />
            <button className="btn" onClick={addThought}>
              Add note
            </button>
          </div>
          {thoughts.map(([ms, t], i) => (
            <p key={i}>
              <span className="dim">[{stamp(ms)}]</span> {t}
            </p>
          ))}
        </div>
      )}

      {busy && (
        <div className="card" role="status">
          Finishing the transcript on this computer and writing the notes. Long meetings take longer; the transcript is saved even if the notes fail.
        </div>
      )}
    </>
  );
}

export function MeetingsPage() {
  const toast = useToast();
  const [tab, setTab] = useState<"record" | "past">("record");
  const [view, setView] = useState<MeetingView | null>(null);
  const open = (id: string) => void meetings.get(id).then(setView, (e) => toast(errorText(e), true));
  return (
    <>
      <PageHead
        title="Meeting notes"
        sub="Records your microphone and the system audio, transcribes on this computer, and writes notes with decisions and action items."
      />
      {!view && (
        <div style={{ marginBottom: "0.8rem" }}>
          <Segmented<"record" | "past">
            label="Meeting notes view"
            value={tab}
            onChange={setTab}
            options={[
              ["record", "Record"],
              ["past", "Past meetings"],
            ]}
          />
        </div>
      )}
      {/* The recorder stays mounted (hidden) so a recording keeps its screen state. */}
      <div hidden={!!view || tab !== "record"}>
        <Recorder onSaved={setView} />
      </div>
      {view ? (
        <MeetingDetail
          view={view}
          setView={setView}
          onBack={() => {
            setView(null);
            setTab("past");
          }}
          onDeleted={() => {
            setView(null);
            setTab("past");
          }}
        />
      ) : (
        tab === "past" && <Library open={open} />
      )}
    </>
  );
}

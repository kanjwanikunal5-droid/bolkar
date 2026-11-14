import { useEffect, useMemo, useState } from "react";
import { api, type Dictation } from "./api";
import { Icon } from "./icons";
import { FixBox } from "./Pages";
import { EmptyState, PageHead, Segmented, appName, dayLabel, timeOf, useToast } from "./ui";

function Row({ d, onChanged, startFixing }: { d: Dictation; onChanged: () => void; startFixing: boolean }) {
  const toast = useToast();
  const [fixing, setFixing] = useState(startFixing);
  const [raw, setRaw] = useState(false);
  useEffect(() => setFixing(startFixing), [startFixing]);
  const ai = d.cleanup && !d.cleanup.startsWith("rules");
  return (
    <div className="timeline-item" style={{ gridTemplateColumns: "4.2rem minmax(0,1fr) auto" }}>
      <span className="time">{timeOf(d.created_at)}</span>
      <div style={{ minWidth: 0 }}>
        {fixing ? (
          <FixBox
            id={d.id}
            text={d.final_text}
            onDone={() => {
              setFixing(false);
              onChanged();
            }}
          />
        ) : (
          <p style={{ margin: 0, whiteSpace: "pre-wrap", overflowWrap: "anywhere" }}>{d.final_text || <span className="dim">(nothing heard)</span>}</p>
        )}
        {raw && (
          <p className="raw" style={{ marginTop: "0.4rem" }}>
            <span className="dim">Heard: </span>
            {d.raw_text}
          </p>
        )}
        <div className="row" style={{ gap: "0.4rem", marginTop: "0.35rem" }}>
          <span className="badge">{appName(d.app_process) || "No app"}</span>
          {d.level === "COMMAND" && <span className="badge accent">Command</span>}
          {d.inserted ? <span className="badge ok">Typed</span> : <span className="badge">{d.insert_method || "Not typed"}</span>}
          {ai && <span className="badge accent">AI clean-up</span>}
          <span className="faint" style={{ fontSize: "0.68rem" }} title={d.engine}>
            {d.word_count} words
          </span>
        </div>
        {d.error && <p className="bad">{d.error}</p>}
      </div>
      <span className="actions row" style={{ gap: "0.15rem", alignSelf: "start" }}>
        <button className="btn ghost small" title="Copy" aria-label="Copy" onClick={() => void api.copy(d.final_text).then(() => toast("Copied"))}>
          <Icon.Copy size={16} />
        </button>
        <button className="btn ghost small" title="Fix" aria-label="Fix" onClick={() => setFixing(true)}>
          <Icon.Edit size={16} />
        </button>
        <button className="btn ghost small" title="What the recogniser heard" aria-label="Show original" aria-pressed={raw} onClick={() => setRaw((v) => !v)}>
          <Icon.Info size={16} />
        </button>
        <button
          className="btn ghost small danger"
          title="Delete"
          aria-label="Delete"
          onClick={() => void api.deleteDictation(d.id).then(() => {
            toast("Deleted");
            onChanged();
          })}
        >
          <Icon.Trash size={16} />
        </button>
      </span>
    </div>
  );
}

type Filter = "all" | "typed" | "commands";

export function HistoryPage({ dictations, fixId, onChanged }: { dictations: Dictation[]; fixId: string | null; onChanged: () => void }) {
  const [q, setQ] = useState("");
  const [filter, setFilter] = useState<Filter>("all");
  const shown = useMemo(() => {
    const s = q.trim().toLowerCase();
    return dictations.filter(
      (d) =>
        (filter === "all" || (filter === "typed" ? d.inserted : d.level === "COMMAND")) &&
        (!s || d.final_text.toLowerCase().includes(s) || d.raw_text.toLowerCase().includes(s) || d.app_process.toLowerCase().includes(s)),
    );
  }, [dictations, q, filter]);

  return (
    <div className="page">
      <PageHead title="History" sub="Every dictation, kept on this computer. Your original words are stored apart from the cleaned text.">
        <Segmented<Filter>
          label="Show"
          value={filter}
          onChange={setFilter}
          options={[
            ["all", "All"],
            ["typed", "Typed"],
            ["commands", "Commands"],
          ]}
        />
      </PageHead>
      <div className="card">
        <div className="row" style={{ marginBottom: "0.5rem" }}>
          <span className="dim">
            <Icon.Search size={18} />
          </span>
          <input type="search" aria-label="Search history" placeholder="Search your dictations" value={q} onChange={(e) => setQ(e.target.value)} style={{ flex: 1 }} />
        </div>
        {shown.length === 0 ? (
          <EmptyState icon={<Icon.Clock size={36} />} title={dictations.length ? "No matches" : "Nothing dictated yet"}>
            {dictations.length ? "Try other words." : "Your dictations will appear here."}
          </EmptyState>
        ) : (
          shown.map((d, i) => (
            <div key={d.id}>
              {(i === 0 || dayLabel(d.created_at) !== dayLabel(shown[i - 1].created_at)) && <div className="day-head">{dayLabel(d.created_at)}</div>}
              <Row d={d} onChanged={onChanged} startFixing={d.id === fixId} />
            </div>
          ))
        )}
      </div>
    </div>
  );
}

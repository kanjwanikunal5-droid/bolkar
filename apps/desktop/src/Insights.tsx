import type { Insights } from "./api";
import { Icon } from "./icons";
import { CountUp, EmptyState, PageHead } from "./ui";

/** Half-circle gauge: your speaking speed, with a marker at typing speed. */
function SpeedGauge({ wpm, typing }: { wpm: number | null; typing: number }) {
  const max = Math.max(160, Math.ceil(((wpm ?? 0) * 1.15) / 20) * 20);
  const r = 70;
  const arc = (v: number) => {
    const a = Math.PI * (1 - Math.min(v, max) / max);
    return [90 + r * Math.cos(a), 90 - r * Math.sin(a)];
  };
  const [ex, ey] = arc(wpm ?? 0);
  const [tx, ty] = arc(typing);
  const large = 0;
  return (
    <svg viewBox="0 0 180 104" width="100%" style={{ maxWidth: "15rem" }} role="img" aria-label={`Speaking speed ${Math.round(wpm ?? 0)} words per minute`}>
      <path d={`M20 90 A${r} ${r} 0 0 1 160 90`} fill="none" stroke="var(--surface-3)" strokeWidth="14" strokeLinecap="round" />
      {wpm != null && wpm > 0 && (
        <path
          d={`M20 90 A${r} ${r} 0 ${large} 1 ${ex.toFixed(1)} ${ey.toFixed(1)}`}
          fill="none"
          stroke="var(--accent-fill)"
          strokeWidth="14"
          strokeLinecap="round"
        />
      )}
      <line x1={tx} y1={ty - 0} x2={90 + (tx - 90) * 0.72} y2={90 + (ty - 90) * 0.72} stroke="var(--amber-fill)" strokeWidth="3" strokeLinecap="round" />
    </svg>
  );
}

function Calendar({ days }: { days: Insights["days"] }) {
  const max = Math.max(1, ...days.map((d) => d.words));
  const level = (w: number) => (w === 0 ? 0 : Math.min(4, 1 + Math.floor((w / max) * 3.999)));
  // Pad the start so columns are whole weeks starting on Monday.
  const first = new Date(days[0].date + "T00:00:00");
  const pad = (first.getDay() + 6) % 7;
  const today = days[days.length - 1].date;
  const months: { col: number; label: string }[] = [];
  days.forEach((d, i) => {
    const date = new Date(d.date + "T00:00:00");
    if (date.getDate() === 1 || i === 0) {
      months.push({ col: Math.floor((i + pad) / 7), label: date.toLocaleDateString([], { month: "short" }) });
    }
  });
  const cols = Math.ceil((days.length + pad) / 7);
  return (
    <div>
      <div style={{ display: "grid", gridTemplateColumns: `repeat(${cols}, 1fr)`, gap: 4, fontSize: "0.64rem", marginBottom: 4 }} className="dim">
        {Array.from({ length: cols }, (_, c) => (
          <span key={c}>{months.find((m) => m.col === c)?.label ?? ""}</span>
        ))}
      </div>
      <div className="heatmap" style={{ gridTemplateColumns: `repeat(${cols}, 1fr)` }}>
        {Array.from({ length: pad }, (_, i) => (
          <span key={`p${i}`} className="cell empty" />
        ))}
        {days.map((d) => (
          <span
            key={d.date}
            className={`cell ${d.date === today ? "today" : ""}`}
            data-level={level(d.words)}
            title={`${new Date(d.date + "T00:00:00").toLocaleDateString([], { weekday: "short", day: "numeric", month: "short" })}: ${
              d.words
            } words, ${d.dictations} dictation${d.dictations === 1 ? "" : "s"}`}
          />
        ))}
      </div>
      <div className="row spread" style={{ marginTop: "0.6rem" }}>
        <span className="legend">
          Less
          {[0, 1, 2, 3, 4].map((l) => (
            <i key={l} style={{ background: `var(--heat-${l})` }} />
          ))}
          More
        </span>
        <span className="legend">
          <i style={{ outline: "2px solid var(--amber-fill)", background: "transparent" }} /> Today
        </span>
      </div>
    </div>
  );
}

function fmtMinutes(m: number) {
  if (m < 1) return { value: Math.round(m * 60), unit: "s" };
  if (m < 60) return { value: Math.round(m), unit: "min" };
  return { value: Math.round((m / 60) * 10) / 10, unit: "h" };
}

export function InsightsPage({ data }: { data: Insights | null }) {
  if (!data) {
    return (
      <div className="page">
        <PageHead title="Insights" />
        <div className="grid cols-3">
          {[0, 1, 2].map((i) => (
            <div key={i} className="card skeleton" style={{ height: "9rem" }} />
          ))}
        </div>
      </div>
    );
  }
  const saved = fmtMinutes(data.minutes_saved);
  const pages = data.total_words / 250;
  const faster = data.speaking_wpm ? data.speaking_wpm / data.typing_wpm : null;
  return (
    <div className="page">
      <PageHead
        title="Insights"
        sub={
          data.first_day
            ? `Everything counted from your own history on this computer since ${new Date(data.first_day + "T00:00:00").toLocaleDateString([], {
                day: "numeric",
                month: "long",
                year: "numeric",
              })}.`
            : "Your numbers appear here after your first dictation. Counted on this computer only."
        }
      />

      <div className="grid cols-3" style={{ marginBottom: "1rem" }}>
        <div className="card lift">
          <div className="row spread" style={{ alignItems: "flex-start" }}>
            <div className="stat">
              <span className="stat-icon">
                <Icon.Gauge size={18} />
              </span>
              <span className="label">Speaking speed</span>
              <span className="value">
                {data.speaking_wpm ? <CountUp value={Math.round(data.speaking_wpm)} /> : "–"}
                <small>words / min</small>
              </span>
              <span className="sub">
                {faster ? `${faster.toFixed(1)}x typing at ${data.typing_wpm} wpm` : "Dictate to measure it"}
              </span>
            </div>
          </div>
          <SpeedGauge wpm={data.speaking_wpm} typing={data.typing_wpm} />
          <div className="legend" style={{ marginTop: "0.2rem" }}>
            <i style={{ background: "var(--accent-fill)" }} /> You
            <i style={{ background: "var(--amber-fill)", marginLeft: "0.6rem" }} /> Typing ({data.typing_wpm} wpm)
          </div>
        </div>

        <div className="card lift">
          <div className="stat">
            <span className="stat-icon">
              <Icon.Words size={18} />
            </span>
            <span className="label">Words dictated</span>
            <span className="value">
              <CountUp value={data.total_words} />
            </span>
            <span className="sub">
              {data.total_dictations} dictations
              {pages >= 1 ? ` · about ${pages.toFixed(pages < 10 ? 1 : 0)} pages` : ""}
            </span>
          </div>
          <div className="row" style={{ marginTop: "0.9rem", gap: "1.4rem" }}>
            <div className="stat">
              <span className="label">Today</span>
              <strong>
                <CountUp value={data.words_today} />
              </strong>
            </div>
            <div className="stat">
              <span className="label">Last 7 days</span>
              <strong>
                <CountUp value={data.words_this_week} />
              </strong>
            </div>
          </div>
        </div>

        <div className="card lift">
          <div className="stat">
            <span className="stat-icon amber">
              <Icon.Hourglass size={18} />
            </span>
            <span className="label">Time saved</span>
            <span className="value">
              <CountUp value={saved.value} decimals={saved.unit === "h" ? 1 : 0} />
              <small>{saved.unit}</small>
            </span>
            <span className="sub">Estimate: the same words typed at {data.typing_wpm} words per minute, minus your speaking time.</span>
          </div>
        </div>
      </div>

      <div className="grid cols-2" style={{ marginBottom: "1rem" }}>
        <div className="card">
          <div className="card-title">
            <h2>Activity</h2>
            <span className="row" style={{ gap: "0.5rem" }}>
              <span className="badge amber">
                <Icon.Flame size={13} /> {data.current_streak}-day streak
              </span>
              <span className="badge">Best {data.longest_streak}</span>
            </span>
          </div>
          <Calendar days={data.days} />
        </div>

        <div className="card">
          <div className="card-title">
            <h2>Where you dictate</h2>
            <span className="badge">{data.active_days} active days</span>
          </div>
          {data.apps.length === 0 ? (
            <EmptyState icon={<Icon.Chart size={32} />} title="No dictations yet" />
          ) : (
            data.apps.map((a) => (
              <div className="bar-row" key={a.name}>
                <span style={{ fontWeight: 600, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{a.name}</span>
                <span className="track">
                  <span className="fill" style={{ width: `${Math.max(6, a.share * 100)}%` }}>
                    {Math.round(a.share * 100)}%
                  </span>
                </span>
                <span className="dim" style={{ textAlign: "right", fontSize: "0.72rem" }}>
                  {a.words.toLocaleString()} words
                </span>
              </div>
            ))
          )}
        </div>
      </div>

      <div className="grid cols-3">
        <div className="card lift">
          <div className="stat">
            <span className="stat-icon">
              <Icon.Edit size={18} />
            </span>
            <span className="label">Fixes you made</span>
            <span className="value">
              <CountUp value={data.fixes} />
            </span>
            <span className="sub">Corrections teach the dictionary (Alt + Shift + F).</span>
          </div>
        </div>
        <div className="card lift">
          <div className="stat">
            <span className="stat-icon">
              <Icon.Book size={18} />
            </span>
            <span className="label">Words it learned</span>
            <span className="value">
              <CountUp value={data.learned_words + data.your_words} />
            </span>
            <span className="sub">
              {data.learned_words} from your fixes · {data.your_words} added by you
            </span>
          </div>
        </div>
        <div className="card lift">
          <div className="stat">
            <span className="stat-icon">
              <Icon.Wand size={18} />
            </span>
            <span className="label">Voice edits</span>
            <span className="value">
              <CountUp value={data.commands} />
            </span>
            <span className="sub">Command Mode (Alt + Shift + C).</span>
          </div>
        </div>
      </div>
    </div>
  );
}

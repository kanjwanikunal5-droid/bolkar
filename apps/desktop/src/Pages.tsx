import { useCallback, useEffect, useState } from "react";
import { api, errorText, type AiStatus, type Snippet, type Suggestion, type Term, type UploadEvent } from "./api";

function useLoad<T>(load: () => Promise<T>, initial: T) {
  const [value, setValue] = useState<T>(initial);
  const refresh = useCallback(() => void load().then(setValue), [load]);
  useEffect(refresh, [refresh]);
  return [value, refresh] as const;
}

function Banner({ text }: { text: string | null }) {
  return text ? (
    <div className="banner" role="alert">
      {text}
    </div>
  ) : null;
}

const SOURCE_LABEL: Record<Term["source"], string> = { manual: "added by you", learned: "learned from a fix", pack: "word pack" };

export function DictionaryPage() {
  const [terms, refresh] = useLoad(api.terms, [] as Term[]);
  const [term, setTerm] = useState("");
  const [heard, setHeard] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [filter, setFilter] = useState("");

  const add = () => {
    const variants = heard.split(",").map((v) => v.trim()).filter(Boolean);
    api.addTerm(term, variants).then(
      () => {
        setTerm("");
        setHeard("");
        setError(null);
        refresh();
      },
      (e) => setError(errorText(e)),
    );
  };
  const shown = terms.filter((t) => t.term.toLowerCase().includes(filter.toLowerCase()));

  return (
    <>
      <h2>Dictionary</h2>
      <p className="dim">
        Words written exactly the way you want them. They also help the speech recogniser hear them. A word is only
        replaced when it matches as a whole word; nothing is changed because it merely sounds similar.
      </p>
      <Banner text={error} />
      <div className="card">
        <h3>Add a word</h3>
        <div className="field">
          <label htmlFor="term">Write it as</label>
          <input id="term" type="text" value={term} placeholder="e.g. EBITDA" onChange={(e) => setTerm(e.target.value)} />
        </div>
        <div className="field">
          <label htmlFor="heard">Often heard as (optional)</label>
          <input id="heard" type="text" value={heard} placeholder="e.g. ebit da, e bit da" onChange={(e) => setHeard(e.target.value)} />
        </div>
        <button className="btn primary" disabled={!term.trim()} onClick={add}>
          Add
        </button>
      </div>
      <div className="card">
        <div className="row spread">
          <h3>{terms.length} words</h3>
          <input type="text" aria-label="Search the dictionary" placeholder="Search" value={filter} onChange={(e) => setFilter(e.target.value)} />
        </div>
        {shown.map((t) => (
          <div className="dictation row spread" key={t.id}>
            <span>
              <strong>{t.term}</strong>
              {t.spoken_variants.length > 0 && <span className="dim"> ← {t.spoken_variants.join(", ")}</span>}
              {!t.auto_replace && <span className="dim"> (hint only, never forced)</span>}
            </span>
            <span className="row">
              <span className="tag">{SOURCE_LABEL[t.source]}</span>
              <button className="btn" aria-label={`Delete ${t.term}`} onClick={() => void api.deleteTerm(t.id).then(refresh)}>
                Delete
              </button>
            </span>
          </div>
        ))}
      </div>
    </>
  );
}

export function SnippetsPage() {
  const [snippets, refresh] = useLoad(api.snippets, [] as Snippet[]);
  const [trigger, setTrigger] = useState("");
  const [body, setBody] = useState("");
  const [error, setError] = useState<string | null>(null);
  const add = () =>
    api.addSnippet(trigger, body).then(
      () => {
        setTrigger("");
        setBody("");
        setError(null);
        refresh();
      },
      (e) => setError(errorText(e)),
    );
  return (
    <>
      <h2>Snippets</h2>
      <p className="dim">Say a short phrase and it is replaced by a saved block of text, exactly as saved.</p>
      <Banner text={error} />
      <div className="card">
        <h3>Add a snippet</h3>
        <div className="field">
          <label htmlFor="trigger">When I say</label>
          <input id="trigger" type="text" value={trigger} placeholder="e.g. insert my address" onChange={(e) => setTrigger(e.target.value)} />
        </div>
        <div className="field">
          <label htmlFor="body">Write</label>
          <textarea id="body" className="practice" value={body} onChange={(e) => setBody(e.target.value)} />
        </div>
        <button className="btn primary" disabled={!trigger.trim() || !body.trim()} onClick={() => void add()}>
          Save
        </button>
      </div>
      <div className="card">
        {snippets.length === 0 && <p className="dim">No snippets yet.</p>}
        {snippets.map((s) => (
          <div className="dictation" key={s.id}>
            <div className="row spread">
              <strong>"{s.trigger}"</strong>
              <button className="btn" aria-label={`Delete snippet ${s.trigger}`} onClick={() => void api.deleteSnippet(s.id).then(refresh)}>
                Delete
              </button>
            </div>
            <p className="raw">{s.body}</p>
          </div>
        ))}
      </div>
    </>
  );
}

/** Edit box for one dictation; after saving, offers the words it learned. */
export function FixBox({ id, text, onDone }: { id: string; text: string; onDone: () => void }) {
  const [value, setValue] = useState(text);
  const [suggestions, setSuggestions] = useState<Suggestion[] | null>(null);
  const [chosen, setChosen] = useState<boolean[]>([]);
  const [error, setError] = useState<string | null>(null);

  if (suggestions) {
    if (suggestions.length === 0) {
      return (
        <div>
          <p className="ok">Saved.</p>
          <button className="btn" onClick={onDone}>
            Close
          </button>
        </div>
      );
    }
    return (
      <div>
        <p>Saved. Add these to your dictionary so they are written correctly next time?</p>
        {suggestions.map((s, i) => (
          <label className="row" key={s.heard + s.term}>
            <input type="checkbox" checked={chosen[i]} onChange={(e) => setChosen(chosen.map((c, j) => (j === i ? e.target.checked : c)))} />
            "{s.heard}" → <strong>{s.term}</strong>
          </label>
        ))}
        <div className="row">
          <button
            className="btn primary"
            onClick={() =>
              void api.learnTerms(suggestions.filter((_, i) => chosen[i]).map((s) => [s.heard, s.term])).then(onDone, (e) => setError(errorText(e)))
            }
          >
            Add selected
          </button>
          <button className="btn" onClick={onDone}>
            No thanks
          </button>
        </div>
        <Banner text={error} />
      </div>
    );
  }

  return (
    <div>
      <textarea className="practice" aria-label="Corrected text" value={value} autoFocus onChange={(e) => setValue(e.target.value)} />
      <div className="row">
        <button
          className="btn primary"
          disabled={value === text}
          onClick={() =>
            void api.fixDictation(id, value).then(
              (s) => {
                setSuggestions(s);
                setChosen(s.map(() => true));
              },
              (e) => setError(errorText(e)),
            )
          }
        >
          Save fix
        </button>
        <button className="btn" onClick={onDone}>
          Cancel
        </button>
      </div>
      <Banner text={error} />
    </div>
  );
}

/** Settings card: optional AI clean-up with the user's own key and explicit consent. */
export function AiSettings() {
  const [status, refresh] = useLoad(api.ai, null as AiStatus | null);
  const [provider, setProvider] = useState("none");
  const [model, setModel] = useState("");
  const [baseUrl, setBaseUrl] = useState("");
  const [key, setKey] = useState("");
  const [consent, setConsent] = useState(false);
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    if (!status) return;
    setProvider(status.provider);
    setModel(status.model);
    setBaseUrl(status.base_url);
    setConsent(status.consent);
  }, [status]);

  if (!status) return null;
  const preset = status.presets.find((p) => p.id === provider);
  const save = () =>
    api.setAi(provider, model, baseUrl, key || null, consent).then(
      () => {
        setKey("");
        setMessage({ ok: true, text: provider === "none" ? "AI clean-up is off." : "Saved." });
        refresh();
      },
      (e) => setMessage({ ok: false, text: errorText(e) }),
    );

  return (
    <div className="card">
      <h3>AI clean-up (optional)</h3>
      <p className="dim">
        Off by default. When on, the text of each dictation (never the audio) is sent to the service you choose, to fix
        grammar and self-corrections the built-in rules cannot. Every answer is checked: if it adds a number, name, e-mail
        or link you did not say, or takes longer than 2.5 seconds, the built-in rules are used instead.
      </p>
      <p>
        Status: <strong className={status.active ? "ok" : ""}>{status.active ? `On (${status.provider})` : "Off - built-in rules only"}</strong>
      </p>
      <div className="field">
        <label htmlFor="provider">Service</label>
        <select
          id="provider"
          value={provider}
          onChange={(e) => {
            setProvider(e.target.value);
            setModel("");
            setBaseUrl("");
            setConsent(false);
            setMessage(null);
          }}
        >
          <option value="none">Off</option>
          {status.presets.map((p) => (
            <option key={p.id} value={p.id}>
              {p.name}
            </option>
          ))}
        </select>
      </div>
      {preset && (
        <>
          <div className="field">
            <label htmlFor="model">Model</label>
            <input id="model" type="text" value={model} placeholder={preset.model || "model name"} onChange={(e) => setModel(e.target.value)} />
          </div>
          {(preset.id === "custom" || preset.id === "ollama") && (
            <div className="field">
              <label htmlFor="baseurl">Server address</label>
              <input id="baseurl" type="text" value={baseUrl} placeholder={preset.base_url || "https://..."} onChange={(e) => setBaseUrl(e.target.value)} />
            </div>
          )}
          {preset.needs_key && (
            <div className="field">
              <label htmlFor="key">API key</label>
              <span className="row">
                <input
                  id="key"
                  type="password"
                  autoComplete="off"
                  value={key}
                  placeholder={status.has_key && status.provider === provider ? "saved (paste a new one to replace it)" : "paste your key"}
                  onChange={(e) => setKey(e.target.value)}
                />
                {preset.key_url && <span className="dim">Get a free key at {preset.key_url}</span>}
              </span>
            </div>
          )}
          <p className="raw">{preset.data_note}</p>
          <label className="row">
            <input type="checkbox" checked={consent} onChange={(e) => setConsent(e.target.checked)} />I understand and agree to
            send dictation text to this service.
          </label>
          <p className="dim">The key is stored in Windows Credential Manager on this computer only.</p>
        </>
      )}
      <div className="row">
        <button className="btn primary" disabled={busy || (provider !== "none" && !consent)} onClick={() => void save()}>
          Save
        </button>
        {status.active && (
          <button
            className="btn"
            disabled={busy}
            onClick={() => {
              setBusy(true);
              setMessage({ ok: true, text: "Testing with a sample sentence..." });
              void api
                .testAi()
                .then((r) => setMessage({ ok: r.ok, text: r.message }), (e) => setMessage({ ok: false, text: errorText(e) }))
                .finally(() => setBusy(false));
            }}
          >
            Test
          </button>
        )}
        {status.has_key && status.provider !== "none" && (
          <button className="btn" onClick={() => void api.forgetAiKey(status.provider).then(refresh)}>
            Remove key and turn off
          </button>
        )}
      </div>
      {message && <p className={message.ok ? "ok" : "bad"}>{message.text}</p>}
    </div>
  );
}

export function UploadsCard() {
  const [events, refresh] = useLoad(api.uploads, [] as UploadEvent[]);
  return (
    <div className="card">
      <div className="row spread">
        <h3>Data that left this computer</h3>
        <button className="btn" onClick={refresh}>
          Refresh
        </button>
      </div>
      {events.length === 0 ? (
        <p className="dim">Nothing has been sent anywhere.</p>
      ) : (
        events.map((e, i) => (
          <p key={i}>
            <span className="dim">{new Date(e.at).toLocaleString()}</span> · {e.detail}
          </p>
        ))
      )}
    </div>
  );
}

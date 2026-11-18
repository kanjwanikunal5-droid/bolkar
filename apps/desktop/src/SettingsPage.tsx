import { listen } from "@tauri-apps/api/event";
import { useCallback, useEffect, useRef, useState } from "react";
import { api, errorText, type AiStatus, type Model, type ModelProgress, type Settings, type Status, type UpdateInfo } from "./api";
import { Icon } from "./icons";
import { AiSettings, UploadsCard } from "./Pages";
import { MicLevel, PageHead, Segmented, SettingRow, Switch, applyTheme, useToast, type ThemeChoice } from "./ui";

type Section = "general" | "speech" | "groq" | "writing" | "privacy" | "about";

const SECTIONS: [Section, string, JSX.Element][] = [
  ["general", "General", <Icon.Gear size={18} />],
  ["speech", "Speech", <Icon.Mic size={18} />],
  ["groq", "Groq & AI", <Icon.Sparkles size={18} />],
  ["writing", "Writing", <Icon.Pen size={18} />],
  ["privacy", "Privacy & data", <Icon.Shield size={18} />],
  ["about", "About & updates", <Icon.Info size={18} />],
];

type Set = (key: string, value: string, note?: string) => void;

/* ---------- General ---------- */

function General({ settings, set }: { settings: Settings; set: Set }) {
  const [name, setName] = useState(settings.user_name ?? "");
  const timer = useRef<number | undefined>(undefined);
  return (
    <div className="card">
      <SettingRow title="Appearance" help="Follow Windows, or always light or dark.">
        <Segmented<ThemeChoice>
          label="Appearance"
          value={(settings.theme as ThemeChoice) || "system"}
          onChange={(v) => {
            applyTheme(v);
            set("theme", v, "Appearance saved");
          }}
          options={[
            ["system", "System"],
            ["light", "Light"],
            ["dark", "Dark"],
          ]}
        />
      </SettingRow>
      <SettingRow title="Text size" help="Everything in this window grows or shrinks.">
        <select aria-label="Text size" value={settings.ui_scale ?? "125"} onChange={(e) => set("ui_scale", e.target.value, "Text size saved")}>
          <option value="100">Normal (100%)</option>
          <option value="115">Larger (115%)</option>
          <option value="125">Large (125%)</option>
          <option value="150">Very large (150%)</option>
        </select>
      </SettingRow>
      <SettingRow title="Daily goal" help="Words per day for the challenge on Home.">
        <select aria-label="Daily goal" value={settings.daily_goal ?? "100"} onChange={(e) => set("daily_goal", e.target.value, "Daily goal saved")}>
          {["50", "100", "200", "300", "500", "1000"].map((n) => (
            <option key={n} value={n}>
              {n} words
            </option>
          ))}
        </select>
      </SettingRow>
      <SettingRow title="Sounds" help="A soft sound when listening starts and when your text is typed.">
        <Switch label="Sounds" checked={settings.sounds !== "off"} onChange={(on) => set("sounds", on ? "on" : "off", on ? "Sounds on" : "Sounds off")} />
      </SettingRow>
      <SettingRow title="Your name" help="Only used to greet you on Home. Stays on this computer.">
        <input
          type="text"
          aria-label="Your name"
          placeholder="Optional"
          value={name}
          onChange={(e) => {
            setName(e.target.value);
            window.clearTimeout(timer.current);
            const v = e.target.value.trim();
            timer.current = window.setTimeout(() => set("user_name", v, "Name saved"), 700);
          }}
        />
      </SettingRow>
    </div>
  );
}

/* ---------- Speech ---------- */

function Models({ selected, onSelect }: { selected: string; onSelect: (id: string) => void }) {
  const toast = useToast();
  const [models, setModels] = useState<Model[]>([]);
  const [progress, setProgress] = useState<Record<string, ModelProgress>>({});
  const refresh = useCallback(() => void api.models().then(setModels), []);
  useEffect(() => {
    refresh();
    const off = listen<ModelProgress>("model-progress", (e) => {
      const p = e.payload;
      if (p.finished) {
        setProgress((all) => {
          const next = { ...all };
          delete next[p.id];
          return next;
        });
        if (p.error) toast(`Download failed: ${p.error}`, true);
        else toast("Model installed");
        refresh();
      } else setProgress((all) => ({ ...all, [p.id]: p }));
    });
    return () => void off.then((f) => f());
  }, [refresh, toast]);

  return (
    <div className="stack" style={{ gap: "0.6rem" }}>
      {models.map((m) => {
        const p = progress[m.id];
        const active = selected === m.id;
        return (
          <div
            key={m.id}
            className="card lift"
            style={{ marginBottom: 0, borderColor: active ? "var(--accent)" : undefined, boxShadow: active ? "0 0 0 3px var(--accent-soft)" : undefined }}
          >
            <div className="row spread" style={{ alignItems: "flex-start" }}>
              <div style={{ minWidth: 0, flex: 1 }}>
                <div className="row" style={{ gap: "0.5rem" }}>
                  <strong>{m.label}</strong>
                  {active && <span className="badge ok">In use</span>}
                  {m.installed && !active && <span className="badge">Installed</span>}
                </div>
                <div className="dim" style={{ fontSize: "0.72rem", marginTop: "0.2rem" }}>
                  {m.size_mb} MB · about {m.ram_mb} MB of memory{m.note ? ` · ${m.note}` : ""}
                </div>
                {p && (
                  <div className="row" style={{ marginTop: "0.5rem" }}>
                    <progress value={p.done} max={p.total || 1} aria-label={`Downloading ${m.label}`} />
                    <span className="dim">{p.total ? Math.round((p.done / p.total) * 100) : 0}%</span>
                  </div>
                )}
              </div>
              <div className="row">
                {m.installed ? (
                  <>
                    {!active && (
                      <button className="btn primary small" onClick={() => onSelect(m.id)}>
                        Use this
                      </button>
                    )}
                    {!active && (
                      <button
                        className="btn ghost small danger"
                        aria-label={`Delete ${m.label}`}
                        onClick={() => void api.deleteModel(m.id).then(refresh, (e) => toast(errorText(e), true))}
                      >
                        <Icon.Trash size={15} />
                      </button>
                    )}
                  </>
                ) : (
                  !p && (
                    <button className="btn small" onClick={() => void api.downloadModel(m.id)}>
                      <Icon.Download size={15} /> Download
                    </button>
                  )
                )}
              </div>
            </div>
          </div>
        );
      })}
    </div>
  );
}

function Speech({ settings, set, status }: { settings: Settings; set: Set; status: Status | null }) {
  return (
    <>
      <div className="card">
        <SettingRow title="Language" help="English + Hinglish decides by itself for every sentence. Hinglish is written in Roman letters, like WhatsApp. Shortcut: Alt + Shift + H.">
          <Segmented
            label="Language"
            value={settings.language === "en" || settings.language === "hinglish" ? settings.language : "en-hinglish"}
            onChange={(v) => set("language", v, "Language saved")}
            options={[
              ["en-hinglish", "English + Hinglish"],
              ["en", "English"],
              ["hinglish", "Hinglish"],
            ]}
          />
        </SettingRow>
        <SettingRow title="Microphone" help="Speak to test. The microphone only records while you hold the keys.">
          <MicLevel />
        </SettingRow>
        <SettingRow title="Instant start" help="Keeps the microphone ready so your first word is never cut off. Only the last 0.3 s is kept in memory; nothing is saved.">
          <Switch label="Instant start" checked={settings.keep_mic_open === "true"} onChange={(on) => set("keep_mic_open", String(on), on ? "Instant start on" : "Instant start off")} />
        </SettingRow>
        {status?.gpu_build && (
          <SettingRow
            title="Use the graphics card"
            help={
              settings.use_gpu === "true"
                ? "Much faster speech recognition on this computer."
                : "OFF: speech recognition on this computer is many times slower; with a large model a dictation can take minutes. Turn it on."
            }
          >
            <Switch label="Use the graphics card" checked={settings.use_gpu === "true"} onChange={(on) => set("use_gpu", String(on), on ? "Graphics card on" : "Graphics card off")} />
          </SettingRow>
        )}
      </div>
      <h2 style={{ margin: "1.3rem 0 0.7rem" }}>Speech model on this computer</h2>
      <p className="dim" style={{ marginTop: 0 }}>
        Used when Groq is off or not reachable, and to tell English from Hinglish. The best installed model is picked automatically.
      </p>
      <Models selected={settings.model ?? ""} onSelect={(id) => set("model", id, "Speech model changed")} />
    </>
  );
}

/* ---------- Groq & AI ---------- */

function Groq({ ai, refresh }: { ai: AiStatus | null; refresh: () => void }) {
  const toast = useToast();
  const [key, setKey] = useState("");
  const [busy, setBusy] = useState(false);
  const [replacing, setReplacing] = useState(false);
  if (!ai) return <div className="card skeleton" style={{ height: "12rem" }} />;
  const connected = !!ai.groq_key_hint && !replacing;

  const connect = () => {
    setBusy(true);
    api
      .connectGroq(key)
      .then(
        (msg) => {
          toast(msg);
          setKey("");
          setReplacing(false);
          refresh();
        },
        (e) => toast(errorText(e), true),
      )
      .finally(() => setBusy(false));
  };

  return (
    <>
      <div className="card" style={{ borderColor: connected ? "var(--accent)" : undefined }}>
        <div className="row spread" style={{ alignItems: "flex-start" }}>
          <div className="row" style={{ alignItems: "flex-start", gap: "0.9rem" }}>
            <span className="stat-icon" style={{ marginBottom: 0 }}>
              <Icon.Key size={18} />
            </span>
            <div>
              <h2 style={{ margin: 0 }}>Groq</h2>
              {connected ? (
                <p className="dim" style={{ margin: "0.15rem 0 0" }}>
                  <span className="badge ok">
                    <Icon.Check size={12} /> Connected
                  </span>{" "}
                  Key ending ••••{ai.groq_key_hint}, kept in Windows Credential Manager. You never need to enter it again.
                </p>
              ) : (
                <p className="dim" style={{ margin: "0.15rem 0 0" }}>
                  Best measured accuracy for English and Hinglish. Free key from <strong>console.groq.com/keys</strong>.
                </p>
              )}
            </div>
          </div>
          {connected && (
            <span className="row">
              <button className="btn small" onClick={() => setReplacing(true)}>
                Replace key
              </button>
              <button
                className="btn ghost small danger"
                onClick={() =>
                  void api.forgetAiKey("groq").then(() => {
                    void api.setCloudAsr(false, false);
                    toast("Groq disconnected and key removed");
                    refresh();
                  })
                }
              >
                Disconnect
              </button>
            </span>
          )}
        </div>
        {!connected && (
          <div style={{ marginTop: "1rem" }}>
            <div className="row">
              <input
                type="password"
                autoComplete="off"
                aria-label="Groq API key"
                placeholder="Paste your Groq key (gsk_...)"
                value={key}
                onChange={(e) => setKey(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && key.trim() && connect()}
                style={{ flex: 1, minWidth: "16rem" }}
              />
              <button className="btn primary" disabled={!key.trim() || busy} onClick={connect}>
                {busy ? "Checking..." : "Connect"}
              </button>
              {replacing && (
                <button className="btn ghost" onClick={() => setReplacing(false)}>
                  Cancel
                </button>
              )}
            </div>
            <p className="dim" style={{ fontSize: "0.72rem", marginBottom: 0 }}>
              Connecting turns on both switches below: your dictation <strong>audio</strong> is sent to Groq for speech recognition and the{" "}
              <strong>text</strong> for clean-up. Groq&apos;s free tier has daily limits; read Groq&apos;s privacy policy. You can switch either off at any
              time, and every upload is listed under Privacy &amp; data.
            </p>
          </div>
        )}
      </div>

      <div className="card">
        <SettingRow
          title="Speech recognition by Groq"
          help="Your audio is recognised by Groq Whisper: the best English and Hinglish we measured. If Groq is slow or offline, this computer takes over automatically."
        >
          <Switch
            label="Speech recognition by Groq"
            checked={ai.cloud_asr}
            disabled={!ai.groq_key_hint}
            onChange={(on) =>
              void api.setCloudAsr(on, true).then(
                () => {
                  toast(on ? "Groq speech recognition on" : "Recognising on this computer");
                  refresh();
                },
                (e) => toast(errorText(e), true),
              )
            }
          />
        </SettingRow>
        <SettingRow
          title="AI clean-up"
          help="The text (never the audio) is tidied: grammar, Hinglish spelling, self-corrections. Every answer is checked; anything that adds a number, name or link you did not say is thrown away. Also writes meeting notes: the meeting transcript is sent as text; meeting audio never leaves this computer."
        >
          <Switch
            label="AI clean-up"
            checked={ai.active}
            disabled={!ai.groq_key_hint && ai.provider === "none"}
            onChange={(on) =>
              void api.setAiEnabled(on).then(
                () => {
                  toast(on ? "AI clean-up on" : "AI clean-up off");
                  refresh();
                },
                (e) => toast(errorText(e), true),
              )
            }
          />
        </SettingRow>
        <SettingRow title="Check the connection" help="Sends one fixed sample sentence, no personal data.">
          <button
            className="btn small"
            disabled={!ai.active || busy}
            onClick={() => {
              setBusy(true);
              void api
                .testAi()
                .then((r) => toast(r.message, !r.ok), (e) => toast(errorText(e), true))
                .finally(() => setBusy(false));
            }}
          >
            <Icon.Refresh size={15} /> Test
          </button>
        </SettingRow>
      </div>
      <details className="card">
        <summary style={{ cursor: "pointer", fontWeight: 650 }}>Use another AI service instead (advanced)</summary>
        <div style={{ marginTop: "0.8rem" }}>
          <AiSettings />
        </div>
      </details>
    </>
  );
}

/* ---------- Writing ---------- */

function Writing({ settings, set }: { settings: Settings; set: Set }) {
  return (
    <div className="card">
      <SettingRow title="Clean-up style" help="Light removes “um”, fixes self-corrections and formats lists. Professional also polishes grammar (needs AI clean-up). Literal keeps every word.">
        <Segmented
          label="Clean-up style"
          value={settings.level ?? "LIGHT"}
          onChange={(v) => set("level", v, "Clean-up style saved")}
          options={[
            ["LITERAL", "Literal"],
            ["LIGHT", "Light"],
            ["PROFESSIONAL", "Professional"],
          ]}
        />
      </SettingRow>
      <SettingRow
        title="Lists in hands-free mode"
        help="Natural writing keeps connected sentences together as a paragraph. A list is made only when you speak one: “first ..., second ...”, “pehli baat ...”, or “bullet point ...”. Auto-detect also turns short, separate requests into bullets. Always makes every hands-free dictation of two or more sentences into bullets."
      >
        <select
          aria-label="Lists in hands-free mode"
          value={["natural", "auto", "always"].includes(settings.list_format ?? "") ? settings.list_format : "natural"}
          onChange={(e) => set("list_format", e.target.value, "List setting saved")}
        >
          <option value="natural">Natural writing (recommended)</option>
          <option value="auto">Auto-detect lists</option>
          <option value="always">Always use bullets</option>
        </select>
      </SettingRow>
      <SettingRow title="Spoken commands" help="Say “new line”, “full stop” or “bullet point” while dictating.">
        <Switch label="Spoken commands" checked={settings.spoken_commands === "true"} onChange={(on) => set("spoken_commands", String(on), on ? "Spoken commands on" : "Spoken commands off")} />
      </SettingRow>
    </div>
  );
}

/* ---------- Privacy ---------- */

function Keep({ id, label, value, set, choices }: { id: string; label: string; value: string | undefined; set: Set; choices: [string, string][] }) {
  return (
    <select aria-label={label} value={value ?? choices[0][0]} onChange={(e) => set(id, e.target.value, `${label}: saved`)}>
      {choices.map(([v, l]) => (
        <option key={v} value={v}>
          {l}
        </option>
      ))}
    </select>
  );
}

const DAYS: [string, string][] = [
  ["1", "1 day"],
  ["7", "7 days"],
  ["30", "30 days"],
  ["90", "90 days"],
  ["365", "1 year"],
  ["0", "Keep forever"],
];

function Diagnostics({ settings, set }: { settings: Settings; set: Set }) {
  const toast = useToast();
  const [info, setInfo] = useState<{ enabled: boolean; path: string | null; bytes: number } | null>(null);
  const refresh = useCallback(() => void api.diagnosticsInfo().then(setInfo, () => setInfo(null)), []);
  useEffect(refresh, [refresh, settings.diagnostics]);
  return (
    <div className="card">
      <div className="card-title">
        <h2>Diagnostics log</h2>
      </div>
      <p className="dim" style={{ marginTop: 0 }}>
        A small file on this computer that records what happened (versions, steps, timings, error messages) so problems can be found. It never contains
        what you said, transcripts, meeting titles, names or keys. At most 1 MB.
      </p>
      <SettingRow title="Keep a diagnostics log" help={info ? `Now ${(info.bytes / 1024).toFixed(0)} KB.` : ""}>
        <Switch
          label="Keep a diagnostics log"
          checked={settings.diagnostics !== "off"}
          onChange={(on) => set("diagnostics", on ? "on" : "off", on ? "Diagnostics log on" : "Diagnostics log off")}
        />
      </SettingRow>
      <div className="row" style={{ flexWrap: "wrap" }}>
        <button className="btn small" disabled={!info?.path} onClick={() => info?.path && void api.revealFile(info.path).catch((e) => toast(errorText(e), true))}>
          Open log folder
        </button>
        <button
          className="btn small"
          onClick={() =>
            void api.exportDiagnostics().then(
              (p) => {
                toast("Diagnostics saved in Downloads");
                void api.revealFile(p);
              },
              (e) => toast(errorText(e), true),
            )
          }
        >
          Save diagnostics for support
        </button>
        <button
          className="btn ghost small danger"
          onClick={() =>
            void api.deleteLogs().then(
              () => {
                toast("Diagnostics log deleted");
                refresh();
              },
              (e) => toast(errorText(e), true),
            )
          }
        >
          Delete log
        </button>
      </div>
    </div>
  );
}

function Privacy({ settings, set }: { settings: Settings; set: Set }) {
  return (
    <>
      <div className="card">
        <div className="card-title">
          <h2>Stored on this computer</h2>
        </div>
        <SettingRow title="Keep dictation recordings" help="Lets you replay or re-transcribe a dictation.">
          <Switch label="Keep dictation recordings" checked={settings.save_audio === "true"} onChange={(on) => set("save_audio", String(on), on ? "Recordings kept" : "Recordings not kept")} />
        </SettingRow>
        <SettingRow title="Delete dictation audio after" help="Only the audio files of dictations. Their text stays in History.">
          <Keep id="audio_retention_days" label="Delete dictation audio after" value={settings.audio_retention_days ?? "7"} set={set} choices={DAYS.slice(0, 4)} />
        </SettingRow>
        <SettingRow title="Delete meeting audio after" help="Only the meeting recordings (WAV files). Transcripts and notes stay in Past meetings.">
          <Keep id="meeting_audio_retention_days" label="Delete meeting audio after" value={settings.meeting_audio_retention_days ?? "30"} set={set} choices={DAYS} />
        </SettingRow>
        <SettingRow title="Delete dictation history after" help="The text of old dictations, with any audio left.">
          <Keep id="history_retention_days" label="Delete dictation history after" value={settings.history_retention_days ?? "0"} set={set} choices={[["0", "Keep forever"], ...DAYS.slice(1, 5)]} />
        </SettingRow>
        <SettingRow title="Delete meetings after" help="Whole meetings: audio, transcript, your notes and the AI notes.">
          <Keep id="meeting_retention_days" label="Delete meetings after" value={settings.meeting_retention_days ?? "0"} set={set} choices={[["0", "Keep forever"], ...DAYS.slice(1, 5)]} />
        </SettingRow>
        <p className="dim" style={{ fontSize: "0.72rem", marginBottom: 0 }}>
          Checked when the app starts and every 6 hours. A meeting that is being recorded is never deleted. If a file cannot be deleted, it stays listed and the
          failure is noted in the diagnostics log.
        </p>
      </div>
      <div className="card">
        <div className="card-title">
          <h2>Sent to other services</h2>
        </div>
        <p className="dim" style={{ marginTop: 0 }}>
          With speech recognition on this computer, your audio never leaves it. Only if you turn them on in Groq &amp; AI: <strong>Groq speech recognition</strong>{" "}
          sends the audio of each dictation to Groq; <strong>AI clean-up</strong> sends the text of each dictation, and the meeting transcript for notes. Meeting
          audio is always recognised on this computer. Each upload is listed below.
        </p>
      </div>
      <UploadsCard />
      <Diagnostics settings={settings} set={set} />
    </>
  );
}

/* ---------- About ---------- */

function About({ settings, set, status }: { settings: Settings; set: Set; status: Status | null }) {
  const [info, setInfo] = useState<UpdateInfo | null>(null);
  const [busy, setBusy] = useState(false);
  return (
    <div className="card">
      <SettingRow title={`${status?.app_name ?? ""} ${status?.version ?? ""}`} help="Free and open source. Speech is recognised on this computer unless you turn on Groq.">
        <button
          className="btn small"
          disabled={busy}
          onClick={() => {
            setBusy(true);
            void api.checkUpdate().then((i) => {
              setInfo(i);
              setBusy(false);
            });
          }}
        >
          <Icon.Refresh size={15} /> {busy ? "Checking..." : "Check for updates"}
        </button>
        {info?.newer && (
          <button className="btn primary small" onClick={() => void api.openReleasePage(info.url)}>
            <Icon.Download size={15} /> Get {info.latest}
          </button>
        )}
      </SettingRow>
      {info && <p className={info.newer ? "ok" : "dim"}>{info.message}</p>}
      <SettingRow title="Check automatically" help="Asks GitHub when the app starts. Nothing is ever installed without you.">
        <Switch
          label="Check for updates automatically"
          checked={settings.check_updates === "on"}
          onChange={(on) => set("check_updates", on ? "on" : "off", on ? "Automatic update check on" : "Automatic update check off")}
        />
      </SettingRow>
    </div>
  );
}

export function SettingsPage({
  settings,
  status,
  ai,
  section,
  setSection,
  set,
  refreshAi,
}: {
  settings: Settings;
  status: Status | null;
  ai: AiStatus | null;
  section: string;
  setSection: (s: string) => void;
  set: Set;
  refreshAi: () => void;
}) {
  const current = (SECTIONS.find(([id]) => id === section)?.[0] ?? "general") as Section;
  return (
    <div className="page">
      <PageHead title="Settings" sub="Changes are saved the moment you make them." />
      <div className="settings">
        <nav className="settings-nav" aria-label="Settings sections">
          {SECTIONS.map(([id, label, icon]) => (
            <button key={id} className="nav-item" aria-current={current === id ? "page" : undefined} onClick={() => setSection(id)}>
              {icon}
              {label}
            </button>
          ))}
        </nav>
        <div key={current} className="page" style={{ maxWidth: "none" }}>
          {current === "general" && <General settings={settings} set={set} />}
          {current === "speech" && <Speech settings={settings} set={set} status={status} />}
          {current === "groq" && <Groq ai={ai} refresh={refreshAi} />}
          {current === "writing" && <Writing settings={settings} set={set} />}
          {current === "privacy" && <Privacy settings={settings} set={set} />}
          {current === "about" && <About settings={settings} set={set} status={status} />}
        </div>
      </div>
    </div>
  );
}

/**
 * DEVELOPMENT ONLY. Lets `pnpm dev` show the interface in an ordinary browser (no Tauri) so
 * pages can be reviewed in both themes. The data below is sample data for layout checks; it is
 * never part of the installed app (main.tsx loads this file only when import.meta.env.DEV is
 * true and the Tauri runtime is absent).
 */
type Handler = (args: Record<string, unknown>) => unknown;

const now = Date.now();
const iso = (minsAgo: number) => new Date(now - minsAgo * 60_000).toISOString();
const texts = [
  ["Bhai kal meeting chaar baje hai, please time pe aa jana.", "WhatsApp.exe"],
  ["Please send me the updated balance sheet before the end of the day.", "OUTLOOK.EXE"],
  ["Let's keep the presentation short and focus on the key numbers.", "WINWORD.EXE"],
  ["Report ready hai, main tujhe shaam tak bhej dunga.", "WhatsApp.exe"],
  ["Can we reschedule our call to Thursday afternoon?", "chrome.exe"],
  ["The client has approved the proposal, so we can start next week.", "OUTLOOK.EXE"],
  ["Our cash flow looks healthy, but receivables are taking longer to collect.", "EXCEL.EXE"],
];
const dictations = texts.map(([t, app], i) => ({
  id: `d${i}`,
  created_at: iso(i * 170 + 5),
  app_process: app,
  app_title: "",
  app_category: "",
  language: "en-hinglish",
  engine: "Groq - whisper-large-v3",
  level: i === 2 ? "COMMAND" : "LIGHT",
  raw_text: t,
  final_text: t,
  inserted: true,
  insert_method: "clipboard:ctrl+v",
  duration_ms: 4200,
  audio_path: null,
  word_count: t.split(" ").length,
  error: null,
  cleanup: i % 2 ? "groq" : "rules",
}));
const days = Array.from({ length: 119 }, (_, i) => {
  const d = new Date(now - (118 - i) * 86_400_000);
  const w = i > 60 && (i * 7) % 5 !== 0 ? ((i * 37) % 260) + 20 : 0;
  return { date: d.toISOString().slice(0, 10), words: w, dictations: w ? Math.ceil(w / 22) : 0 };
});
let settings: Record<string, string> = {
  model: "large-v3",
  language: "en-hinglish",
  level: "LIGHT",
  use_gpu: "true",
  keep_mic_open: "true",
  spoken_commands: "true",
  save_audio: "true",
  audio_retention_days: "7",
  ui_scale: "125",
  scratchpad: "",
  check_updates: "off",
  theme: "system",
  user_name: "Kunal",
  daily_goal: "100",
  sounds: "on",
  style_chat: "casual",
  style_email: "formal",
  style_docs: "clear",
  style_other: "clear",
};

const handlers: Record<string, Handler> = {
  get_status: () => ({
    app_name: "Preview",
    version: "0.2.0",
    gpu_build: true,
    running_as_admin: false,
    hook_error: null,
    selected_model: settings.model,
    model_installed: true,
    words_this_week: 1260,
    shortcuts: [],
    cloud_speech: true,
    gpu_on: true,
    gpu_present: true,
    other_dictation_apps: ["Wispr Flow"],
  }),
  get_settings: () => settings,
  set_setting: (a) => {
    settings = { ...settings, [a.key as string]: a.value as string };
  },
  list_dictations: () => dictations,
  get_insights: () => ({
    total_words: 18432,
    total_dictations: 702,
    commands: 14,
    words_today: 64,
    words_this_week: 1260,
    speaking_wpm: 131.4,
    minutes_saved: 372,
    typing_wpm: 40,
    current_streak: 12,
    longest_streak: 19,
    active_days: 41,
    days,
    apps: [
      { name: "WhatsApp", words: 6100, share: 0.33 },
      { name: "Outlook", words: 4300, share: 0.23 },
      { name: "Word", words: 3700, share: 0.2 },
      { name: "Chrome", words: 2600, share: 0.14 },
      { name: "Excel", words: 1700, share: 0.09 },
    ],
    fixes: 23,
    learned_words: 9,
    your_words: 6,
    first_day: days[61].date,
  }),
  get_ai: () => ({
    presets: [],
    provider: "groq",
    model: "qwen/qwen3.8-27b",
    base_url: "",
    has_key: true,
    consent: true,
    active: true,
    cloud_asr: true,
    cloud_asr_possible: true,
    cloud_asr_note: "",
    groq_key_hint: "x7Qe",
  }),
  list_models: () => [
    { id: "large-v3", label: "Large v3 (best for Hinglish, with AI clean-up on)", file: "", size_mb: 1031, ram_mb: 2600, note: "About 3 times slower than Turbo.", installed: true },
    { id: "large-v3-turbo", label: "Large v3 Turbo (recommended: fast and accurate)", file: "", size_mb: 547, ram_mb: 1500, note: "", installed: true },
    { id: "small", label: "Small (balanced)", file: "", size_mb: 181, ram_mb: 700, note: "", installed: false },
  ],
  mic_status: () => ({ open: true, level: 0.02 + Math.random() * 0.05 }),
  list_uploads: () => [],
  list_terms: () => [],
  list_snippets: () => [],
  meeting_status: () => null,
  check_update: () => ({ current: "0.2.0", latest: null, newer: false, url: null, message: "You have the latest version." }),
  list_audio_devices: () => ({
    inputs: ["Microphone Array (Realtek)", "Headset Microphone (USB)"],
    outputs: ["Speakers (Realtek)", "Headphones (USB)"],
    default_input: "Microphone Array (Realtek)",
    default_output: "Speakers (Realtek)",
  }),
  list_meetings: (a) =>
    [sampleMeeting, { ...sampleMeeting, id: "m2", title: "Interrupted recording", status: "interrupted", has_summary: false, segments: 0, started_at: iso(3000) }]
      .filter((m) => !a.query || m.title.toLowerCase().includes(String(a.query).toLowerCase()))
      .map(({ id, title, started_at, duration_ms, status, has_audio, has_summary, segments }) => ({ id, title, started_at, duration_ms, status, has_audio, has_summary, segments })),
  get_meeting: () => sampleView,
  diagnostics_info: () => ({ enabled: true, path: "C:\\data\\logs\\diagnostics.log", bytes: 18432 }),
};

const sampleMeeting = {
  id: "m1",
  title: "Weekly finance review",
  started_at: iso(1500),
  duration_ms: 1_380_000,
  status: "summarised",
  has_audio: true,
  has_summary: true,
  segments: 4,
};
const seg = (id: string, ms: number, speaker: string, text: string, edited = false) => ({
  id,
  start_ms: ms,
  end_ms: ms + 6000,
  speaker,
  text,
  original: edited ? text.replace("margin report", "report") : text,
  edited,
});
const sampleView = {
  ...sampleMeeting,
  segments: [
    seg("s1", 4000, "System audio", "Sales went up 12 percent this quarter because volumes were higher."),
    seg("s2", 11000, "Microphone", "But the discounts reduced our margin, so we need to look at pricing."),
    seg("s3", 19000, "System audio", "Maybe we should move the price review to next month."),
    seg("s4", 27000, "System audio", "Priya will send the margin report by Friday.", true),
  ],
  thoughts: [[15000, "Ask about the discount policy"]],
  summary: {
    overview: { title: "Weekly finance review", date: null, duration_min: 23, participants: [], purpose: null },
    executive_summary: [{ text: "Sales grew 12% but discounts squeezed the margin.", evidence: ["s1", "s2"], check: "linked" }],
    topics: [
      {
        title: "Sales and margin",
        narrative: {
          text: "Sales rose 12% this quarter on higher volumes, but discounts reduced the margin, so pricing needs a closer look.",
          evidence: ["s1", "s2"],
          check: "quoted",
        },
        points: [],
      },
    ],
    decisions: [{ text: "Move the price review to next month.", status: "proposed", evidence: ["s3"], check: "linked" }],
    action_items: [{ action: "Send the margin report", owner: "Priya", deadline: "Friday", evidence: ["s4"], check: "linked", deadline_tentative: false }],
    risks: [{ text: "Margin keeps falling if discounts continue at 20%.", evidence: ["s2"], check: "needs_review", note: "the number 20 is not in the cited transcript lines" }],
    open_questions: [],
    numbers: [{ value: "12%", context: "sales growth this quarter", evidence: ["s1"] }],
    next_steps: [],
    takeaway: null,
  },
  dropped: [],
  flags: ['"Margin keeps falling if discounts continue at 20%.": the number 20 is not in the cited transcript lines'],
  markdown: "# Weekly finance review",
  note: null,
  problems: [],
};

let callbackId = 1;
(window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {
  transformCallback: () => callbackId++,
  invoke: async (cmd: string, args: Record<string, unknown> = {}) => {
    if (cmd.startsWith("plugin:event|")) return callbackId++;
    const h = handlers[cmd];
    if (!h) throw `preview: no sample data for ${cmd}`;
    return h(args);
  },
};

export {};

import { invoke as tauriInvoke, type InvokeArgs } from "@tauri-apps/api/core";

const READ_ONLY = new Set([
  "get_status",
  "get_settings",
  "list_dictations",
  "list_models",
  "get_ai",
  "list_uploads",
  "list_terms",
  "list_snippets",
  "last_dictation_id",
  "mic_status",
  "meeting_status",
  "get_insights",
  "list_meetings",
  "get_meeting",
  "list_audio_devices",
  "diagnostics_info",
]);

/**
 * The window can open a moment before the app has finished starting up, and the first
 * calls then fail. Read-only calls are retried for up to about 6 seconds; calls that change
 * something are never retried, so nothing can happen twice.
 */
async function invoke<T>(cmd: string, args?: InvokeArgs): Promise<T> {
  const attempts = READ_ONLY.has(cmd) ? 20 : 1;
  for (let i = 1; ; i++) {
    try {
      return await tauriInvoke<T>(cmd, args);
    } catch (e) {
      if (i >= attempts) throw e;
      await new Promise((r) => setTimeout(r, 300));
    }
  }
}

export interface Status {
  app_name: string;
  version: string;
  gpu_build: boolean;
  running_as_admin: boolean;
  hook_error: string | null;
  selected_model: string;
  model_installed: boolean;
  words_this_week: number;
  shortcuts: [string, string][];
  cloud_speech: boolean;
  gpu_on: boolean;
  gpu_present: boolean;
  /** Other dictation apps running now (may use the same keys or the microphone). */
  other_dictation_apps: string[];
}

export interface Dictation {
  id: string;
  created_at: string;
  app_process: string;
  app_title: string;
  app_category: string;
  language: string;
  engine: string;
  level: string;
  raw_text: string;
  final_text: string;
  inserted: boolean;
  insert_method: string;
  duration_ms: number;
  audio_path: string | null;
  word_count: number;
  error: string | null;
  cleanup: string;
}

export interface Model {
  id: string;
  label: string;
  file: string;
  size_mb: number;
  ram_mb: number;
  note: string;
  installed: boolean;
}

export interface UpdateInfo {
  current: string;
  latest: string | null;
  newer: boolean;
  url: string | null;
  message: string;
}

export interface ModelProgress {
  id: string;
  done: number;
  total: number;
  finished: boolean;
  error: string | null;
}

export interface PillState {
  state: "listening" | "transcribing" | "polishing" | "done" | "info" | "error" | "hidden";
  message: string;
  level: number;
  seconds: number;
  hands_free: boolean;
  engine: string;
  sounds: boolean;
}

export type Settings = Record<string, string>;

export interface Preset {
  id: string;
  name: string;
  base_url: string;
  model: string;
  needs_key: boolean;
  key_url: string;
  data_note: string;
}

export interface AiStatus {
  presets: Preset[];
  provider: string;
  model: string;
  base_url: string;
  has_key: boolean;
  consent: boolean;
  active: boolean;
  cloud_asr: boolean;
  cloud_asr_possible: boolean;
  cloud_asr_note: string;
  groq_key_hint: string | null;
}

export interface Insights {
  total_words: number;
  total_dictations: number;
  commands: number;
  words_today: number;
  words_this_week: number;
  speaking_wpm: number | null;
  minutes_saved: number;
  typing_wpm: number;
  current_streak: number;
  longest_streak: number;
  active_days: number;
  days: { date: string; words: number; dictations: number }[];
  apps: { name: string; words: number; share: number }[];
  fixes: number;
  learned_words: number;
  your_words: number;
  first_day: string | null;
}

export interface Term {
  id: string;
  term: string;
  spoken_variants: string[];
  source: "manual" | "learned" | "pack";
  auto_replace: boolean;
}

export interface Snippet {
  id: string;
  trigger: string;
  body: string;
}

export interface Suggestion {
  heard: string;
  term: string;
}

export interface UploadEvent {
  at: string;
  kind: string;
  detail: string;
}

export const api = {
  status: () => invoke<Status>("get_status"),
  settings: () => invoke<Settings>("get_settings"),
  setSetting: (key: string, value: string) => invoke<void>("set_setting", { key, value }),
  dictations: (limit = 200) => invoke<Dictation[]>("list_dictations", { limit }),
  deleteDictation: (id: string) => invoke<void>("delete_dictation", { id }),
  copy: (text: string) => invoke<void>("copy_text", { text }),
  checkUpdate: () => invoke<UpdateInfo>("check_update"),
  openReleasePage: (url: string | null) => invoke<void>("open_release_page", { url }),
  models: () => invoke<Model[]>("list_models"),
  downloadModel: (id: string) => invoke<void>("download_model", { id }),
  deleteModel: (id: string) => invoke<void>("delete_model", { id }),
  ai: () => invoke<AiStatus>("get_ai"),
  insights: () => invoke<Insights>("get_insights"),
  connectGroq: (key: string) => invoke<string>("connect_groq", { key }),
  setAiEnabled: (on: boolean) => invoke<void>("set_ai_enabled", { on }),
  setAi: (provider: string, model: string, baseUrl: string, key: string | null, consent: boolean) =>
    invoke<void>("set_ai", { provider, model, baseUrl, key, consent }),
  forgetAiKey: (provider: string) => invoke<void>("forget_ai_key", { provider }),
  testAi: () => invoke<{ ok: boolean; ms: number; message: string }>("test_ai"),
  uploads: () => invoke<UploadEvent[]>("list_uploads"),
  terms: () => invoke<Term[]>("list_terms"),
  addTerm: (term: string, variants: string[], autoReplace = true) =>
    invoke<void>("add_term", { term, variants, autoReplace }),
  deleteTerm: (id: string) => invoke<void>("delete_term", { id }),
  snippets: () => invoke<Snippet[]>("list_snippets"),
  addSnippet: (trigger: string, body: string) => invoke<void>("add_snippet", { trigger, body }),
  deleteSnippet: (id: string) => invoke<void>("delete_snippet", { id }),
  lastDictationId: () => invoke<string | null>("last_dictation_id"),
  fixDictation: (id: string, text: string) => invoke<Suggestion[]>("fix_dictation", { id, text }),
  learnTerms: (items: [string, string][]) => invoke<void>("learn_terms", { items }),
  setCloudAsr: (on: boolean, consent: boolean) => invoke<void>("set_cloud_asr", { on, consent }),
  micStatus: () => invoke<{ open: boolean; level: number }>("mic_status"),
  diagnosticsInfo: () => invoke<{ enabled: boolean; path: string | null; bytes: number }>("diagnostics_info"),
  exportDiagnostics: () => invoke<string>("export_diagnostics"),
  deleteLogs: () => invoke<void>("delete_logs"),
  revealFile: (path: string) => invoke<void>("reveal_file", { path }),
};

export interface Segment {
  id: string;
  start_ms: number;
  end_ms: number;
  speaker: string;
  text: string;
}

export interface MeetingLive {
  recording: boolean;
  seconds: number;
  mic_level: number;
  system_level: number;
  problems: string[];
  mic_device: string;
  system_device: string;
  live: { segments: Segment[]; mode: "starting" | "live" | "after"; error: string | null };
}

export type Check = "linked" | "quoted" | "needs_review";

export interface Point {
  text: string;
  evidence: string[];
  quote?: string | null;
  check?: Check;
  note?: string | null;
}

export interface Summary {
  overview: { title: string; date: string | null; duration_min: number | null; participants: string[]; purpose: string | null };
  executive_summary: Point[];
  topics: { title: string; narrative: Point | null; points: Point[] }[];
  decisions: { text: string; status: "confirmed" | "proposed"; evidence: string[]; quote?: string | null; check?: Check; note?: string | null }[];
  action_items: {
    action: string;
    owner: string | null;
    deadline: string | null;
    evidence: string[];
    quote?: string | null;
    check?: Check;
    deadline_tentative?: boolean;
    note?: string | null;
  }[];
  risks: Point[];
  open_questions: Point[];
  numbers: { value: string; context: string; evidence: string[] }[];
  next_steps: Point[];
  takeaway: Point | null;
}

export interface ViewSegment extends Segment {
  original: string;
  edited: boolean;
}

export interface MeetingView {
  id: string;
  title: string;
  started_at: string;
  duration_ms: number;
  status: "summarised" | "transcribed" | "interrupted" | string;
  has_audio: boolean;
  segments: ViewSegment[];
  thoughts: [number, string][];
  summary: Summary | null;
  dropped: string[];
  flags: string[];
  markdown: string;
  note: string | null;
  problems: string[];
}

export interface MeetingRow {
  id: string;
  title: string;
  started_at: string;
  duration_ms: number;
  status: string;
  has_audio: boolean;
  has_summary: boolean;
  segments: number;
}

export interface Devices {
  inputs: string[];
  outputs: string[];
  default_input: string | null;
  default_output: string | null;
}

export const meetings = {
  devices: () => invoke<Devices>("list_audio_devices"),
  start: (mic: boolean, system: boolean, consent: boolean, micDevice: string, systemDevice: string) =>
    invoke<void>("start_meeting", { mic, system, consent, micDevice, systemDevice }),
  status: () => invoke<MeetingLive | null>("meeting_status"),
  stop: (title: string, thoughts: [number, string][]) => invoke<MeetingView>("stop_meeting", { title, thoughts }),
  list: (query = "") => invoke<MeetingRow[]>("list_meetings", { query }),
  get: (id: string) => invoke<MeetingView>("get_meeting", { id }),
  rename: (id: string, title: string) => invoke<MeetingView>("rename_meeting", { id, title }),
  editLine: (id: string, line: string, text: string) => invoke<MeetingView>("edit_meeting_line", { id, line, text }),
  rewriteNotes: (id: string) => invoke<MeetingView>("rewrite_meeting_notes", { id }),
  transcribeAgain: (id: string) => invoke<MeetingView>("transcribe_saved_meeting", { id }),
  exportNotes: (id: string, format: "md" | "txt") => invoke<string>("export_meeting", { id, format }),
  remove: (id: string) => invoke<void>("delete_meeting", { id }),
  removeAll: () => invoke<number>("delete_all_meetings"),
};

/** Tauri rejects with a plain string; show it as-is. */
export const errorText = (e: unknown) => (typeof e === "string" ? e : e instanceof Error ? e.message : String(e));

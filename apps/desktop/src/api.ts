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
  models: () => invoke<Model[]>("list_models"),
  downloadModel: (id: string) => invoke<void>("download_model", { id }),
  deleteModel: (id: string) => invoke<void>("delete_model", { id }),
  ai: () => invoke<AiStatus>("get_ai"),
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
  micStatus: () => invoke<{ open: boolean; level: number }>("mic_status"),
};

/** Tauri rejects with a plain string; show it as-is. */
export const errorText = (e: unknown) => (typeof e === "string" ? e : e instanceof Error ? e.message : String(e));

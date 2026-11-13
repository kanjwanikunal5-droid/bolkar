import { invoke } from "@tauri-apps/api/core";

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
};

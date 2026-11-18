// Groq, called directly from the browser with the user's own key.
const BASE = "https://api.groq.com/openai/v1";
export const SPEECH_MODEL = "whisper-large-v3";
const TEXT_MODELS = ["qwen/qwen3.8-27b", "openai/gpt-oss-20b", "openai/gpt-oss-120b"];

async function check(res: Response) {
  if (res.ok) return res;
  if (res.status === 401 || res.status === 403) throw new Error("Groq refused the key. Check it in Settings.");
  if (res.status === 429) throw new Error("Groq's free limit is reached for the moment. Try again in a minute.");
  throw new Error(`Groq error ${res.status}`);
}

export async function transcribe(key: string, wav: Blob, language: string | null, prompt: string): Promise<string> {
  const form = new FormData();
  form.append("file", wav, "dictation.wav");
  form.append("model", SPEECH_MODEL);
  form.append("temperature", "0");
  form.append("response_format", "json");
  if (language) form.append("language", language);
  if (prompt) form.append("prompt", prompt);
  const res = await check(await fetch(`${BASE}/audio/transcriptions`, { method: "POST", headers: { Authorization: `Bearer ${key}` }, body: form }));
  const data = (await res.json().catch(() => null)) as { text?: unknown } | null;
  if (!data || typeof data.text !== "string") throw new Error("Groq sent an answer that could not be read.");
  return data.text.trim();
}

/** The chosen text model, remembered per key (a different key may see other models). */
let chosen: { key: string; model: string } | null = null;

async function textModel(key: string): Promise<string> {
  if (chosen && chosen.key === key) return chosen.model;
  const res = await check(await fetch(`${BASE}/models`, { headers: { Authorization: `Bearer ${key}` } }));
  const data = (await res.json().catch(() => null)) as { data?: { id?: unknown }[] } | null;
  const ids = Array.isArray(data?.data) ? data.data.map((m) => String(m.id)) : [];
  const model = TEXT_MODELS.find((m) => ids.includes(m));
  if (!model) throw new Error("No suitable Groq text model is available.");
  chosen = { key, model };
  return model;
}

/** Forget the remembered model (after the key is changed or removed). */
export function resetModelChoice() {
  chosen = null;
}

export async function chat(key: string, system: string, user: string, timeoutMs: number): Promise<string> {
  const model = await textModel(key);
  const body: Record<string, unknown> = {
    model,
    temperature: 0,
    messages: [
      { role: "system", content: system },
      { role: "user", content: user },
    ],
  };
  if (model.includes("qwen3")) Object.assign(body, { reasoning_effort: "none", reasoning_format: "hidden" });
  if (model.startsWith("openai/gpt-oss")) body.reasoning_effort = "low";
  const ctrl = new AbortController();
  const timer = setTimeout(() => ctrl.abort(), timeoutMs);
  try {
    const res = await check(
      await fetch(`${BASE}/chat/completions`, {
        method: "POST",
        headers: { Authorization: `Bearer ${key}`, "Content-Type": "application/json" },
        body: JSON.stringify(body),
        signal: ctrl.signal,
      }),
    );
    const data = (await res.json().catch(() => null)) as { choices?: { message?: { content?: unknown } }[] } | null;
    const content = data?.choices?.[0]?.message?.content;
    if (typeof content !== "string") throw new Error("Groq sent an empty or unreadable answer.");
    return content;
  } finally {
    clearTimeout(timer);
  }
}

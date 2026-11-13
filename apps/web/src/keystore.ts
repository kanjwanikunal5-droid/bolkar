// The user's API key, encrypted with a passphrase (PBKDF2 + AES-GCM) and kept only in this
// browser's IndexedDB. Nothing is ever sent anywhere except to the provider itself.

const DB = "bolkar";
const STORE = "secrets";

function idb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB, 1);
    req.onupgradeneeded = () => req.result.createObjectStore(STORE);
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

async function put(key: string, value: unknown) {
  const db = await idb();
  await new Promise<void>((resolve, reject) => {
    const tx = db.transaction(STORE, "readwrite");
    tx.objectStore(STORE).put(value, key);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

async function get<T>(key: string): Promise<T | undefined> {
  const db = await idb();
  return new Promise((resolve, reject) => {
    const req = db.transaction(STORE).objectStore(STORE).get(key);
    req.onsuccess = () => resolve(req.result as T | undefined);
    req.onerror = () => reject(req.error);
  });
}

async function derive(passphrase: string, salt: BufferSource) {
  const base = await crypto.subtle.importKey("raw", new TextEncoder().encode(passphrase), "PBKDF2", false, ["deriveKey"]);
  return crypto.subtle.deriveKey(
    { name: "PBKDF2", salt, iterations: 250_000, hash: "SHA-256" },
    base,
    { name: "AES-GCM", length: 256 },
    false,
    ["encrypt", "decrypt"],
  );
}

interface Sealed {
  salt: Uint8Array;
  iv: Uint8Array;
  data: ArrayBuffer;
}

export async function saveKey(apiKey: string, passphrase: string) {
  const salt = crypto.getRandomValues(new Uint8Array(16));
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const data = await crypto.subtle.encrypt({ name: "AES-GCM", iv }, await derive(passphrase, salt), new TextEncoder().encode(apiKey));
  await put("groq", { salt, iv, data } satisfies Sealed);
}

export async function hasKey() {
  return (await get<Sealed>("groq")) !== undefined;
}

/** Returns the key, or throws if the passphrase is wrong. */
export async function unlockKey(passphrase: string): Promise<string> {
  const sealed = await get<Sealed>("groq");
  if (!sealed) throw new Error("No key saved yet.");
  try {
    const plain = await crypto.subtle.decrypt({ name: "AES-GCM", iv: sealed.iv as BufferSource }, await derive(passphrase, sealed.salt as BufferSource), sealed.data);
    return new TextDecoder().decode(plain);
  } catch {
    throw new Error("Wrong passphrase.");
  }
}

export async function forgetKey() {
  await put("groq", undefined);
}

export interface HistoryItem {
  at: string;
  raw: string;
  text: string;
  how: string;
}

export async function loadHistory(): Promise<HistoryItem[]> {
  return (await get<HistoryItem[]>("history")) ?? [];
}

export async function addHistory(item: HistoryItem) {
  const all = await loadHistory();
  await put("history", [item, ...all].slice(0, 200));
}

export async function clearHistory() {
  await put("history", []);
}

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

/** Removes the records themselves (not just their values), in one transaction. */
async function del(...keys: string[]) {
  const db = await idb();
  await new Promise<void>((resolve, reject) => {
    const tx = db.transaction(STORE, "readwrite");
    for (const k of keys) tx.objectStore(STORE).delete(k);
    tx.oncomplete = () => resolve();
    tx.onerror = () => reject(tx.error);
  });
}

/** Keys of the records in the store (for checks and tests). */
export async function storedKeys(): Promise<string[]> {
  const db = await idb();
  return new Promise((resolve, reject) => {
    const req = db.transaction(STORE).objectStore(STORE).getAllKeys();
    req.onsuccess = () => resolve(req.result.map(String));
    req.onerror = () => reject(req.error);
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

/**
 * "Remember on this device": the API key is encrypted with an AES key that the browser
 * creates as non-extractable and keeps in IndexedDB. No passphrase to type, and the raw
 * key is never stored in readable form. (A passphrase remains available for shared devices.)
 *
 * What this protects and what it does not: it keeps the key unreadable at rest (someone
 * looking at the browser's stored data sees only encrypted bytes). It does NOT protect
 * against malicious script running inside this web app's own page (for example through a
 * cross-site-scripting bug): such script could ask the browser to decrypt with the same
 * device key. That is why the page loads no third-party scripts and has a strict
 * Content-Security-Policy. On a shared computer, use the passphrase option.
 */
export async function saveKeyOnDevice(apiKey: string) {
  const deviceKey = await crypto.subtle.generateKey({ name: "AES-GCM", length: 256 }, false, ["encrypt", "decrypt"]);
  const iv = crypto.getRandomValues(new Uint8Array(12));
  const data = await crypto.subtle.encrypt({ name: "AES-GCM", iv }, deviceKey, new TextEncoder().encode(apiKey));
  await put("devicekey", deviceKey);
  await put("groq", { salt: new Uint8Array(0), iv, data, device: true });
}

/** "device" (opens without a passphrase), "passphrase", or null when no key is saved. */
export async function keyKind(): Promise<"device" | "passphrase" | null> {
  const sealed = await get<Sealed & { device?: boolean }>("groq");
  if (!sealed) return null;
  return sealed.device ? "device" : "passphrase";
}

export async function unlockOnDevice(): Promise<string> {
  const sealed = await get<Sealed>("groq");
  const deviceKey = await get<CryptoKey>("devicekey");
  if (!sealed || !deviceKey) throw new Error("No key saved yet.");
  const plain = await crypto.subtle.decrypt({ name: "AES-GCM", iv: sealed.iv as BufferSource }, deviceKey, sealed.data);
  return new TextDecoder().decode(plain);
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

/** Deletes the encrypted key and the device key records from IndexedDB. */
export async function forgetKey() {
  await del("groq", "devicekey");
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

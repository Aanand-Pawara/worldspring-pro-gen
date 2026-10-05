// This browser's world storage (IndexedDB): saved worlds, each world's current edits (kept
// here rather than in share links once they grow), and uploaded pictures (see assets.ts).
import type { Edits, WorldFile } from '../gen/protocol';

export interface SavedWorld {
  id: string;
  name: string;
  file: WorldFile;
  savedAt: number;
}

const DB = 'pfm'; // pre-Worldspring name, kept so saved worlds survive the rename
const STORE = 'worlds';
let db: Promise<IDBDatabase> | null = null;

/**
 * Open database `name` at `version`, `upgrade` making its stores. Older generators' builds on the
 * site (`versions.ts`) share this browser's databases with the newest, so a database a newer build
 * has already taken to a later version is opened as it is (schema changes must only add stores).
 */
export function openDb(name: string, version: number, upgrade: (d: IDBDatabase) => void): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(name, version);
    req.onupgradeneeded = () => upgrade(req.result);
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => {
      if (req.error?.name !== 'VersionError') return reject(req.error);
      const now = indexedDB.open(name);
      now.onsuccess = () => resolve(now.result);
      now.onerror = () => reject(now.error);
    };
  });
}

function open(): Promise<IDBDatabase> {
  return (db ??= openDb(DB, 2, (d) => {
    const names = d.objectStoreNames;
    if (!names.contains(STORE)) d.createObjectStore(STORE, { keyPath: 'id' });
    if (!names.contains('edits')) d.createObjectStore('edits');
    if (!names.contains('assets')) d.createObjectStore('assets');
  }));
}

export async function tx<T>(mode: IDBTransactionMode, fn: (s: IDBObjectStore) => IDBRequest<T>, store = STORE): Promise<T> {
  const d = await open();
  return new Promise((resolve, reject) => {
    const req = fn(d.transaction(store, mode).objectStore(store));
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

// A world's edits are kept field by field (`<worldKey>|<field>`), so a change stores only the
// fields it touched: a world with thousands of objects doesn't write them all for a rename.
// (Older saves hold the whole edits under `<worldKey>`; they are read, and replaced on the
// next save.)

/** The fields as last stored, by world (edits are never changed in place: the same object is
 * the same field). */
const stored = new Map<string, Record<string, unknown>>();

/** A world's current edits (by `worldKey`), or null. */
export async function loadEdits(key: string): Promise<Edits | null> {
  try {
    const d = await open();
    const range = IDBKeyRange.bound(`${key}|`, `${key}|￿`);
    const [keys, values, whole] = await new Promise<[IDBValidKey[], unknown[], unknown]>((resolve, reject) => {
      const t = d.transaction('edits', 'readonly');
      const s = t.objectStore('edits');
      const k = s.getAllKeys(range);
      const v = s.getAll(range);
      const w = s.get(key);
      t.oncomplete = () => resolve([k.result, v.result, w.result]);
      t.onerror = () => reject(t.error);
    });
    if (!keys.length) return (whole as Edits | undefined) ?? null;
    const e = Object.fromEntries(keys.map((k, i) => [String(k).slice(key.length + 1), values[i]])) as Edits;
    stored.set(key, { ...e });
    return e;
  } catch {
    return null;
  }
}

/** Ask the browser to keep this site's storage under pressure (once, at the first thing worth
 * keeping): everything a visitor makes lives only here. */
let persistAsked = false;
function askPersist() {
  if (persistAsked) return;
  persistAsked = true;
  void navigator.storage?.persist?.().catch(() => {});
}

export async function saveEdits(key: string, edits: Edits): Promise<void> {
  if (Object.keys(edits).length) askPersist();
  try {
    const d = await open();
    const was = stored.get(key);
    const now = edits as Record<string, unknown>;
    await new Promise<void>((resolve, reject) => {
      const t = d.transaction('edits', 'readwrite');
      const s = t.objectStore('edits');
      if (!was) {
        // First save here: the old whole record and any fields stored before go.
        s.delete(key);
        s.delete(IDBKeyRange.bound(`${key}|`, `${key}|￿`));
      }
      for (const f of new Set([...Object.keys(was ?? {}), ...Object.keys(now)])) {
        if (was && was[f] === now[f]) continue;
        if (now[f] === undefined) s.delete(`${key}|${f}`);
        else s.put(now[f], `${key}|${f}`);
      }
      t.oncomplete = () => resolve();
      t.onerror = () => reject(t.error);
    });
    stored.set(key, { ...now });
  } catch (e) {
    console.warn('[library] edits not saved', e);
  }
}

export async function listWorlds(): Promise<SavedWorld[]> {
  try {
    const all = await tx<SavedWorld[]>('readonly', (s) => s.getAll() as IDBRequest<SavedWorld[]>);
    return all.sort((a, b) => b.savedAt - a.savedAt);
  } catch {
    return [];
  }
}

export async function saveWorld(name: string, file: WorldFile): Promise<void> {
  askPersist();
  const id = `${Date.now().toString(36)}-${Math.random().toString(36).slice(2, 8)}`;
  await tx('readwrite', (s) => s.put({ id, name, file, savedAt: Date.now() } satisfies SavedWorld));
}

export async function deleteWorld(id: string): Promise<void> {
  await tx('readwrite', (s) => s.delete(id));
}

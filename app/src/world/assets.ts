// Uploaded pictures (custom sprites, portraits), by content hash: kept in this browser
// (IndexedDB) and, when mapd is running, on its disk too, so agents and other browsers on this
// machine see them. The world file refers to them by id only.
import type { Edits } from '../gen/protocol';
import { mapdHttp } from '../sync/mapd';
import { tx } from './library';

export interface Asset {
  blob: Blob;
  name: string;
}

/** A content hash (hex): SHA-256 where the page may use it, else a 64-bit FNV-style hash. */
async function hashOf(bytes: ArrayBuffer): Promise<string> {
  if (globalThis.crypto?.subtle) {
    const d = new Uint8Array(await crypto.subtle.digest('SHA-256', bytes));
    return [...d.subarray(0, 16)].map((b) => b.toString(16).padStart(2, '0')).join('');
  }
  const b = new Uint8Array(bytes);
  let h1 = 0x811c9dc5;
  let h2 = 0x9747b28c;
  for (let i = 0; i < b.length; i++) {
    h1 = Math.imul(h1 ^ b[i], 0x01000193);
    h2 = Math.imul(h2 ^ b[i], 0x5bd1e995) ^ (h2 >>> 13);
  }
  return (h1 >>> 0).toString(16).padStart(8, '0') + (h2 >>> 0).toString(16).padStart(8, '0');
}

/** Largest picture taken in (bytes): a bigger one can exhaust a tab's memory as it is decoded. */
export const MAX_PICTURE_BYTES = 20 * 1024 * 1024;
/** Longest side (px) a picture is kept at. */
const MAX_PICTURE_SIDE = 2048;

/** A picture fit to keep: refused over `MAX_PICTURE_BYTES`, drawn down to `MAX_PICTURE_SIDE` if larger
 * (as WebP; one the browser can't decode, such as some SVGs, is kept as it is). */
export async function fitPicture(blob: Blob): Promise<Blob> {
  if (blob.size > MAX_PICTURE_BYTES) throw new Error(`That picture is too big (${Math.round(MAX_PICTURE_BYTES / 1048576)} MB at most)`);
  let bmp: ImageBitmap;
  try {
    bmp = await createImageBitmap(blob);
  } catch {
    return blob;
  }
  try {
    const k = MAX_PICTURE_SIDE / Math.max(bmp.width, bmp.height);
    if (k >= 1 || typeof OffscreenCanvas === 'undefined') return blob;
    const c = new OffscreenCanvas(Math.max(1, Math.round(bmp.width * k)), Math.max(1, Math.round(bmp.height * k)));
    c.getContext('2d')!.drawImage(bmp, 0, 0, c.width, c.height);
    return await c.convertToBlob({ type: 'image/webp', quality: 0.9 });
  } finally {
    bmp.close();
  }
}

const memo = new Map<string, Promise<Asset | null>>();

/** Keep a picture (`fitPicture`); returns its id. */
export async function putAsset(picture: Blob, name = ''): Promise<string> {
  const blob = await fitPicture(picture);
  const id = await hashOf(await blob.arrayBuffer());
  await tx('readwrite', (s) => s.put({ blob, name } satisfies Asset, id), 'assets');
  memo.set(id, Promise.resolve({ blob, name }));
  void upload(id, blob);
  return id;
}

/** A picture by id: from this browser, else from mapd (then kept here). */
export function getAsset(id: string): Promise<Asset | null> {
  let p = memo.get(id);
  if (!p) {
    p = (async () => {
      try {
        const local = (await tx('readonly', (s) => s.get(id), 'assets')) as Asset | undefined;
        if (local) return local;
      } catch {
        // No IndexedDB (private mode): mapd may still have it.
      }
      const base = mapdHttp();
      if (!base) return null;
      try {
        const r = await fetch(`${base}/assets/${id}`);
        if (!r.ok) return null;
        const a: Asset = { blob: await r.blob(), name: '' };
        await tx('readwrite', (s) => s.put(a, id), 'assets').catch(() => {});
        return a;
      } catch {
        return null;
      }
    })();
    memo.set(id, p);
    // A miss is tried again next time (mapd may start later).
    void p.then((a) => a ?? memo.delete(id));
  }
  return p;
}

/** Give mapd the pictures these ids name that it lacks (from this browser). */
export async function shareAssets(ids: Iterable<string>) {
  const base = mapdHttp();
  if (!base) return;
  for (const id of ids) {
    try {
      if ((await fetch(`${base}/assets/${id}`, { method: 'HEAD' })).ok) continue;
      const a = await getAsset(id);
      if (a) await upload(id, a.blob);
    } catch {
      return;
    }
  }
}

async function upload(id: string, blob: Blob) {
  const base = mapdHttp();
  if (!base) return;
  try {
    await fetch(`${base}/assets/${id}`, { method: 'PUT', body: blob });
  } catch {
    // mapd not running: the picture stays in this browser; shareAssets sends it later.
  }
}

const urls = new Map<string, Promise<string | null>>();

/** An object URL for a picture (made once per id), or null if it can't be found. */
export function assetUrl(id: string): Promise<string | null> {
  let p = urls.get(id);
  if (!p) {
    p = getAsset(id).then((a) => (a ? URL.createObjectURL(a.blob) : null));
    urls.set(id, p);
    void p.then((u) => u ?? urls.delete(id));
  }
  return p;
}

/** Keep a picture under the id a world file gives it (pictures bundled in an imported file). */
export async function keepAsset(id: string, blob: Blob, name = '') {
  if (!/^[0-9a-f]{16,64}$/.test(id)) return;
  await tx('readwrite', (s) => s.put({ blob, name } satisfies Asset, id), 'assets').catch(() => {});
  memo.set(id, Promise.resolve({ blob, name }));
  void upload(id, blob);
}

/** The pictures a world's edits refer to (NPC portraits, uploaded sprites and those placed). */
export function assetIds(e: Edits | undefined): string[] {
  const placed = Object.values(e?.objects ?? {}).flatMap((o) => (typeof o.kind === 'string' && o.kind.startsWith('s:') ? [o.kind.slice(2)] : []));
  return [...new Set([...Object.values(e?.npcs ?? {}).flatMap((n) => (n.portrait ? [n.portrait] : [])), ...Object.keys(e?.sprites ?? {}), ...placed])];
}

/** Pictures as data URLs by id, for a world file that carries its own. */
export async function bundleAssets(ids: string[]): Promise<Record<string, { name: string; data: string }>> {
  const out: Record<string, { name: string; data: string }> = {};
  for (const id of ids) {
    const a = await getAsset(id);
    if (!a) continue;
    const data = await new Promise<string>((resolve, reject) => {
      const r = new FileReader();
      r.onload = () => resolve(String(r.result));
      r.onerror = () => reject(r.error);
      r.readAsDataURL(a.blob);
    });
    out[id] = { name: a.name, data };
  }
  return out;
}

/** Keep the pictures a world file carries (`bundleAssets`). */
export async function unbundleAssets(bundle: unknown) {
  if (!bundle || typeof bundle !== 'object') return;
  for (const [id, v] of Object.entries(bundle as Record<string, { name?: string; data?: string }>)) {
    // (Base64 is 4/3 the size of the picture.)
    if (typeof v?.data !== 'string' || !v.data.startsWith('data:image/') || v.data.length > (MAX_PICTURE_BYTES * 4) / 3 + 100) continue;
    try {
      await keepAsset(id, await (await fetch(v.data)).blob(), v.name ?? '');
    } catch {
      // A broken picture: the rest still load.
    }
  }
}

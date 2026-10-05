// Uploaded sprites as textures for battlemap chunks, by asset id: loaded from the asset store
// on first use, drawn down to at most `MAX_PX` on their long side (pictures straight from an
// agent may be large), one texture each. Chunks that asked for one still loading are told
// when it is ready (`onLoaded`) and are built again.
import { Texture } from 'pixi.js';
import { assetUrl } from '../world/assets';

/** Long side of a sprite's texture (px): 64 px a square, up to eight squares across. */
const MAX_PX = 512;

const textures = new Map<string, Texture>();
const loading = new Set<string>();
const listeners = new Set<(id: string) => void>();

/** Calls `f(id)` whenever a sprite's texture becomes ready (or failed: a stand-in then). */
export function onLoaded(f: (id: string) => void): () => void {
  listeners.add(f);
  return () => listeners.delete(f);
}

/** A sprite's texture, or null while it loads (`onLoaded` says when it is ready). */
export function customTexture(id: string): Texture | null {
  const t = textures.get(id);
  if (t) return t;
  if (!loading.has(id)) {
    loading.add(id);
    void load(id).then((tex) => {
      textures.set(id, tex);
      loading.delete(id);
      for (const f of listeners) f(id);
    });
  }
  return null;
}

/** A picture as an image, drawn down to `max` px on its long side (a canvas). */
export async function shrink(src: Blob | string, max = MAX_PX): Promise<HTMLCanvasElement> {
  const url = typeof src === 'string' ? src : URL.createObjectURL(src);
  try {
    const img = new Image();
    img.decoding = 'async';
    img.src = url;
    await img.decode();
    const w0 = img.naturalWidth || max;
    const h0 = img.naturalHeight || max;
    const k = Math.min(1, max / Math.max(w0, h0));
    const c = document.createElement('canvas');
    c.width = Math.max(1, Math.round(w0 * k));
    c.height = Math.max(1, Math.round(h0 * k));
    const g = c.getContext('2d')!;
    g.imageSmoothingQuality = 'high';
    g.drawImage(img, 0, 0, c.width, c.height);
    return c;
  } finally {
    if (typeof src !== 'string') URL.revokeObjectURL(url);
  }
}

async function load(id: string): Promise<Texture> {
  try {
    const url = await assetUrl(id);
    if (url) return Texture.from(await shrink(url));
  } catch {
    // A broken picture: the stand-in below.
  }
  return standIn();
}

let missing: Texture | null = null;

/** A sprite whose picture can't be found: a dashed disc with a question mark. */
function standIn(): Texture {
  if (missing) return missing;
  const c = document.createElement('canvas');
  c.width = c.height = 64;
  const g = c.getContext('2d')!;
  g.fillStyle = 'rgba(120, 96, 70, 0.55)';
  g.strokeStyle = '#2b241d';
  g.lineWidth = 3;
  g.setLineDash([6, 4]);
  g.beginPath();
  g.arc(32, 32, 28, 0, Math.PI * 2);
  g.fill();
  g.stroke();
  g.fillStyle = '#f5ecd6';
  g.font = 'bold 34px Georgia, serif';
  g.textAlign = 'center';
  g.textBaseline = 'middle';
  g.fillText('?', 32, 34);
  missing = Texture.from(c);
  return missing;
}

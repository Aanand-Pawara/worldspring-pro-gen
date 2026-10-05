// Sketch previews off the main thread: the world's terrain stages on a coarse grid (WASM
// `sketch_preview`), as an image plus the sketch's conflicts. Only the latest request is
// answered; older ones still queued are skipped.
import init, { sketch_preview } from '../gen/pkg/worldgen_wasm.js';
import type { Conflict } from '../gen/protocol';

export interface PreviewRequest {
  id: number;
  worldJson: string;
  width: number;
}

export type PreviewReply =
  | { id: number; ok: true; w: number; h: number; rgba: Uint8ClampedArray<ArrayBuffer>; conflicts: Conflict[]; ms: number }
  | { id: number; ok: false; error: string };

let ready: Promise<unknown> | null = null;
let latest = 0;

self.onmessage = async (e: MessageEvent<PreviewRequest>) => {
  const m = e.data;
  latest = m.id;
  await (ready ??= init());
  if (m.id !== latest) return;
  const post = (r: PreviewReply, transfer: Transferable[] = []) => (self as unknown as Worker).postMessage(r, transfer);
  try {
    const start = performance.now();
    const out = sketch_preview(m.worldJson, m.width);
    const dv = new DataView(out.buffer, out.byteOffset, out.byteLength);
    const [w, h, n] = [dv.getUint32(0, true), dv.getUint32(4, true), dv.getUint32(8, true)];
    const conflicts = JSON.parse(new TextDecoder().decode(out.subarray(12, 12 + n))) as Conflict[];
    const rgba = new Uint8ClampedArray(out.slice(12 + n, 12 + n + w * h * 4).buffer);
    post({ id: m.id, ok: true, w, h, rgba, conflicts, ms: performance.now() - start }, [rgba.buffer]);
  } catch (err) {
    post({ id: m.id, ok: false, error: String(err) });
  }
};

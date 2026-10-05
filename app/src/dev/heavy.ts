// Stress bench (?bench=heavy): how a world bears a lot of customisation. The same steps run
// twice, on the world as generated and then loaded with: designed dungeons, tens of thousands
// of objects put down and cleared, uploaded sprites, NPCs, plot points, notes, renames and
// created sites. Each time: small edits timed (a rename, a brush stroke of objects, a dungeon
// design saved) and a pan over a battlemap thick with objects and sprites while a rename lands
// every 2 s. The world's edits are put back afterwards, and the bench's pictures removed. It
// only runs on a world with no edits (so nothing of the user's is ever at stake).
import { runBench, type BenchResult } from './bench';
import type { MapView } from '../render/MapView';
import type { Created, Edits, Npc, Placed, Plot, SiteDesign } from '../gen/protocol';
import { putAsset } from '../world/assets';
import { tx } from '../world/library';
import { SETTLEMENT_KINDS } from '../ui/gazetteer';

/** How much the heavy world holds. */
const HEAVY = { designs: 50, objects: 20000, clears: 2000, sprites: 40, npcs: 2000, plots: 500, notes: 2000, renames: 1000, created: 30 };
/** Side (ft) of the square about the focus that the objects fill (about 6 x 6 battlemap chunks). */
const SPREAD_FT = 3600;
const BATTLEMAP_ZOOM = Math.log2(64 / 5);

/** One kind of edit, timed: on the main thread (`sync`), to the next frame drawn (`frame`), and
 * until a generator worker has taken the edits in and answers (`worker`); median and worst. */
export interface EditTiming {
  syncMs: [number, number];
  frameMs: [number, number];
  workerMs: [number, number];
}

export interface HeavyResult {
  error?: string;
  holds: typeof HEAVY & { editsKB: number; designsKB: number };
  /** Making the heavy world's edits and putting them in (one change), until the map settled. */
  loadMs: { build: number; apply: number; settle: number };
  heapMB: { empty: number; heavy: number };
  edits: { empty: Record<string, EditTiming>; heavy: Record<string, EditTiming> };
  /** Generating an underground site: as generated, and built from its design. */
  siteMs: { generated: number; designed: number };
  /** The pan: fps and frame times, and the generators' mean ms per job meanwhile. */
  pan: { empty: PanStats; heavy: PanStats };
}

interface PanStats {
  avgFps: number;
  low1Fps: number;
  p95Ms: number;
  maxMs: number;
  jobMs: number;
  /** The slowest frames and what the frame before was doing (`BenchResult.slowFrames`). */
  slowest?: BenchResult['slowFrames'];
}

const median = (v: number[]) => [...v].sort((a, b) => a - b)[Math.floor(v.length / 2)] ?? 0;
const r1 = (v: number) => Math.round(v * 10) / 10;
const frame = () => new Promise<void>((r) => requestAnimationFrame(() => r()));
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
const heapMB = () => r1(((performance as unknown as { memory?: { usedJSHeapSize: number } }).memory?.usedJSHeapSize ?? 0) / 2 ** 20);

export async function runHeavyBench(view: MapView, edits: () => Edits, apply: (e: Edits) => void): Promise<HeavyResult> {
  const publish = (r: HeavyResult) => {
    console.log('[bench]', JSON.stringify(r));
    (window as unknown as { __benchResult: HeavyResult }).__benchResult = r;
    return r;
  };
  const empty = (): HeavyResult => ({ holds: { ...HEAVY, editsKB: 0, designsKB: 0 }, loadMs: { build: 0, apply: 0, settle: 0 }, heapMB: { empty: 0, heavy: 0 }, edits: { empty: {}, heavy: {} }, siteMs: { generated: 0, designed: 0 }, pan: { empty: blankPan(), heavy: blankPan() } });
  if (Object.keys(edits()).length) return publish({ ...empty(), error: 'The stress bench runs on a world with no edits: open a fresh seed (or start over) first.' });
  await settle(view, 8000);
  const result = empty();
  const [fx, fy] = focus(view);
  const assets: string[] = [];
  try {
    // --- As generated.
    const sites = await designable(view, HEAVY.designs + 1);
    result.edits.empty = await timeEdits(view, edits, apply, sites[0], fx, fy);
    result.heapMB.empty = heapMB();
    result.pan.empty = await pan(view, edits, apply, fx, fy);
    // --- Loaded.
    const t0 = performance.now();
    const heavy = await build(view, sites.slice(1), fx, fy, assets);
    result.loadMs.build = Math.round(performance.now() - t0);
    const t1 = performance.now();
    apply(heavy);
    result.loadMs.apply = Math.round(performance.now() - t1);
    await settle(view, 30000);
    result.loadMs.settle = Math.round(performance.now() - t1);
    result.holds.editsKB = Math.round(JSON.stringify(heavy).length / 1024);
    result.holds.designsKB = Math.round(JSON.stringify(heavy.designs).length / 1024);
    result.heapMB.heavy = heapMB();
    result.edits.heavy = await timeEdits(view, edits, apply, sites[0], fx, fy);
    // A site underground generated, and one built from its design.
    let t = performance.now();
    await view.gen.interior(sites[0]);
    result.siteMs.generated = r1(performance.now() - t);
    t = performance.now();
    await view.gen.interior(sites[1]);
    result.siteMs.designed = r1(performance.now() - t);
    result.pan.heavy = await pan(view, edits, apply, fx, fy);
  } finally {
    apply({});
    for (const id of assets) await tx('readwrite', (s) => s.delete(id), 'assets').catch(() => {});
  }
  return publish(result);
}

function blankPan(): PanStats {
  return { avgFps: 0, low1Fps: 0, p95Ms: 0, maxMs: 0, jobMs: 0 };
}

/** Wait until the map has everything in view (or `ms` passed). */
async function settle(view: MapView, ms: number) {
  const start = performance.now();
  await sleep(300);
  while (view.readiness < 1 && performance.now() - start < ms) await sleep(100);
}

/** High ground near the middle of the map: a battlemap with plenty of scatter. */
function focus(view: MapView): [number, number] {
  const big = view.overlay?.features.find((f) => f.kind === 'town') ?? view.overlay?.features[0];
  const g = view.geom!;
  return big ? [big.x + 2500, big.y + 1500] : [g.map_w_ft / 2, g.map_h_ft / 2];
}

/** Underground sites that can be designed, the first `n`: under the generated ruins, caves,
 * mines and lava tubes (layouts after the settlements, in the overlay's order). */
async function designable(view: MapView, n: number): Promise<string[]> {
  const features = view.overlay?.features ?? [];
  const towns = features.filter((f) => SETTLEMENT_KINDS.includes(f.kind)).length;
  const pois = features.filter((f) => ['ruin', 'tower', 'waystation', 'camp', 'cave', 'mine', 'lava_tube'].includes(f.kind) && !f.id.startsWith('c:'));
  const out: string[] = [];
  for (let j = 0; j < pois.length && out.length < n; j++) {
    if (!['ruin', 'cave', 'mine', 'lava_tube'].includes(pois[j].kind)) continue;
    const id = `u:${towns + j}:0`;
    if (!('error' in (await view.gen.design(id)))) out.push(id);
  }
  return out;
}

/** The heavy world's edits. */
async function build(view: MapView, sites: string[], fx: number, fy: number, assets: string[]): Promise<Edits> {
  let seed = 12345;
  const rnd = () => ((seed = (seed * 1103515245 + 12345) & 0x7fffffff) / 0x7fffffff);
  const words = 'old grey watchful crooked silver hidden tired loyal ambitious wary scarred cheerful'.split(' ');
  const text = (n: number) => Array.from({ length: n }, () => words[Math.floor(rnd() * words.length)]).join(' ');
  const e: Edits = {};
  // Sprites: little pictures drawn here, uploaded as the user would.
  e.sprites = {};
  for (let i = 0; i < HEAVY.sprites; i++) {
    const c = document.createElement('canvas');
    c.width = c.height = 128;
    const g = c.getContext('2d')!;
    g.fillStyle = `hsl(${(i * 37) % 360} 45% 45%)`;
    g.beginPath();
    g.arc(64, 64, 30 + (i % 5) * 6, 0, Math.PI * 2);
    g.fill();
    g.fillStyle = '#f5ecd6';
    g.font = '40px serif';
    g.fillText(String(i), 40, 78);
    const blob = await new Promise<Blob>((r) => c.toBlob((b) => r(b!), 'image/png'));
    const id = await putAsset(blob, `bench sprite ${i}`);
    assets.push(id);
    e.sprites[id] = { name: `Bench sprite ${i}`, size: 1 + (i % 3), cover: i % 3, blocks_move: i % 2 === 0, blocks_sight: false, difficult: false, height_ft: 4 };
  }
  // Objects and clears over a square of battlemaps about the focus.
  e.objects = {};
  const kinds = [1, 2, 8, 12, 14, 20, 25];
  for (let i = 0; i < HEAVY.objects; i++) {
    const kind: Placed['kind'] = i % 5 === 0 ? `s:${assets[i % assets.length]}` : kinds[i % kinds.length];
    e.objects[`o:heavy${i}`] = { kind, x: fx + (rnd() - 0.5) * SPREAD_FT, y: fy + (rnd() - 0.5) * SPREAD_FT, rot: rnd() * 6.28, scale: 1, variant: i & 3 };
  }
  e.cleared = {};
  for (let i = 0; i < HEAVY.clears; i++) e.cleared[`x:heavy${i}`] = { x: fx + (rnd() - 0.5) * SPREAD_FT, y: fy + (rnd() - 0.5) * SPREAD_FT, r: 5 + rnd() * 10 };
  // Designed dungeons: each site copied, doors added (a design the size of a real one).
  e.designs = {};
  for (const id of sites) {
    const r = await view.gen.design(id, undefined, { doors: null });
    if (!('error' in r)) e.designs[id] = r.design as SiteDesign;
  }
  // The notebook.
  const features = view.overlay?.features ?? [];
  e.npcs = {};
  for (let i = 0; i < HEAVY.npcs; i++) {
    const npc: Npc = { name: `Bench ${text(2)} ${i}`, appearance: text(30), mannerisms: text(20), attitude: { stance: 'neutral', text: text(15) }, goals: text(25), notes: text(40), tags: ['bench', text(1)] };
    if (i % 4 === 0 && sites.length) npc.location = { id: sites[i % sites.length], level: 0 };
    e.npcs[`n:heavy${i}`] = npc;
  }
  e.plots = {};
  for (let i = 0; i < HEAVY.plots; i++) {
    const plot: Plot = { title: `Bench plot ${i}`, text: text(60), status: 'idea', anchors: features.length ? [features[i % features.length].id] : [], npcs: [`n:heavy${i}`], tags: ['bench'] };
    e.plots[`p:heavy${i}`] = plot;
  }
  e.notes = {};
  // (On every feature, then on buildings of the first town.)
  for (let i = 0; i < HEAVY.notes; i++) e.notes[i < features.length ? features[i].id : `b:0:${i}`] = { text: text(50), tags: ['bench'] };
  e.renames = {};
  for (let i = 0; i < Math.min(HEAVY.renames, features.length); i++) e.renames[features[i].id] = `Bench ${text(1)} ${i}`;
  // Created sites about the map.
  e.created = [];
  const g = view.geom!;
  for (let i = 0; i < HEAVY.created * 3 && e.created.length < HEAVY.created; i++) {
    const kind = ['ruin', 'camp', 'tower'][i % 3];
    const id = `c:${e.created.length}`;
    const spot = await view.gen.spot(kind, kind === 'ruin' ? 'dungeon' : undefined, id, g.map_w_ft * (0.15 + 0.7 * rnd()), g.map_h_ft * (0.15 + 0.7 * rnd()));
    if (!spot || 'error' in spot) continue;
    const c: Created = { id, kind, x: spot.x, y: spot.y, name: spot.name };
    if (kind === 'ruin') c.under = 'dungeon';
    e.created.push(c);
  }
  return e;
}

/** Small edits timed, five of each: a rename, a brush stroke (40 objects), a design saved. */
async function timeEdits(view: MapView, edits: () => Edits, apply: (e: Edits) => void, site: string, fx: number, fy: number): Promise<Record<string, EditTiming>> {
  const design = await view.gen.design(site);
  const kinds: Record<string, (n: number, e: Edits) => Edits> = {
    rename: (n, e) => ({ ...e, renames: { ...(e.renames ?? {}), [`bench:${n}`]: `Renamed ${n}` } }),
    brush: (n, e) => {
      const objects = { ...(e.objects ?? {}) };
      for (let k = 0; k < 40; k++) objects[`o:stroke${n}_${k}`] = { kind: 2, x: fx + 300 + n * 40 + (k % 8) * 5, y: fy + Math.floor(k / 8) * 5, rot: 0, scale: 1, variant: 0 };
      return { ...e, objects };
    },
    design: (n, e) => {
      if ('error' in design) return e;
      const d = structuredClone(design.design);
      d.levels[d.levels.length - 1].name = `Redesigned ${n}`;
      return { ...e, designs: { ...(e.designs ?? {}), [site]: d } };
    },
  };
  const out: Record<string, EditTiming> = {};
  for (const [name, make] of Object.entries(kinds)) {
    const sync: number[] = [];
    const drawn: number[] = [];
    const worker: number[] = [];
    for (let n = 0; n < 5; n++) {
      // (Made as the app makes edits: new objects only for what changes.)
      const next = make(n, edits());
      await frame();
      const t0 = performance.now();
      apply(next);
      sync.push(performance.now() - t0);
      await frame();
      drawn.push(performance.now() - t0);
      await view.gen.place('nowhere');
      worker.push(performance.now() - t0);
      await sleep(150);
    }
    out[name] = { syncMs: [r1(median(sync)), r1(Math.max(...sync))], frameMs: [r1(median(drawn)), r1(Math.max(...drawn))], workerMs: [r1(median(worker)), r1(Math.max(...worker))] };
  }
  // Those edits taken back out.
  const e = edits();
  const without = <T,>(m: Record<string, T> | undefined, drop: (k: string) => boolean) => Object.fromEntries(Object.entries(m ?? {}).filter(([k]) => !drop(k)));
  apply({ ...e, renames: without(e.renames, (k) => k.startsWith('bench:')), objects: without(e.objects, (k) => k.startsWith('o:stroke')), designs: without(e.designs, (k) => k === site) });
  return out;
}

/** A pan over the objects at battlemap zoom, a rename every 2 s. */
async function pan(view: MapView, edits: () => Edits, apply: (e: Edits) => void, fx: number, fy: number): Promise<PanStats> {
  const z = BATTLEMAP_ZOOM - 1;
  view.cam.set({ cx: fx - 900, cy: fy - 600, zoom: z });
  await settle(view, 15000);
  let last = performance.now();
  let n = 0;
  const jobs: number[] = [];
  const r: BenchResult = await runBench(view, {
    quiet: true,
    tick: (now) => {
      const s = (view as unknown as { genStats: { avgMs: number; jobs: number } | null }).genStats;
      if (s) jobs.push(s.avgMs);
      if (now - last < 2000) return;
      last = now;
      const e = edits();
      apply({ ...e, renames: { ...(e.renames ?? {}), 'bench:pan': `Pan ${n++}` } });
    },
    segs: [
      { hold: 500 },
      { to: { cx: fx + 900, cy: fy - 600, zoom: z }, ms: 6000 },
      { to: { cx: fx + 900, cy: fy + 600, zoom: z + 0.5 }, ms: 4000 },
      { to: { cx: fx - 900, cy: fy + 600, zoom: z }, ms: 6000 },
      { to: { cx: fx, cy: fy, zoom: z - 1.5 }, ms: 3000 },
      { hold: 1000 },
    ],
  });
  const e = edits();
  const { 'bench:pan': _, ...renames } = e.renames ?? {};
  apply({ ...e, renames });
  return { avgFps: r1(r.avgFps), low1Fps: r1(r.low1Fps), p95Ms: r1(r.p95Ms), maxMs: r1(r.maxMs), jobMs: r1(median(jobs)), slowest: r.slowFrames.slice(0, 5) };
}

// The designer's changes to a site design (`SiteDesign`, mirroring
// crates/worldgen/src/under/design.rs): each takes the design and changes it in place. Squares
// are run-length encoded in the design; `Level` holds one level decoded while it changes.
// Walls, doors' rooms and the rules a site must keep are worked out by the generator (Ask op
// `design`), never here.
import type { DesignLevel, SiteDesign } from '../../gen/protocol';

/** Items that are ways: in from the surface, up and down a level. */
export const WAYS = ['exit', 'up', 'down'];
export const BOSS = 'boss chamber';
/** Feet a level lies below the one above. */
const LEVEL_FT = 20;

export type Item = DesignLevel['items'][number];

/** Room per square from runs. */
export function decode(runs: number[], n: number): Int16Array {
  const out = new Int16Array(n).fill(-1);
  let k = 0;
  for (let i = 0; i + 1 < runs.length && k < n; i += 2) {
    out.fill(runs[i], k, Math.min(n, k + runs[i + 1]));
    k += runs[i + 1];
  }
  return out;
}

export function encode(cells: Int16Array): number[] {
  const out: number[] = [];
  for (const c of cells) {
    if (out.length && out[out.length - 2] === c) out[out.length - 1]++;
    else out.push(c, 1);
  }
  return out;
}

/** One level's squares, changed and written back. */
export function withCells(d: SiteDesign, li: number, f: (cells: Int16Array, lv: DesignLevel) => void) {
  const lv = d.levels[li];
  const cells = decode(lv.cells, d.nx * d.ny);
  f(cells, lv);
  lv.cells = encode(cells);
}

/** Whether an item covers square (x, y). */
export const covers = (f: Item, x: number, y: number) => x >= f.x && x < f.x + f.w && y >= f.y && y < f.y + f.h;

/** A new room on a level (always added: an emptied room keeps its index, and its name). */
export function addRoom(d: SiteDesign, li: number, kind: string, raise_ft = 0): number {
  d.levels[li].rooms.push({ kind, raise_ft });
  return d.levels[li].rooms.length - 1;
}

/** Squares given to `room` (-1: rock). Props on squares turned to rock go with them. */
export function paint(d: SiteDesign, li: number, squares: Iterable<number>, room: number) {
  const nx = d.nx;
  const rock: number[] = [];
  withCells(d, li, (cells) => {
    for (const k of squares) {
      if (k < 0 || k >= cells.length) continue;
      cells[k] = room;
      if (room < 0) rock.push(k);
    }
  });
  if (rock.length) {
    const lv = d.levels[li];
    lv.items = lv.items.filter((f) => WAYS.includes(f.kind) || !rock.some((k) => covers(f, k % nx, Math.floor(k / nx))));
  }
}

/** The squares of a rectangle between two corners (inclusive), inside the grid. */
export function rectSquares(d: SiteDesign, x0: number, y0: number, x1: number, y1: number): number[] {
  const out: number[] = [];
  const [a, b] = [Math.max(0, Math.min(x0, x1)), Math.min(d.nx - 1, Math.max(x0, x1))];
  const [c, e] = [Math.max(0, Math.min(y0, y1)), Math.min(d.ny - 1, Math.max(y0, y1))];
  for (let j = c; j <= e; j++) for (let i = a; i <= b; i++) out.push(j * d.nx + i);
  return out;
}

/** A door on an edge: none → door → secret door → none. */
export function cycleDoor(d: SiteDesign, li: number, x: number, y: number, side: number) {
  const lv = d.levels[li];
  const at = lv.doors.findIndex((e) => e[0] === x && e[1] === y && e[2] === side);
  if (at < 0) lv.doors.push([x, y, side, 0]);
  else if (lv.doors[at][3] === 0) lv.doors[at] = [x, y, side, 1];
  else lv.doors.splice(at, 1);
}

/** The prop at a square (not a way), if any. */
export function propAt(d: SiteDesign, li: number, x: number, y: number): number {
  return d.levels[li].items.findIndex((f) => !WAYS.includes(f.kind) && covers(f, x, y));
}

/** Put a prop down where its squares are floor and free; false if they aren't. */
export function addProp(d: SiteDesign, li: number, kind: string, x: number, y: number, w: number, h: number): boolean {
  const lv = d.levels[li];
  if (x < 0 || y < 0 || x + w > d.nx || y + h > d.ny) return false;
  const cells = decode(lv.cells, d.nx * d.ny);
  for (let j = y; j < y + h; j++) for (let i = x; i < x + w; i++) if (cells[j * d.nx + i] < 0 || lv.items.some((f) => covers(f, i, j))) return false;
  lv.items.push({ kind, x, y, w, h });
  return true;
}

/** The way down from level `li` at (x, y), and the way up onto the level below under it (the
 * square there made floor, a landing round it, if it was rock). */
export function setWayDown(d: SiteDesign, li: number, x: number, y: number) {
  if (li <= 0) return;
  const lv = d.levels[li];
  lv.items = lv.items.filter((f) => f.kind !== 'down' && !covers(f, x, y));
  lv.items.push({ kind: 'down', x, y, w: 1, h: 1 });
  const below = d.levels[li - 1];
  below.items = below.items.filter((f) => f.kind !== 'up' && !covers(f, x, y));
  below.items.unshift({ kind: 'up', x, y, w: 1, h: 1 });
  landing(d, li - 1, x, y);
}

/** Rock round (x, y) on a level made a landing (3 × 3, inside the grid), if (x, y) is rock. */
function landing(d: SiteDesign, li: number, x: number, y: number) {
  const nx = d.nx;
  const cells = decode(d.levels[li].cells, nx * d.ny);
  if (cells[y * nx + x] >= 0) return;
  const room = addRoom(d, li, 'landing');
  paint(
    d,
    li,
    rectSquares(d, Math.max(1, x - 1), Math.max(1, y - 1), Math.min(nx - 2, x + 1), Math.min(d.ny - 2, y + 1)).filter((k) => cells[k] < 0),
    room,
  );
}

/** Its default name: "Level 2 · 40 ft down", or "· the deep" for the deepest. */
function levelName(depth: number, deepest: boolean): string {
  return deepest ? `Level ${depth} · the deep` : `Level ${depth} · ${depth * LEVEL_FT} ft down`;
}
const DEFAULT_NAME = /^Level \d+ · (the deep|\d+ ft down)$/;

/** A level dug below the deepest: a landing round its way up, under a way down from the level
 * above (on its free floor nearest its middle). */
export function addLevel(d: SiteDesign) {
  const n = d.levels.length;
  const deep = d.levels[0];
  const { nx, ny } = d;
  const cells = decode(deep.cells, nx * ny);
  const free = (k: number) => {
    const [x, y] = [k % nx, Math.floor(k / nx)];
    return cells[k] >= 0 && x > 1 && y > 1 && x < nx - 2 && y < ny - 2 && !deep.items.some((f) => covers(f, x, y)) && !deep.doors.some((e) => (e[0] === x && e[1] === y) || (e[2] === 0 ? e[0] + 1 === x && e[1] === y : e[0] === x && e[1] + 1 === y));
  };
  let [sx, sy, c] = [0, 0, 0];
  cells.forEach((r, k) => {
    if (r >= 0) [sx, sy, c] = [sx + (k % nx), sy + Math.floor(k / nx), c + 1];
  });
  const [mx, my] = c ? [sx / c, sy / c] : [nx / 2, ny / 2];
  let best = -1;
  for (let k = 0; k < cells.length; k++) {
    if (free(k) && (best < 0 || Math.hypot((k % nx) - mx, Math.floor(k / nx) - my) < Math.hypot((best % nx) - mx, Math.floor(best / nx) - my))) best = k;
  }
  if (best < 0) return false;
  if (DEFAULT_NAME.test(deep.name)) deep.name = levelName(n, false);
  d.levels.unshift({ name: levelName(n + 1, true), elevation_ft: deep.elevation_ft - LEVEL_FT, natural: deep.natural, cells: [-1, nx * ny], rooms: [], doors: [], items: [] });
  setWayDown(d, 1, best % nx, Math.floor(best / nx));
  return true;
}

/** The deepest level filled in (a site keeps one level at least). */
export function removeLevel(d: SiteDesign) {
  if (d.levels.length <= 1) return;
  d.levels.shift();
  const deep = d.levels[0];
  deep.items = deep.items.filter((f) => f.kind !== 'down');
  if (DEFAULT_NAME.test(deep.name)) deep.name = levelName(d.levels.length, true);
}

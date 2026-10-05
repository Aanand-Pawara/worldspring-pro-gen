// After the world is regenerated from a changed sketch, generated features can move (a
// town placed a few miles away, a range redrawn) and so get new ids. Edits on a feature that
// no longer exists move to the nearest feature of the same kind that took its place.
import type { Edits, Feature } from '../gen/protocol';

/** How far a feature may have moved and still be the same one (ft): 40 mi, or more for big
 * ones (a range's or forest's anchor sits mid-way along it and moves as its shape does). */
const reachFor = (f: Feature) => Math.max(40 * 5280, 0.6 * f.extent_ft);

/** Ids of buildings, districts, wall towers, sites underground and created sites: not moved. */
const LOCAL = /^[bdtuwkc]:/;

/** `edits` with keys of vanished features moved to their nearest same-kind successors (in
 * `after`, by their positions in `before`), and what moved; null if nothing did. */
export function reanchor(edits: Edits, before: Feature[], after: Feature[]): { edits: Edits; moved: [string, string][] } | null {
  const now = new Set(after.map((f) => f.id));
  const was = new Map(before.map((f) => [f.id, f]));
  const keyed = new Set([...Object.keys(edits.renames ?? {}), ...Object.keys(edits.notes ?? {}), ...(edits.hidden ?? [])]);
  const taken = new Set([...keyed].filter((id) => now.has(id)));
  const moved: [string, string][] = [];
  for (const id of keyed) {
    if (now.has(id) || LOCAL.test(id)) continue;
    const old = was.get(id);
    if (!old) continue;
    let best: Feature | null = null;
    let bestD = reachFor(old);
    for (const f of after) {
      if (f.kind !== old.kind || taken.has(f.id)) continue;
      const d = Math.hypot(f.x - old.x, f.y - old.y);
      if (d < bestD) [best, bestD] = [f, d];
    }
    if (best) {
      taken.add(best.id);
      moved.push([id, best.id]);
    }
  }
  if (!moved.length) return null;
  const to = new Map(moved);
  const rekey = <T>(r: Record<string, T> | undefined) => (r ? Object.fromEntries(Object.entries(r).map(([k, v]) => [to.get(k) ?? k, v])) : r);
  return {
    edits: {
      ...edits,
      ...(edits.renames ? { renames: rekey(edits.renames) } : {}),
      ...(edits.notes ? { notes: rekey(edits.notes) } : {}),
      ...(edits.hidden ? { hidden: edits.hidden.map((k) => to.get(k) ?? k) } : {}),
    },
    moved,
  };
}

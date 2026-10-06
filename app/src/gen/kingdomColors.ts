/** Shared deterministic kingdom colours used by the map and every kingdom UI surface. */
export type KingdomColourEntry = { id: number };

function hash32(n: number): number {
  n = Math.imul(n ^ (n >>> 16), 0x45d9f3b);
  n = Math.imul(n ^ (n >>> 16), 0x45d9f3b);
  return (n ^ (n >>> 16)) >>> 0;
}

function hslToHex(h: number, s: number, l: number): number {
  const c = (1 - Math.abs(2 * l - 1)) * s;
  const x = c * (1 - Math.abs((h / 60) % 2 - 1));
  const m = l - c / 2;
  let r = 0, g = 0, b = 0;
  if (h < 60) [r, g, b] = [c, x, 0];
  else if (h < 120) [r, g, b] = [x, c, 0];
  else if (h < 180) [r, g, b] = [0, c, x];
  else if (h < 240) [r, g, b] = [0, x, c];
  else if (h < 300) [r, g, b] = [x, 0, c];
  else [r, g, b] = [c, 0, x];
  return ((Math.round((r + m) * 255) << 16) | (Math.round((g + m) * 255) << 8) | Math.round((b + m) * 255)) >>> 0;
}

/** Build a stable, collision-free colour assignment for the kingdoms in one generated world. */
export function buildKingdomColours(kingdoms: KingdomColourEntry[], seed: number): Map<number, number> {
  const ids = [...new Set(kingdoms.map((k) => k.id))].sort((a, b) => a - b);
  const out = new Map<number, number>();
  const used = new Set<number>();
  const offset = (hash32(seed >>> 0) / 0x1_0000_0000) * 360;
  for (let i = 0; i < ids.length; i++) {
    let colour = 0;
    let attempt = 0;
    do {
      const hue = (offset + i * 137.507764 + attempt * 0.73) % 360;
      const sat = 0.62 + ((hash32(seed + ids[i] * 31 + attempt) % 7) / 100);
      const light = 0.52 + ((hash32(seed + ids[i] * 67 + attempt) % 7) / 100);
      colour = hslToHex(hue, sat, light);
      attempt++;
    } while (used.has(colour));
    used.add(colour);
    out.set(ids[i], colour);
  }
  return out;
}

export function kingdomCssColour(colour: number): string {
  return `#${colour.toString(16).padStart(6, '0')}`;
}

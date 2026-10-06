import type { Feature, Overlay } from './protocol';

export interface KingdomRecord {
  id: number;
  name: string;
  capital: Feature | null;
  population: number;
  areaCells: number;
  cities: number;
  towns: number;
  villages: number;
  settlements: Feature[];
  neighbours: number[];
}

export function buildKingdomIndex(overlay: Overlay | null): KingdomRecord[] {
  if (!overlay) return [];
  const byId = new Map<number, KingdomRecord>();
  const featureById = new Map(overlay.features.map((feature) => [feature.id, feature]));
  for (const kingdom of overlay.kingdoms ?? []) {
    byId.set(kingdom.id, {
      id: kingdom.id,
      name: kingdom.name,
      capital: null,
      population: kingdom.population,
      areaCells: kingdom.area_cells,
      cities: kingdom.cities,
      towns: kingdom.towns,
      villages: kingdom.villages,
      settlements: [],
      neighbours: [],
    });
  }
  for (const feature of overlay.features) {
    const id = feature.kingdom_id;
    if (id === undefined || !['metropolis', 'city', 'town', 'village'].includes(feature.kind)) continue;
    const kingdom = byId.get(id) ?? {
      id,
      name: feature.kingdom_name ?? `Kingdom ${id + 1}`,
      capital: null,
      population: 0,
      areaCells: 0,
      cities: 0,
      towns: 0,
      villages: 0,
      settlements: [],
      neighbours: [],
    };
    kingdom.settlements.push(feature);
    if (feature.political_rank === 'capital') kingdom.capital = feature;
    byId.set(id, kingdom);
  }
  for (const kingdom of byId.values()) {
    if (!kingdom.capital) {
      const raw = overlay.kingdoms?.find((item) => item.id === kingdom.id)?.capital;
      const feature = raw !== undefined ? featureById.get(String(raw)) : undefined;
      if (feature) kingdom.capital = feature;
    }
  }
  const neighbours = new Map<number, Set<number>>();
  for (const border of overlay.kingdom_borders ?? []) {
    if (border.kingdom < 0 || border.other < 0 || border.kingdom === border.other) continue;
    const a = neighbours.get(border.kingdom) ?? new Set<number>();
    const b = neighbours.get(border.other) ?? new Set<number>();
    a.add(border.other);
    b.add(border.kingdom);
    neighbours.set(border.kingdom, a);
    neighbours.set(border.other, b);
  }
  for (const kingdom of byId.values()) kingdom.neighbours = [...(neighbours.get(kingdom.id) ?? [])].sort((a, b) => a - b);
  return [...byId.values()].sort((a, b) => b.population - a.population || a.name.localeCompare(b.name));
}

export function kingdomById(kingdoms: KingdomRecord[], id: number | null): KingdomRecord | null {
  return id === null ? null : kingdoms.find((kingdom) => kingdom.id === id) ?? null;
}

<script lang="ts">
  import type { Feature, Overlay, WorldFile } from '../../gen/protocol';
  import Icon from '../Icon.svelte';

  interface Props {
    world: WorldFile;
    overlay: Overlay | null;
    onSelect: (f: Feature) => void;
  }

  let { world, overlay, onSelect }: Props = $props();
  let shown = $state(0);

  const features = $derived(overlay?.features ?? []);
  const settlements = $derived(features.filter((f) => ['metropolis', 'city', 'town', 'village'].includes(f.kind)));
  const peaks = $derived(features.filter((f) => f.kind === 'peak' || f.kind === 'volcano'));
  const rivers = $derived(features.filter((f) => f.kind === 'river'));
  const lakes = $derived(features.filter((f) => f.kind === 'lake' || f.kind === 'salt_lake'));
  const waterfalls = $derived(features.filter((f) => f.kind === 'waterfall'));
  const landmasses = $derived(features.filter((f) => f.kind === 'continent' || f.kind === 'island'));
  const population = (f: Feature) => Number(/pop\. ([\d,]+)/.exec(f.detail ?? '')?.[1]?.replace(/,/g, '') ?? 0);
  const drop = (f: Feature) => Number(/drop ~([\d,]+) ft/.exec(f.detail ?? '')?.[1]?.replace(/,/g, '') ?? 0);
  const miles = (ft: number) => ft / 5280;
  const area = (ft: number) => (ft * ft) / (5280 * 5280);
  const fmt = (n: number) => n.toLocaleString();
  const fmtMiles = (n: number) => n.toLocaleString(undefined, { maximumFractionDigits: 0 });
  const name = (f: Feature) => f.name;

  const largestSettlement = $derived(settlements.reduce<Feature | null>((best, f) => !best || population(f) > population(best) ? f : best, null));
  const highestPeak = $derived(peaks.reduce<Feature | null>((best, f) => !best || (f.elev_ft ?? 0) > (best.elev_ft ?? 0) ? f : best, null));
  const longestRiver = $derived(rivers.reduce<Feature | null>((best, f) => !best || f.extent_ft > best.extent_ft ? f : best, null));
  const largestLake = $derived(lakes.reduce<Feature | null>((best, f) => !best || f.extent_ft > best.extent_ft ? f : best, null));
  const largestLandmass = $derived(landmasses.reduce<Feature | null>((best, f) => !best || f.extent_ft > best.extent_ft ? f : best, null));
  const highestWaterfall = $derived(waterfalls.reduce<Feature | null>((best, f) => !best || drop(f) > drop(best) ? f : best, null));
  const totalPopulation = $derived(settlements.reduce((n, f) => n + population(f), 0));
  const cities = $derived(settlements.filter((f) => f.kind === 'city' || f.kind === 'metropolis').length);
  const villages = $derived(settlements.filter((f) => f.kind === 'village').length);

  type Fact = { text: string; feature?: Feature; tag: string };
  const facts = $derived.by<Fact[]>(() => {
    if (!overlay) return [];
    const out: Fact[] = [
      { text: `This world has ${fmt(features.length)} named geographic features in its generated overlay.`, tag: 'GEOGRAPHY' },
      { text: `The generated map spans ${fmt(world.params.width_mi ?? 0)} × ${fmt(world.params.height_mi ?? 0)} miles.`, tag: 'WORLD' },
      { text: `The generated settlements contain an estimated ${fmt(totalPopulation)} people across ${fmt(settlements.length)} settlements.`, tag: 'PEOPLE' },
      { text: `There are ${fmt(cities)} cities or metropolises and ${fmt(villages)} villages in the generated settlement network.`, tag: 'SETTLEMENTS' },
    ];
    if (largestSettlement) out.push({ text: `${name(largestSettlement)} is the largest settlement, with an estimated population of ${fmt(population(largestSettlement))}.`, feature: largestSettlement, tag: 'SETTLEMENTS' });
    if (highestPeak) out.push({ text: `${name(highestPeak)} is the highest named peak at ${fmt(Math.round(highestPeak.elev_ft ?? 0))} ft.`, feature: highestPeak, tag: 'TERRAIN' });
    if (longestRiver) out.push({ text: `${name(longestRiver)} is the longest generated river at about ${fmtMiles(miles(longestRiver.extent_ft))} miles.`, feature: longestRiver, tag: 'WATER' });
    if (largestLake) out.push({ text: `${name(largestLake)} is the largest generated lake by its stored extent, about ${fmtMiles(area(largestLake.extent_ft))} square miles.`, feature: largestLake, tag: 'WATER' });
    if (largestLandmass) out.push({ text: `${name(largestLandmass)} is the largest generated landmass by its stored extent, about ${fmtMiles(area(largestLandmass.extent_ft))} square miles.`, feature: largestLandmass, tag: 'GEOGRAPHY' });
    if (highestWaterfall) out.push({ text: `${name(highestWaterfall)} has the greatest generated waterfall drop at about ${fmt(drop(highestWaterfall))} ft.`, feature: highestWaterfall, tag: 'WATER' });
    return out;
  });

  const current = $derived(facts.length ? facts[shown % facts.length] : null);

  function randomFact() {
    if (facts.length < 2) return;
    let next = Math.floor(Math.random() * facts.length);
    if (next === shown % facts.length) next = (next + 1) % facts.length;
    shown = next;
  }

  function reset() {
    shown = 0;
  }
  $effect(() => {
    void overlay;
    reset();
  });
</script>

<div class="facts">
  <div class="hero">
    <div>
      <div class="eyebrow">GENERATED WORLD</div>
      <h2>Random facts</h2>
      <p>Every fact below is calculated from this world's generated data. No lore is invented just to fill the box.</p>
    </div>
    <button class="ws-btn primary fact-btn" onclick={randomFact} disabled={facts.length < 2} title="Show another fact">
      <Icon name="dice" size={16} /> Random fact
    </button>
  </div>

  {#if current}
    <article class="fact-card">
      <div class="tag">{current.tag}</div>
      <div class="fact">{current.text}</div>
      {#if current.feature}
        <button class="show" onclick={() => onSelect(current.feature!)}><Icon name="pin" size={14} /> Show on map</button>
      {/if}
    </article>
    <div class="counter">Fact {((shown % facts.length) + 1).toLocaleString()} of {facts.length.toLocaleString()}</div>
  {:else}
    <div class="empty">
      <Icon name="info" size={18} />
      <span>The world has not finished generating, so there are no facts yet.</span>
    </div>
  {/if}
</div>

<style>
  .facts { display:flex; flex-direction:column; gap:12px; }
  .hero { display:flex; flex-direction:column; gap:8px; }
  .eyebrow { font-size:10px; letter-spacing:.08em; color:var(--ink-3); }
  h2 { margin:1px 0 2px; font-size:20px; }
  p { margin:0; color:var(--ink-2); font-size:12px; line-height:1.45; }
  .fact-btn { align-self:flex-start; }
  .fact-card { padding:14px; border:1px solid var(--line); border-radius:var(--radius); background:var(--btn-hover); }
  .tag { margin-bottom:8px; font-size:10px; letter-spacing:.08em; font-weight:bold; color:var(--ink-3); }
  .fact { font-size:16px; line-height:1.5; }
  .show { display:inline-flex; align-items:center; gap:5px; margin-top:12px; border:0; background:none; color:var(--ink-2); font:11px var(--font); cursor:pointer; padding:3px 0; }
  .show:hover { color:var(--ink); }
  .counter { color:var(--ink-3); font:10px var(--mono); text-align:center; }
  .empty { display:flex; gap:8px; align-items:center; color:var(--ink-3); padding:12px 4px; font-size:12px; }
</style>

<script lang="ts">
  import type { Feature, Overlay, WorldFile } from '../../gen/protocol';
  import Icon from '../Icon.svelte';

  interface Props {
    world: WorldFile;
    overlay: Overlay | null;
    onSelect: (f: Feature) => void;
  }

  let { world, overlay, onSelect }: Props = $props();
  let mode = $state<'overview' | 'facts' | 'rankings'>('overview');
  let ranking = $state<'settlements' | 'terrain' | 'water' | 'regions'>('settlements');
  let shown = $state(0);

  const features = $derived(overlay?.features ?? []);
  const settlements = $derived(features.filter((f) => ['metropolis', 'city', 'town', 'village'].includes(f.kind)));
  const peaks = $derived(features.filter((f) => f.kind === 'peak' || f.kind === 'volcano'));
  const rivers = $derived(features.filter((f) => f.kind === 'river'));
  const lakes = $derived(features.filter((f) => ['lake', 'salt_lake', 'salt_flat'].includes(f.kind)));
  const waterfalls = $derived(features.filter((f) => f.kind === 'waterfall'));
  const landmasses = $derived(features.filter((f) => ['continent', 'island'].includes(f.kind)));
  const ranges = $derived(features.filter((f) => f.kind === 'range'));
  const passes = $derived(features.filter((f) => f.kind === 'pass'));
  const volcanoes = $derived(features.filter((f) => f.kind === 'volcano'));
  const regions = $derived(features.filter((f) => ['forest', 'jungle', 'taiga', 'desert', 'swamp', 'plains', 'tundra', 'glacier'].includes(f.kind)));

  const population = (f: Feature) => Number(/pop\. ([\d,]+)/.exec(f.detail ?? '')?.[1]?.replace(/,/g, '') ?? 0);
  const drop = (f: Feature) => Number(/drop ~([\d,]+) ft/.exec(f.detail ?? '')?.[1]?.replace(/,/g, '') ?? 0);
  const miles = (ft: number) => ft / 5280;
  const areaSqMi = (ft: number) => (ft * ft) / (5280 * 5280);
  const fmt = (n: number) => n.toLocaleString();
  const fmtMiles = (n: number) => n.toLocaleString(undefined, { maximumFractionDigits: 0 });
  const name = (f: Feature) => f.name;
  const distanceMi = (a: Feature, b: Feature) => Math.hypot(a.x - b.x, a.y - b.y) / 5280;

  const totalPopulation = $derived(settlements.reduce((n, f) => n + population(f), 0));
  const cities = $derived(settlements.filter((f) => f.kind === 'city' || f.kind === 'metropolis'));
  const towns = $derived(settlements.filter((f) => f.kind === 'town'));
  const villages = $derived(settlements.filter((f) => f.kind === 'village'));
  const capitals = $derived(settlements.filter((f) => (f.detail ?? '').includes(', capital')));
  const cityPopulation = $derived(cities.reduce((n, f) => n + population(f), 0));
  const capitalPopulation = $derived(capitals.reduce((n, f) => n + population(f), 0));
  const cityPopulationShare = $derived(totalPopulation ? Math.round((cityPopulation / totalPopulation) * 100) : 0);
  const capitalPopulationShare = $derived(totalPopulation ? Math.round((capitalPopulation / totalPopulation) * 100) : 0);
  const namedKinds = $derived(new Set(features.map((f) => f.kind)).size);

  const largestSettlement = $derived(settlements.reduce<Feature | null>((best, f) => !best || population(f) > population(best) ? f : best, null));
  const highestPeak = $derived(peaks.reduce<Feature | null>((best, f) => !best || (f.elev_ft ?? 0) > (best.elev_ft ?? 0) ? f : best, null));
  const longestRiver = $derived(rivers.reduce<Feature | null>((best, f) => !best || f.extent_ft > best.extent_ft ? f : best, null));
  const largestRiverBasin = $derived(rivers.reduce<Feature | null>((best, f) => !best || (f.drainage_area_mi2 ?? 0) > (best.drainage_area_mi2 ?? 0) ? f : best, null));
  const largestLake = $derived(lakes.reduce<Feature | null>((best, f) => !best || f.extent_ft > best.extent_ft ? f : best, null));
  const largestLandmass = $derived(landmasses.reduce<Feature | null>((best, f) => !best || f.extent_ft > best.extent_ft ? f : best, null));
  const highestWaterfall = $derived(waterfalls.reduce<Feature | null>((best, f) => !best || drop(f) > drop(best) ? f : best, null));
  const largestRange = $derived(ranges.reduce<Feature | null>((best, f) => !best || f.extent_ft > best.extent_ft ? f : best, null));

  const activeVolcanoes = $derived(volcanoes.filter((f) => (f.detail ?? '').startsWith('active ')));
  const dormantVolcanoes = $derived(volcanoes.filter((f) => (f.detail ?? '').startsWith('dormant ')));
  const extinctVolcanoes = $derived(volcanoes.filter((f) => (f.detail ?? '').startsWith('extinct ')));

  const regionCounts = $derived.by(() => {
    const counts = new Map<string, number>();
    for (const f of regions) counts.set(f.kind, (counts.get(f.kind) ?? 0) + 1);
    return [...counts.entries()].sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]));
  });
  const leadingRegion = $derived(regionCounts[0] ?? null);

  // Relational facts use generated feature anchors. They describe proximity between named
  // generated objects rather than inventing roads, history, ownership or other unsupported lore.
  const settlementsNearLongestRiver = $derived(longestRiver ? settlements.filter((s) => distanceMi(s, longestRiver) <= 50) : []);
  const settlementsNearHighestPeak = $derived(highestPeak ? settlements.filter((s) => distanceMi(s, highestPeak) <= 50) : []);
  const featuresNearLargestCity = $derived(largestSettlement ? features.filter((f) => f.id !== largestSettlement.id && distanceMi(f, largestSettlement) <= 25) : []);
  const nearbyPeak = $derived.by(() => {
    if (!largestSettlement || !peaks.length) return null;
    return peaks.map((p) => ({ f: p, d: distanceMi(p, largestSettlement) })).sort((a, b) => a.d - b.d)[0] ?? null;
  });

  type Fact = { text: string; tag: string; feature?: Feature };
  const facts = $derived.by<Fact[]>(() => {
    if (!overlay) return [];
    const out: Fact[] = [
      { text: `The generated world contains ${fmt(features.length)} named features across ${fmt(namedKinds)} feature types.`, tag: 'WORLD' },
      { text: `The map covers ${fmt(world.params.width_mi ?? 0)} × ${fmt(world.params.height_mi ?? 0)} miles.`, tag: 'WORLD' },
      { text: `Its named settlements account for an estimated ${fmt(totalPopulation)} people across ${fmt(settlements.length)} settlements.`, tag: 'PEOPLE' },
      { text: `${fmt(cities.length)} cities or metropolises, ${fmt(towns.length)} towns and ${fmt(villages.length)} villages make up the settlement network.`, tag: 'PEOPLE' },
      { text: `${fmt(capitals.length)} generated settlement${capitals.length === 1 ? '' : 's'} ${capitals.length === 1 ? 'is' : 'are'} marked as capital${capitals.length === 1 ? '' : 's'}.`, tag: 'CIVILIZATION' },
      { text: `Cities and metropolises contain about ${fmt(cityPopulationShare)}% of the generated population.`, tag: 'CIVILIZATION' },
      { text: capitals.length ? `Capital settlements contain about ${fmt(capitalPopulationShare)}% of the generated population.` : 'No generated settlement is marked as a capital.', tag: 'CIVILIZATION' },
      { text: `${fmt(volcanoes.length)} named volcanoes are present: ${fmt(activeVolcanoes.length)} active, ${fmt(dormantVolcanoes.length)} dormant and ${fmt(extinctVolcanoes.length)} extinct.`, tag: 'GEOLOGY' },
      { text: `${fmt(regions.length)} named biome regions were generated; ${leadingRegion ? `${leadingRegion[1]} are ${leadingRegion[0].replace('_', ' ')} regions, the most represented named region type.` : 'no large named biome region was extracted.'}`, tag: 'CLIMATE' },
    ];
    if (largestSettlement) out.push({ text: `${name(largestSettlement)} is the largest settlement, with an estimated population of ${fmt(population(largestSettlement))}.`, tag: 'PEOPLE', feature: largestSettlement });
    if (highestPeak) out.push({ text: `${name(highestPeak)} is the highest named summit at ${fmt(Math.round(highestPeak.elev_ft ?? 0))} ft above sea level.`, tag: 'TERRAIN', feature: highestPeak });
    if (longestRiver) out.push({ text: `${name(longestRiver)} is the longest named river at about ${fmtMiles(miles(longestRiver.extent_ft))} miles.`, tag: 'WATER', feature: longestRiver });
    if (largestRiverBasin) out.push({ text: `${name(largestRiverBasin)} drains the largest modeled catchment, about ${fmtMiles(largestRiverBasin.drainage_area_mi2 ?? 0)} square miles, at stream order ${largestRiverBasin.stream_order ?? 1}.`, tag: 'HYDROLOGY', feature: largestRiverBasin });
    if (largestLake) out.push({ text: `${name(largestLake)} is the largest generated lake-type feature by stored extent, about ${fmtMiles(areaSqMi(largestLake.extent_ft))} square miles.`, tag: 'WATER', feature: largestLake });
    if (largestLandmass) out.push({ text: `${name(largestLandmass)} is the largest generated landmass by stored extent, about ${fmtMiles(areaSqMi(largestLandmass.extent_ft))} square miles.`, tag: 'GEOGRAPHY', feature: largestLandmass });
    if (highestWaterfall) out.push({ text: `${name(highestWaterfall)} has the greatest named waterfall drop at about ${fmt(drop(highestWaterfall))} ft.`, tag: 'WATER', feature: highestWaterfall });
    if (largestRange) out.push({ text: `${name(largestRange)} is the longest generated mountain-range feature by its stored extent.`, tag: 'TERRAIN', feature: largestRange });
    if (longestRiver && settlementsNearLongestRiver.length) out.push({ text: `${fmt(settlementsNearLongestRiver.length)} settlements have generated label anchors within 50 miles of ${name(longestRiver)}.`, tag: 'RELATION', feature: longestRiver });
    if (highestPeak && settlementsNearHighestPeak.length) out.push({ text: `${fmt(settlementsNearHighestPeak.length)} settlements have generated label anchors within 50 miles of ${name(highestPeak)}.`, tag: 'RELATION', feature: highestPeak });
    if (largestSettlement && nearbyPeak) out.push({ text: `${name(largestSettlement)} is about ${fmtMiles(nearbyPeak.d)} miles from the named peak ${name(nearbyPeak.f)}.`, tag: 'RELATION', feature: largestSettlement });
    if (largestSettlement && featuresNearLargestCity.length) out.push({ text: `${name(largestSettlement)} has ${fmt(featuresNearLargestCity.length)} other generated feature anchors within 25 miles.`, tag: 'RELATION', feature: largestSettlement });
    return out;
  });

  const current = $derived(facts.length ? facts[shown % facts.length] : null);
  function randomFact() {
    if (facts.length < 2) return;
    let next = Math.floor(Math.random() * facts.length);
    if (next === shown % facts.length) next = (next + 1) % facts.length;
    shown = next;
  }

  function selectFeature(f: Feature) {
    onSelect(f);
  }

  function selectMode(next: 'overview' | 'facts' | 'rankings') {
    mode = next;
  }

  function selectRanking(next: 'settlements' | 'terrain' | 'water' | 'regions') {
    ranking = next;
  }

  function handleModePointer(e: PointerEvent) {
    e.stopPropagation();
    const button = (e.target as HTMLElement).closest<HTMLButtonElement>('button[data-mode]');
    if (!button || button.disabled) return;
    const next = button.dataset.mode;
    if (next === 'overview' || next === 'facts' || next === 'rankings') selectMode(next);
  }

  function handleRankingPointer(e: PointerEvent) {
    e.stopPropagation();
    const button = (e.target as HTMLElement).closest<HTMLButtonElement>('button[data-ranking]');
    if (!button || button.disabled) return;
    const next = button.dataset.ranking;
    if (next === 'settlements' || next === 'terrain' || next === 'water' || next === 'regions') selectRanking(next);
  }

  const settlementRank = $derived([...settlements].sort((a, b) => population(b) - population(a) || a.name.localeCompare(b.name)));
  const peakRank = $derived([...peaks].sort((a, b) => (b.elev_ft ?? 0) - (a.elev_ft ?? 0)));
  const riverRank = $derived([...rivers].sort((a, b) => b.extent_ft - a.extent_ft));
  const lakeRank = $derived([...lakes].sort((a, b) => b.extent_ft - a.extent_ft));
  const regionRank = $derived([...regionCounts]);

  $effect(() => {
    void overlay;
    shown = 0;
  });
</script>

<div class="almanac">
  <header class="hero">
    <div class="eyebrow">WORLD REFERENCE</div>
    <div class="hero-line">
      <div>
        <h2>World Almanac</h2>
        <p>A readable reference for what this generated world actually contains.</p>
      </div>
      {#if mode === 'facts'}
        <button class="ws-btn primary" onclick={randomFact} disabled={facts.length < 2}><Icon name="dice" size={15} /> Random</button>
      {/if}
    </div>
  </header>

  <nav class="subtabs" aria-label="Almanac views" onpointerup={handleModePointer}>
    <button type="button" data-mode="overview" aria-pressed={mode === 'overview'} class:on={mode === 'overview'} onclick={() => selectMode('overview')}><Icon name="globe" size={14} /> Overview</button>
    <button type="button" data-mode="facts" aria-pressed={mode === 'facts'} class:on={mode === 'facts'} onclick={() => selectMode('facts')}><Icon name="dice" size={14} /> Facts</button>
    <button type="button" data-mode="rankings" aria-pressed={mode === 'rankings'} class:on={mode === 'rankings'} onclick={() => selectMode('rankings')}><Icon name="activity" size={14} /> Rankings</button>
  </nav>

  {#if !overlay}
    <div class="empty"><Icon name="help" size={18} /><span>Generate a world first. The almanac reads the generated overlay directly.</span></div>
  {:else if mode === 'overview'}
    <div class="cards">
      <div><strong>{fmt(totalPopulation)}</strong><span>estimated population</span></div>
      <div><strong>{fmt(settlements.length)}</strong><span>settlements</span></div>
      <div><strong>{fmt(features.length)}</strong><span>named features</span></div>
      <div><strong>{fmt(regions.length)}</strong><span>named biome regions</span></div>
    </div>

    <section>
      <h3>World profile</h3>
      <div class="profile">
        <div><span>Map</span><b>{fmt(world.params.width_mi ?? 0)} × {fmt(world.params.height_mi ?? 0)} mi</b></div>
        <div><span>Cities</span><b>{fmt(cities.length)}</b></div>
        <div><span>Towns</span><b>{fmt(towns.length)}</b></div>
        <div><span>Villages</span><b>{fmt(villages.length)}</b></div>
        <div><span>Capitals</span><b>{fmt(capitals.length)}</b></div>
        <div><span>Urban population</span><b>{fmt(cityPopulationShare)}%</b></div>
        <div><span>Capital population</span><b>{fmt(capitalPopulationShare)}%</b></div>
        <div><span>Mountain ranges</span><b>{fmt(ranges.length)}</b></div>
        <div><span>Named rivers</span><b>{fmt(rivers.length)}</b></div>
        <div><span>Volcanoes</span><b>{fmt(volcanoes.length)}</b></div>
      </div>
    </section>

    <section>
      <h3>Natural extremes</h3>
      <div class="extremes">
        {#if highestPeak}<button onclick={() => selectFeature(highestPeak)}><Icon name="mountain" size={15} /><span>Highest peak<small>{name(highestPeak)}</small></span><b>{fmt(Math.round(highestPeak.elev_ft ?? 0))} ft</b></button>{/if}
        {#if longestRiver}<button onclick={() => selectFeature(longestRiver)}><Icon name="river" size={15} /><span>Longest river<small>{name(longestRiver)}</small></span><b>{fmtMiles(miles(longestRiver.extent_ft))} mi</b></button>{/if}
        {#if largestLake}<button onclick={() => selectFeature(largestLake)}><Icon name="waves" size={15} /><span>Largest lake-type feature<small>{name(largestLake)}</small></span><b>{fmtMiles(areaSqMi(largestLake.extent_ft))} mi²</b></button>{/if}
        {#if highestWaterfall}<button onclick={() => selectFeature(highestWaterfall)}><Icon name="waves" size={15} /><span>Highest waterfall<small>{name(highestWaterfall)}</small></span><b>{fmt(drop(highestWaterfall))} ft</b></button>{/if}
      </div>
    </section>

    <section>
      <h3>Generated ecology</h3>
      <div class="profile">
        {#each regionCounts.slice(0, 8) as [kind, count] (kind)}
          <div><span>{kind.replace('_', ' ')}</span><b>{fmt(count)} named regions</b></div>
        {:else}<div class="muted">No named biome regions were extracted.</div>{/each}
      </div>
    </section>
  {:else if mode === 'facts'}
    {#if current}
      <article class="fact-card">
        <div class="tag">{current.tag}</div>
        <div class="fact">{current.text}</div>
        {#if current.feature}<button class="show" onclick={() => selectFeature(current.feature!)}><Icon name="pin" size={14} /> Show on map</button>{/if}
      </article>
      <div class="counter">Fact {((shown % facts.length) + 1).toLocaleString()} of {facts.length.toLocaleString()}</div>
    {:else}
      <div class="empty">No generated facts are available yet.</div>
    {/if}
    <div class="fact-note">Facts are calculated from generated data. Proximity facts use named feature anchors, so they never pretend we have a road network or historical record when we don't.</div>
  {:else}
    <nav class="rank-tabs" aria-label="Ranking categories" onpointerup={handleRankingPointer}>
      <button type="button" data-ranking="settlements" aria-pressed={ranking === 'settlements'} class:on={ranking === 'settlements'} onclick={() => selectRanking('settlements')}>Settlements</button>
      <button type="button" data-ranking="terrain" aria-pressed={ranking === 'terrain'} class:on={ranking === 'terrain'} onclick={() => selectRanking('terrain')}>Terrain</button>
      <button type="button" data-ranking="water" aria-pressed={ranking === 'water'} class:on={ranking === 'water'} onclick={() => selectRanking('water')}>Water</button>
      <button type="button" data-ranking="regions" aria-pressed={ranking === 'regions'} class:on={ranking === 'regions'} onclick={() => selectRanking('regions')}>Biomes</button>
    </nav>

    {#if ranking === 'settlements'}
      <Rank title="Most populated settlements" items={settlementRank.slice(0, 15)} value={(f) => fmt(population(f))} suffix="" icon="castle" {selectFeature} />
    {:else if ranking === 'terrain'}
      <Rank title="Highest peaks and volcanoes" items={peakRank.slice(0, 15)} value={(f) => fmt(Math.round(f.elev_ft ?? 0))} suffix=" ft" icon="mountain" {selectFeature} />
      {#if ranges.length}<Rank title="Largest mountain ranges" items={[...ranges].sort((a, b) => b.extent_ft - a.extent_ft).slice(0, 10)} value={(f) => fmtMiles(miles(f.extent_ft))} suffix=" mi" icon="mountain" {selectFeature} />{/if}
      {#if passes.length}<Rank title="Highest passes" items={[...passes].sort((a, b) => (b.elev_ft ?? 0) - (a.elev_ft ?? 0)).slice(0, 10)} value={(f) => fmt(Math.round(f.elev_ft ?? 0))} suffix=" ft" icon="mountain" {selectFeature} />{/if}
    {:else if ranking === 'water'}
      <Rank title="Longest rivers" items={riverRank.slice(0, 15)} value={(f) => fmtMiles(miles(f.extent_ft))} suffix=" mi" icon="river" {selectFeature} />
      <Rank title="Largest drainage basins" items={[...rivers].sort((a, b) => (b.drainage_area_mi2 ?? 0) - (a.drainage_area_mi2 ?? 0)).slice(0, 15)} value={(f) => fmtMiles(f.drainage_area_mi2 ?? 0)} suffix=" mi²" icon="river" {selectFeature} />
      <Rank title="Largest lake-type features" items={lakeRank.slice(0, 15)} value={(f) => fmtMiles(areaSqMi(f.extent_ft))} suffix=" mi²" icon="waves" {selectFeature} />
      <Rank title="Highest stream order" items={[...rivers].sort((a, b) => (b.stream_order ?? 0) - (a.stream_order ?? 0) || (b.drainage_area_mi2 ?? 0) - (a.drainage_area_mi2 ?? 0)).slice(0, 15)} value={(f) => fmt(f.stream_order ?? 0)} suffix="" icon="river" {selectFeature} />
      <Rank title="Greatest waterfalls" items={[...waterfalls].sort((a, b) => drop(b) - drop(a)).slice(0, 10)} value={(f) => fmt(drop(f))} suffix=" ft" icon="waves" {selectFeature} />
    {:else}
      <section>
        <h3>Most represented named biome regions</h3>
        {#each regionRank as [kind, count], i (kind)}
          <div class="row region-row">
            <span class="rank">{i + 1}</span>
            <span class="icon"><Icon name="land" size={15} /></span>
            <span class="name">{kind.replace('_', ' ')}<small>generated named regions</small></span>
            <strong>{fmt(count)}</strong>
          </div>
        {:else}<div class="empty">No named biome regions generated.</div>{/each}
      </section>
    {/if}
  {/if}
</div>

{#snippet Rank({ title, items, value, suffix, icon, selectFeature }: { title: string; items: Feature[]; value: (f: Feature) => string; suffix: string; icon: 'castle' | 'mountain' | 'river' | 'waves' | 'land'; selectFeature: (f: Feature) => void })}
  <section>
    <h3>{title}</h3>
    {#each items as f, i (f.id)}
      <button class="row" onclick={() => selectFeature(f)}>
        <span class="rank">{i + 1}</span>
        <span class="icon"><Icon name={icon} size={15} /></span>
        <span class="name">{f.name}<small>{f.kind.replace('_', ' ')}</small></span>
        <strong>{value(f)}{suffix}</strong>
      </button>
    {:else}<div class="empty">Nothing generated for this ranking.</div>{/each}
  </section>
{/snippet}

<style>
  .almanac { display:flex; flex-direction:column; gap:10px; }
  .hero { padding-bottom:2px; }
  .eyebrow { font-size:10px; letter-spacing:.1em; color:var(--ink-3); }
  .hero-line { display:flex; align-items:center; gap:8px; }
  .hero-line > div { min-width:0; flex:1; }
  h2 { margin:1px 0 2px; font-size:20px; }
  h3 { margin:0 0 6px; font-size:11px; text-transform:uppercase; letter-spacing:.06em; color:var(--ink-2); }
  p { margin:0; color:var(--ink-2); font-size:12px; line-height:1.4; }
  .subtabs,.rank-tabs { display:flex; gap:3px; overflow:auto; padding-bottom:1px; }
  .subtabs button,.rank-tabs button { display:inline-flex; align-items:center; gap:4px; border:1px solid var(--line); background:transparent; color:var(--ink-2); border-radius:var(--radius-sm); padding:6px 8px; white-space:nowrap; cursor:pointer; font:11px var(--font); }
  .subtabs button.on,.rank-tabs button.on { background:var(--accent); color:var(--accent-ink); border-color:var(--accent); }
  .cards { display:grid; grid-template-columns:repeat(2,1fr); gap:4px; }
  .cards > div { padding:9px; background:var(--btn-hover); border:1px solid var(--line-faint); border-radius:var(--radius-sm); }
  .cards strong,.cards span { display:block; }
  .cards strong { font:17px var(--mono); }
  .cards span { margin-top:2px; color:var(--ink-3); font-size:10px; }
  section { border-top:1px solid var(--line-faint); padding-top:8px; }
  .profile { display:grid; grid-template-columns:1fr 1fr; gap:4px; }
  .profile > div { display:flex; flex-direction:column; gap:2px; padding:7px; border:1px solid var(--line-faint); border-radius:var(--radius-sm); background:var(--btn-hover); min-width:0; }
  .profile span { color:var(--ink-3); font-size:10px; text-transform:capitalize; }
  .profile b { font:11px var(--mono); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .extremes { display:flex; flex-direction:column; gap:2px; }
  .extremes button { all:unset; display:grid; grid-template-columns:20px 1fr auto; gap:6px; align-items:center; padding:6px 4px; border-radius:var(--radius-sm); cursor:pointer; }
  .extremes button:hover { background:var(--btn-hover); }
  .extremes span { display:flex; flex-direction:column; min-width:0; font-size:11px; }
  .extremes small { color:var(--ink-3); overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .extremes b,.row strong { font:11px var(--mono); white-space:nowrap; }
  .fact-card { padding:14px; border:1px solid var(--line); border-radius:var(--radius); background:var(--btn-hover); }
  .tag { margin-bottom:8px; font-size:10px; letter-spacing:.08em; font-weight:bold; color:var(--ink-3); }
  .fact { font-size:16px; line-height:1.5; }
  .show { display:inline-flex; align-items:center; gap:5px; margin-top:12px; border:0; background:none; color:var(--ink-2); font:11px var(--font); cursor:pointer; padding:3px 0; }
  .show:hover { color:var(--ink); }
  .counter { color:var(--ink-3); font:10px var(--mono); text-align:center; }
  .fact-note { color:var(--ink-3); font-size:10px; line-height:1.4; padding:3px; }
  .row { all:unset; box-sizing:border-box; display:grid; grid-template-columns:22px 20px 1fr auto; align-items:center; gap:6px; width:100%; min-height:38px; padding:3px 4px; cursor:pointer; border-radius:var(--radius-sm); }
  .row:hover,.row:focus-visible { background:var(--btn-hover); }
  .rank { color:var(--ink-3); font:10px var(--mono); text-align:right; }
  .icon { color:var(--ink-2); display:flex; }
  .name { min-width:0; display:flex; flex-direction:column; overflow:hidden; text-align:left; font-size:11px; }
  .name small { color:var(--ink-3); font-size:10px; }
  .empty { display:flex; gap:8px; align-items:center; color:var(--ink-3); padding:12px 4px; font-size:12px; }
  .muted { color:var(--ink-3); font-size:11px; padding:4px; }
</style>

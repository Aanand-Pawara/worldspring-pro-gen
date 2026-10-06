<script lang="ts">
  import type { Feature, Overlay, WorldFile } from '../../gen/protocol';
  import Icon from '../Icon.svelte';
  import AlmanacTabs from './AlmanacTabs.svelte';
  import FactPanel from './FactPanel.svelte';
  import { buildAlmanacModel, fmt, fmtMiles, population, miles, areaSqMi, drop, type AlmanacModel } from './almanac/model';

  interface Props {
    world: WorldFile;
    overlay: Overlay | null;
    onSelect: (f: Feature) => void;
    onSelectKingdom: (id: number) => void;
  }

  let { world, overlay, onSelect, onSelectKingdom }: Props = $props();
  let mode = $state<'overview' | 'facts' | 'rankings' | 'kingdoms'>('overview');
  let ranking = $state<'settlements' | 'terrain' | 'water' | 'regions'>('settlements');
  let shown = $state(0);

  const ALMANAC_MODES = [{ key: 'overview', label: 'Overview', icon: 'globe' }, { key: 'kingdoms', label: 'Kingdoms', icon: 'building' }, { key: 'facts', label: 'Facts', icon: 'dice' }, { key: 'rankings', label: 'Rankings', icon: 'activity' }];
  const RANKING_MODES = [{ key: 'settlements', label: 'Settlements' }, { key: 'terrain', label: 'Terrain' }, { key: 'water', label: 'Water' }, { key: 'regions', label: 'Biomes' }];

  // The view consumes one model boundary. Add new Almanac data in model.ts, not here.
  // Navigation stays component-driven so new Almanac views do not duplicate markup.
  const model = $derived(buildAlmanacModel(world, overlay, mode));
  const features = $derived(model?.features ?? []), settlements = $derived(model?.settlements ?? []), peaks = $derived(model?.peaks ?? []), rivers = $derived(model?.rivers ?? []), lakes = $derived(model?.lakes ?? []), waterfalls = $derived(model?.waterfalls ?? []), landmasses = $derived(model?.landmasses ?? []), ranges = $derived(model?.ranges ?? []), passes = $derived(model?.passes ?? []), volcanoes = $derived(model?.volcanoes ?? []), regions = $derived(model?.regions ?? []);
  const cities = $derived(model?.cities ?? []), towns = $derived(model?.towns ?? []), villages = $derived(model?.villages ?? []), capitals = $derived(model?.capitals ?? []), kingdoms = $derived(model?.kingdoms ?? []), totalPopulation = $derived(model?.totalPopulation ?? 0), cityPopulationShare = $derived(model?.cityPopulationShare ?? 0), capitalPopulationShare = $derived(model?.capitalPopulationShare ?? 0), namedKinds = $derived(model?.namedKinds ?? 0);
  const largestSettlement = $derived(model?.largestSettlement ?? null), highestPeak = $derived(model?.highestPeak ?? null), longestRiver = $derived(model?.longestRiver ?? null), largestRiverBasin = $derived(model?.largestRiverBasin ?? null), largestLake = $derived(model?.largestLake ?? null), largestLandmass = $derived(model?.largestLandmass ?? null), highestWaterfall = $derived(model?.highestWaterfall ?? null), largestRange = $derived(model?.largestRange ?? null);
  const oceanRivers = $derived(model?.oceanRivers ?? []), inlandLakeRivers = $derived(model?.inlandLakeRivers ?? []), dryRivers = $derived(model?.dryRivers ?? []), lakeFedRivers = $derived(model?.lakeFedRivers ?? []), terminalLakes = $derived(model?.terminalLakes ?? []), flowThroughLakes = $derived(model?.flowThroughLakes ?? []), totalRiverMiles = $derived(model?.totalRiverMiles ?? 0), highestOrderRiver = $derived(model?.highestOrderRiver ?? null);
  const activeVolcanoes = $derived(model?.activeVolcanoes ?? []), dormantVolcanoes = $derived(model?.dormantVolcanoes ?? []), extinctVolcanoes = $derived(model?.extinctVolcanoes ?? []), regionCounts = $derived(model?.regionCounts ?? []), leadingRegion = $derived(model?.leadingRegion ?? null);
  const settlementRank = $derived(model?.settlementRank ?? []), peakRank = $derived(model?.peakRank ?? []), riverRank = $derived(model?.riverRank ?? []), lakeRank = $derived(model?.lakeRank ?? []), regionRank = $derived(model?.regionRank ?? []), facts = $derived(model?.facts ?? []);

  function randomFact() {
    if (facts.length < 2) return;
    let next = Math.floor(Math.random() * facts.length);
    if (next === shown % facts.length) next = (next + 1) % facts.length;
    shown = next;
  }

  function selectFeature(f: Feature) { onSelect(f); }
  function selectMode(next: 'overview' | 'facts' | 'rankings' | 'kingdoms') { if (mode === next) return; mode = next; if (next === 'facts') shown = 0; }
  function selectRanking(next: 'settlements' | 'terrain' | 'water' | 'regions') { if (ranking === next) return; ranking = next; }

  $effect(() => { void overlay; shown = 0; });
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
        <button class="ws-btn primary" type="button" onclick={(e) => { e.stopPropagation(); randomFact(); }} disabled={facts.length < 2}><Icon name="dice" size={15} /> Random</button>
      {/if}
    </div>
  </header>

  <AlmanacTabs value={mode} ariaLabel="Almanac views" items={ALMANAC_MODES} onChange={(key) => selectMode(key as typeof mode)} />

  {#if !overlay}
    <div class="empty"><Icon name="help" size={18} /><span>Generate a world first. The almanac reads the generated overlay directly.</span></div>
  {:else if mode === 'overview'}
    <div class="cards">
      <div><strong>{fmt(totalPopulation)}</strong><span>estimated population</span></div>
      <div><strong>{fmt(settlements.length)}</strong><span>settlements</span></div>
      <div><strong>{fmt(features.length)}</strong><span>named features</span></div>
      <div><strong>{fmt(kingdoms.length)}</strong><span>kingdoms</span></div>
      <div><strong>{fmt(regions.length)}</strong><span>named biome regions</span></div>
    </div>

    <section>
      <h3>World profile</h3>
      <div class="profile">
        <div><span>Map</span><b>{fmt(world.params.width_mi ?? 0)} × {fmt(world.params.height_mi ?? 0)} mi</b></div>
        <div><span>Cities</span><b>{fmt(cities.length)}</b></div>
        <div><span>Towns</span><b>{fmt(towns.length)}</b></div>
        <div><span>Villages</span><b>{fmt(villages.length)}</b></div>
        <div><span>Kingdoms</span><b>{fmt(kingdoms.length)}</b></div>
        <div><span>Capitals</span><b>{fmt(capitals.length)}</b></div>
        <div><span>Urban population</span><b>{fmt(cityPopulationShare)}%</b></div>
        <div><span>Capital population</span><b>{fmt(capitalPopulationShare)}%</b></div>
        <div><span>Mountain ranges</span><b>{fmt(ranges.length)}</b></div>
        <div><span>Named rivers</span><b>{fmt(rivers.length)}</b></div>
        <div><span>Volcanoes</span><b>{fmt(volcanoes.length)}</b></div>
        <div><span>Total named river miles</span><b>{fmtMiles(totalRiverMiles)} mi</b></div>
        <div><span>Ocean rivers</span><b>{fmt(oceanRivers.length)}</b></div>
        <div><span>Inland-lake rivers</span><b>{fmt(inlandLakeRivers.length)}</b></div>
        <div><span>Dry rivers</span><b>{fmt(dryRivers.length)}</b></div>
        <div><span>Lake-fed rivers</span><b>{fmt(lakeFedRivers.length)}</b></div>
        <div><span>Terminal lakes</span><b>{fmt(terminalLakes.length)}</b></div>
        <div><span>Flow-through lakes</span><b>{fmt(flowThroughLakes.length)}</b></div>
        <div><span>Highest stream order</span><b>{fmt(highestOrderRiver?.stream_order ?? 0)}</b></div>
      </div>
    </section>

    <section>
      <h3>Kingdoms</h3>
      <div class="profile">{#each kingdoms as k (k.id)}<div><span>{k.name}</span><b>{k.capital?.name ?? 'No capital'} · {fmt(k.population)} people</b><small>{fmt(k.cities)} cities · {fmt(k.towns)} towns · {fmt(k.villages)} villages</small></div>{:else}<div class="muted">No political realms generated.</div>{/each}</div>
    </section>

    <section>
      <h3>Natural extremes</h3>
      <div class="extremes">
        {#if highestPeak}<button onclick={() => selectFeature(highestPeak)}><Icon name="mountain" size={15} /><span>Highest peak<small>{name(highestPeak)}</small></span><b>{fmt(Math.round(highestPeak.elev_ft ?? 0))} ft</b></button>{/if}
        {#if longestRiver}<button onclick={() => selectFeature(longestRiver)}><Icon name="river" size={15} /><span>Longest river<small>{name(longestRiver)}</small></span><b>{fmtMiles(longestRiver.length_mi ?? miles(longestRiver.extent_ft))} mi</b></button>{/if}
        {#if largestLake}<button onclick={() => selectFeature(largestLake)}><Icon name="waves" size={15} /><span>Largest lake-type feature<small>{name(largestLake)}</small></span><b>{fmtMiles(largestLake.area_mi2 ?? areaSqMi(largestLake.extent_ft))} mi²</b></button>{/if}
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
  {:else if mode === 'kingdoms'}
    <section class="kingdom-page">
      <div class="kingdom-intro"><h3>Kingdoms of the world</h3><p>Select a realm to highlight its territory on the map.</p></div>
      <div class="kingdom-list">
        {#each kingdoms as k (k.id)}
          <button type="button" class="kingdom-card" onclick={() => onSelectKingdom(k.id)}>
            <span class="swatch" style:background={['#f05a5a','#4f8df7','#62c370','#f2c94c','#9b72e8','#35b9c8','#f08a4b','#e86aa8'][k.id % 8]}></span>
            <span class="kingdom-main"><strong>{k.name}</strong><small>{fmt(k.area_cells)} territory cells · {fmt(k.population)} people</small><small>{fmt(k.cities)} cities · {fmt(k.towns)} towns · {fmt(k.villages)} villages{k.capital ? ' · Capital: ' + k.capital.name : ''}</small></span>
            <Icon name="chevron-right" size={15} />
          </button>
        {:else}<div class="empty">No kingdoms were generated for this world.</div>{/each}
      </div>
    </section>
  {:else if mode === 'facts'}
    <FactPanel facts={facts} shown={shown} onSelect={selectFeature} onRandom={randomFact} />
    <div class="fact-note">Facts are calculated from generated data only. They describe the generated world, not invented history or lore.</div>
  {:else}
    <AlmanacTabs value={ranking} ariaLabel="Ranking categories" items={RANKING_MODES} onChange={(key) => selectRanking(key as typeof ranking)} />


    {#if ranking === 'settlements'}
      <section>
        <h3>Most populated settlements</h3>
        {#each settlementRank.slice(0, 15) as f, i (f.id)}
          <button type="button" class="row" onclick={() => selectFeature(f)}>
            <span class="rank">{i + 1}</span>
            <span class="icon"><Icon name="castle" size={15} /></span>
            <span class="name">{f.name}<small>{f.kind.replace('_', ' ')}</small></span>
            <strong>{fmt(population(f))}</strong>
          </button>
        {:else}<div class="empty">Nothing generated for this ranking.</div>{/each}
      </section>
      <section>
        <h3>Highest-elevation settlements</h3>
        {#each [...settlements].sort((a, b) => (b.elev_ft ?? 0) - (a.elev_ft ?? 0) || population(b) - population(a)).slice(0, 15) as f, i (f.id)}
          <button type="button" class="row" onclick={() => selectFeature(f)}>
            <span class="rank">{i + 1}</span>
            <span class="icon"><Icon name="castle" size={15} /></span>
            <span class="name">{f.name}<small>{f.kind.replace('_', ' ')}</small></span>
            <strong>{fmt(Math.round(f.elev_ft ?? 0))} ft</strong>
          </button>
        {:else}<div class="empty">Nothing generated for this ranking.</div>{/each}
      </section>
    {:else if ranking === 'terrain'}
      <section>
        <h3>Highest peaks and volcanoes</h3>
        {#each peakRank.slice(0, 15) as f, i (f.id)}
          <button type="button" class="row" onclick={() => selectFeature(f)}>
            <span class="rank">{i + 1}</span>
            <span class="icon"><Icon name="mountain" size={15} /></span>
            <span class="name">{f.name}<small>{f.kind.replace('_', ' ')}</small></span>
            <strong>{fmt(Math.round(f.elev_ft ?? 0))} ft</strong>
          </button>
        {:else}<div class="empty">Nothing generated for this ranking.</div>{/each}
      </section>
      {#if ranges.length}
      <section>
        <h3>Largest mountain ranges</h3>
        {#each [...ranges].sort((a, b) => b.extent_ft - a.extent_ft).slice(0, 10) as f, i (f.id)}
          <button type="button" class="row" onclick={() => selectFeature(f)}>
            <span class="rank">{i + 1}</span>
            <span class="icon"><Icon name="mountain" size={15} /></span>
            <span class="name">{f.name}<small>{f.kind.replace('_', ' ')}</small></span>
            <strong>{fmtMiles(f.length_mi ?? miles(f.extent_ft))} mi</strong>
          </button>
        {:else}<div class="empty">Nothing generated for this ranking.</div>{/each}
      </section>
      {/if}
      {#if passes.length}
      <section>
        <h3>Highest passes</h3>
        {#each [...passes].sort((a, b) => (b.elev_ft ?? 0) - (a.elev_ft ?? 0)).slice(0, 10) as f, i (f.id)}
          <button type="button" class="row" onclick={() => selectFeature(f)}>
            <span class="rank">{i + 1}</span>
            <span class="icon"><Icon name="mountain" size={15} /></span>
            <span class="name">{f.name}<small>{f.kind.replace('_', ' ')}</small></span>
            <strong>{fmt(Math.round(f.elev_ft ?? 0))} ft</strong>
          </button>
        {:else}<div class="empty">Nothing generated for this ranking.</div>{/each}
      </section>
      {/if}
      <section>
        <h3>Longest rivers</h3>
        {#each riverRank.slice(0, 15) as f, i (f.id)}
          <button type="button" class="row" onclick={() => selectFeature(f)}>
            <span class="rank">{i + 1}</span>
            <span class="icon"><Icon name="river" size={15} /></span>
            <span class="name">{f.name}<small>{f.kind.replace('_', ' ')}</small></span>
            <strong>{fmtMiles(f.length_mi ?? miles(f.extent_ft))} mi</strong>
          </button>
        {:else}<div class="empty">Nothing generated for this ranking.</div>{/each}
      </section>
      <section>
        <h3>Largest drainage basins</h3>
        {#each [...rivers].sort((a, b) => (b.drainage_area_mi2 ?? 0) - (a.drainage_area_mi2 ?? 0)).slice(0, 15) as f, i (f.id)}
          <button type="button" class="row" onclick={() => selectFeature(f)}>
            <span class="rank">{i + 1}</span>
            <span class="icon"><Icon name="river" size={15} /></span>
            <span class="name">{f.name}<small>{f.kind.replace('_', ' ')}</small></span>
            <strong>{fmtMiles(f.drainage_area_mi2 ?? 0)} mi²</strong>
          </button>
        {:else}<div class="empty">Nothing generated for this ranking.</div>{/each}
      </section>
      <section>
        <h3>Largest lake-type features</h3>
        {#each lakeRank.slice(0, 15) as f, i (f.id)}
          <button type="button" class="row" onclick={() => selectFeature(f)}>
            <span class="rank">{i + 1}</span>
            <span class="icon"><Icon name="waves" size={15} /></span>
            <span class="name">{f.name}<small>{f.kind.replace('_', ' ')}</small></span>
            <strong>{fmtMiles(f.area_mi2 ?? areaSqMi(f.extent_ft))} mi²</strong>
          </button>
        {:else}<div class="empty">Nothing generated for this ranking.</div>{/each}
      </section>
      <section>
        <h3>Highest stream order</h3>
        {#each [...rivers].sort((a, b) => (b.stream_order ?? 0) - (a.stream_order ?? 0) || (b.drainage_area_mi2 ?? 0) - (a.drainage_area_mi2 ?? 0)).slice(0, 15) as f, i (f.id)}
          <button type="button" class="row" onclick={() => selectFeature(f)}>
            <span class="rank">{i + 1}</span>
            <span class="icon"><Icon name="river" size={15} /></span>
            <span class="name">{f.name}<small>{f.kind.replace('_', ' ')}</small></span>
            <strong>{fmt(f.stream_order ?? 0)}</strong>
          </button>
        {:else}<div class="empty">Nothing generated for this ranking.</div>{/each}
      </section>
      <section>
        <h3>Greatest waterfalls</h3>
        {#each [...waterfalls].sort((a, b) => drop(b) - drop(a)).slice(0, 10) as f, i (f.id)}
          <button type="button" class="row" onclick={() => selectFeature(f)}>
            <span class="rank">{i + 1}</span>
            <span class="icon"><Icon name="waves" size={15} /></span>
            <span class="name">{f.name}<small>{f.kind.replace('_', ' ')}</small></span>
            <strong>{fmt(drop(f))} ft</strong>
          </button>
        {:else}<div class="empty">Nothing generated for this ranking.</div>{/each}
      </section>
    {:else if ranking === 'water'}
      <section>
        <h3>Longest rivers</h3>
        {#each riverRank.slice(0, 15) as f, i (f.id)}
          <button type="button" class="row" onclick={() => selectFeature(f)}>
            <span class="rank">{i + 1}</span>
            <span class="icon"><Icon name="river" size={15} /></span>
            <span class="name">{f.name}<small>{f.kind.replace('_', ' ')}</small></span>
            <strong>{fmtMiles(f.length_mi ?? miles(f.extent_ft))} mi</strong>
          </button>
        {:else}<div class="empty">Nothing generated for this ranking.</div>{/each}
      </section>
      <section>
        <h3>Largest drainage basins</h3>
        {#each [...rivers].sort((a, b) => (b.drainage_area_mi2 ?? 0) - (a.drainage_area_mi2 ?? 0)).slice(0, 15) as f, i (f.id)}
          <button type="button" class="row" onclick={() => selectFeature(f)}>
            <span class="rank">{i + 1}</span>
            <span class="icon"><Icon name="river" size={15} /></span>
            <span class="name">{f.name}<small>{f.kind.replace('_', ' ')}</small></span>
            <strong>{fmtMiles(f.drainage_area_mi2 ?? 0)} mi²</strong>
          </button>
        {:else}<div class="empty">Nothing generated for this ranking.</div>{/each}
      </section>
      <section>
        <h3>Largest lakes</h3>
        {#each lakeRank.slice(0, 15) as f, i (f.id)}
          <button type="button" class="row" onclick={() => selectFeature(f)}>
            <span class="rank">{i + 1}</span>
            <span class="icon"><Icon name="waves" size={15} /></span>
            <span class="name">{f.name}<small>{f.kind.replace('_', ' ')}</small></span>
            <strong>{fmtMiles(f.area_mi2 ?? areaSqMi(f.extent_ft))} mi²</strong>
          </button>
        {:else}<div class="empty">Nothing generated for this ranking.</div>{/each}
      </section>
      <section>
        <h3>Highest stream order</h3>
        {#each [...rivers].sort((a, b) => (b.stream_order ?? 0) - (a.stream_order ?? 0) || (b.drainage_area_mi2 ?? 0) - (a.drainage_area_mi2 ?? 0)).slice(0, 15) as f, i (f.id)}
          <button type="button" class="row" onclick={() => selectFeature(f)}>
            <span class="rank">{i + 1}</span>
            <span class="icon"><Icon name="river" size={15} /></span>
            <span class="name">{f.name}<small>{f.kind.replace('_', ' ')}</small></span>
            <strong>{fmt(f.stream_order ?? 0)}</strong>
          </button>
        {:else}<div class="empty">Nothing generated for this ranking.</div>{/each}
      </section>
      <section>
        <h3>Greatest waterfalls</h3>
        {#each [...waterfalls].sort((a, b) => drop(b) - drop(a)).slice(0, 10) as f, i (f.id)}
          <button type="button" class="row" onclick={() => selectFeature(f)}>
            <span class="rank">{i + 1}</span>
            <span class="icon"><Icon name="waves" size={15} /></span>
            <span class="name">{f.name}<small>{f.kind.replace('_', ' ')}</small></span>
            <strong>{fmt(drop(f))} ft</strong>
          </button>
        {:else}<div class="empty">Nothing generated for this ranking.</div>{/each}
      </section>
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


<style>
  .almanac { display:flex; flex-direction:column; gap:10px; }
  .hero { padding-bottom:2px; }
  .eyebrow { font-size:10px; letter-spacing:.1em; color:var(--ink-3); }
  .hero-line { display:flex; align-items:center; gap:8px; }
  .hero-line > div { min-width:0; flex:1; }
  h2 { margin:1px 0 2px; font-size:20px; }
  h3 { margin:0 0 6px; font-size:11px; text-transform:uppercase; letter-spacing:.06em; color:var(--ink-2); }
  p { margin:0; color:var(--ink-2); font-size:12px; line-height:1.4; }
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
  .fact-note { color:var(--ink-3); font-size:10px; line-height:1.4; padding:3px; }
  .row { all:unset; box-sizing:border-box; display:grid; grid-template-columns:22px 20px 1fr auto; align-items:center; gap:6px; width:100%; min-height:38px; padding:3px 4px; cursor:pointer; border-radius:var(--radius-sm); }
  .row:hover,.row:focus-visible { background:var(--btn-hover); }
  .rank { color:var(--ink-3); font:10px var(--mono); text-align:right; }
  .icon { color:var(--ink-2); display:flex; }
  .name { min-width:0; display:flex; flex-direction:column; overflow:hidden; text-align:left; font-size:11px; }
  .name small { color:var(--ink-3); font-size:10px; }
  .kingdom-page { display:flex; flex-direction:column; gap:8px; }
  .kingdom-intro p { margin:0; color:var(--ink-2); font-size:11px; line-height:1.4; }
  .kingdom-list { display:flex; flex-direction:column; gap:4px; }
  .kingdom-card { display:grid; grid-template-columns:12px 1fr 16px; gap:8px; align-items:center; width:100%; padding:9px 8px; border:1px solid var(--line-faint); border-radius:var(--radius-sm); background:var(--btn-hover); color:var(--ink); text-align:left; cursor:pointer; }
  .kingdom-card:hover { border-color:var(--accent); background:var(--btn-on); }
  .swatch { width:12px; height:34px; border-radius:4px; box-shadow:inset 0 0 0 1px rgba(0,0,0,.12); }
  .kingdom-main { min-width:0; display:flex; flex-direction:column; gap:2px; }
  .kingdom-main strong { font-size:12px; }
  .kingdom-main small { color:var(--ink-3); font-size:10px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .empty { display:flex; gap:8px; align-items:center; color:var(--ink-3); padding:12px 4px; font-size:12px; }
  .loading { min-height:90px; justify-content:center; }
  .spinner { width:14px; height:14px; border:2px solid var(--line-soft); border-top-color:var(--accent); border-radius:50%; animation:spin .7s linear infinite; }
  @keyframes spin { to { transform:rotate(360deg); } }
  .muted { color:var(--ink-3); font-size:11px; padding:4px; }
</style>

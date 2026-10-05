<script lang="ts">
  import type { Feature, Overlay } from '../../gen/protocol';
  import Icon from '../Icon.svelte';

  interface Props {
    overlay: Overlay;
    onSelect: (f: Feature) => void;
  }

  let { overlay, onSelect }: Props = $props();
  let mode = $state<'settlements' | 'geography'>('settlements');
  let settlementKind = $state<'all' | 'metropolis' | 'city' | 'town' | 'village'>('all');

  const settlements = $derived(overlay.features.filter((f) => ['metropolis', 'city', 'town', 'village'].includes(f.kind)));
  const population = (f: Feature) => Number(/pop\. ([\d,]+)/.exec(f.detail ?? '')?.[1]?.replace(/,/g, '') ?? 0);
  const label = (f: Feature) => f.name;
  const fmt = (n: number) => n.toLocaleString();
  const miles = (ft: number) => (ft / 5280).toLocaleString(undefined, { maximumFractionDigits: 0 });
  const areaSqMi = (ft: number) => ((ft * ft) / (5280 * 5280)).toLocaleString(undefined, { maximumFractionDigits: 0 });
  const drop = (f: Feature) => Number(/drop ~([\d,]+) ft/.exec(f.detail ?? '')?.[1]?.replace(/,/g, '') ?? 0);

  const filteredSettlements = $derived(
    settlements
      .filter((f) => settlementKind === 'all' || f.kind === settlementKind)
      .sort((a, b) => population(b) - population(a) || a.name.localeCompare(b.name)),
  );

  const peaks = $derived(
    overlay.features.filter((f) => f.kind === 'peak' || f.kind === 'volcano').sort((a, b) => (b.elev_ft ?? 0) - (a.elev_ft ?? 0)),
  );
  const rivers = $derived(
    overlay.features.filter((f) => f.kind === 'river').sort((a, b) => b.extent_ft - a.extent_ft),
  );
  const lakes = $derived(
    overlay.features.filter((f) => f.kind === 'lake' || f.kind === 'salt_lake').sort((a, b) => b.extent_ft - a.extent_ft),
  );
  const waterfalls = $derived(
    overlay.features.filter((f) => f.kind === 'waterfall').sort((a, b) => drop(b) - drop(a)),
  );
  const landmasses = $derived(
    overlay.features.filter((f) => f.kind === 'continent' || f.kind === 'island').sort((a, b) => b.extent_ft - a.extent_ft),
  );

  const totalPopulation = $derived(settlements.reduce((n, f) => n + population(f), 0));
  const cityCount = $derived(settlements.filter((f) => f.kind === 'city' || f.kind === 'metropolis').length);
  const townCount = $derived(settlements.filter((f) => f.kind === 'town').length);
  const villageCount = $derived(settlements.filter((f) => f.kind === 'village').length);

  function select(f: Feature) {
    onSelect(f);
  }
</script>

<div class="stats">
  <div class="intro">
    <div>
      <div class="eyebrow">WORLD AT A GLANCE</div>
      <h2>World statistics</h2>
      <p>Rankings are derived from the generated world, not stored separately, so they stay deterministic with the seed.</p>
    </div>
  </div>

  <div class="summary">
    <div><b>{fmt(totalPopulation)}</b><span>estimated population</span></div>
    <div><b>{cityCount}</b><span>cities</span></div>
    <div><b>{townCount}</b><span>towns</span></div>
    <div><b>{villageCount}</b><span>villages</span></div>
  </div>

  <div class="seg">
    <button class:on={mode === 'settlements'} onclick={() => (mode = 'settlements')}>Settlements</button>
    <button class:on={mode === 'geography'} onclick={() => (mode = 'geography')}>Geography</button>
  </div>

  {#if mode === 'settlements'}
    <div class="filters">
      {#each [['all', 'All'], ['metropolis', 'Metropolises'], ['city', 'Cities'], ['town', 'Towns'], ['village', 'Villages']] as [value, text] (value)}
        <button class:on={settlementKind === value} onclick={() => (settlementKind = value as typeof settlementKind)}>{text}</button>
      {/each}
    </div>

    <section>
      <h3>Most populated</h3>
      {#each filteredSettlements.slice(0, 15) as f, i (f.id)}
        <button class="row" onclick={() => select(f)}>
          <span class="rank">{i + 1}</span>
          <span class="icon"><Icon name={f.kind === 'village' ? 'tree' : f.kind === 'town' ? 'building' : 'castle'} size={16} /></span>
          <span class="name">{label(f)}<small>{f.detail?.split(',')[0] ?? f.kind}</small></span>
          <strong>{fmt(population(f))}</strong>
        </button>
      {:else}
        <div class="empty">No settlements generated.</div>
      {/each}
    </section>
  {:else}
    <section>
      <h3>Highest peaks</h3>
      {#each peaks.slice(0, 10) as f, i (f.id)}
        <button class="row" onclick={() => select(f)}><span class="rank">{i + 1}</span><span class="icon"><Icon name="mountain" size={16} /></span><span class="name">{label(f)}<small>{f.kind}</small></span><strong>{fmt(drop(f))} ft drop</strong></button>
      {/each}
    </section>

    <section>
      <h3>Longest rivers</h3>
      {#each rivers.slice(0, 10) as f, i (f.id)}
        <button class="row" onclick={() => select(f)}><span class="rank">{i + 1}</span><span class="icon"><Icon name="river" size={16} /></span><span class="name">{label(f)}<small>river</small></span><strong>{areaSqMi(f.extent_ft)} mi²</strong></button>
      {/each}
    </section>

    <section>
      <h3>Largest lakes</h3>
      {#each lakes.slice(0, 10) as f, i (f.id)}
        <button class="row" onclick={() => select(f)}><span class="rank">{i + 1}</span><span class="icon"><Icon name="waves" size={16} /></span><span class="name">{label(f)}<small>{f.kind.replace('_', ' ')}</small></span><strong>{areaSqMi(f.extent_ft)} mi²</strong></button>
      {/each}
    </section>

    <section>
      <h3>Greatest waterfalls</h3>
      {#each waterfalls.slice(0, 10) as f, i (f.id)}
        <button class="row" onclick={() => select(f)}><span class="rank">{i + 1}</span><span class="icon"><Icon name="waves" size={16} /></span><span class="name">{label(f)}<small>{f.detail ?? 'waterfall'}</small></span><strong>{fmt(Math.round(f.elev_ft ?? 0))} ft</strong></button>
      {/each}
    </section>

    <section>
      <h3>Largest landmasses</h3>
      {#each landmasses.slice(0, 10) as f, i (f.id)}
        <button class="row" onclick={() => select(f)}><span class="rank">{i + 1}</span><span class="icon"><Icon name="land" size={16} /></span><span class="name">{label(f)}<small>{f.kind}</small></span><strong>{miles(f.extent_ft)} mi</strong></button>
      {/each}
    </section>
  {/if}
</div>

<style>
  .stats { display:flex; flex-direction:column; gap:10px; }
  .intro { padding-bottom:4px; }
  .eyebrow { font-size:10px; letter-spacing:.08em; color:var(--ink-3); }
  h2 { margin:1px 0 2px; font-size:20px; }
  p { margin:0; color:var(--ink-2); font-size:12px; line-height:1.4; }
  .summary { display:grid; grid-template-columns:repeat(2,1fr); gap:4px; }
  .summary div { padding:8px; background:var(--btn-hover); border:1px solid var(--line-faint); border-radius:var(--radius-sm); }
  .summary b,.summary span { display:block; }
  .summary b { font:17px var(--mono); }
  .summary span { margin-top:1px; color:var(--ink-3); font-size:10px; }
  .seg,.filters { display:flex; gap:3px; overflow:auto; }
  .seg button,.filters button { border:1px solid var(--line); background:transparent; color:var(--ink-2); border-radius:var(--radius-sm); padding:5px 8px; white-space:nowrap; cursor:pointer; font:11px var(--font); }
  .seg button.on,.filters button.on { background:var(--accent); color:var(--accent-ink); border-color:var(--accent); }
  section { border-top:1px solid var(--line-faint); padding-top:7px; }
  h3 { margin:0 0 4px; font-size:12px; text-transform:uppercase; letter-spacing:.05em; color:var(--ink-2); }
  .row { all:unset; box-sizing:border-box; display:grid; grid-template-columns:24px 22px 1fr auto; align-items:center; gap:6px; width:100%; min-height:38px; padding:3px 4px; cursor:pointer; border-radius:var(--radius-sm); }
  .row:hover,.row:focus-visible { background:var(--btn-hover); }
  .rank { color:var(--ink-3); font:11px var(--mono); text-align:right; }
  .icon { color:var(--ink-2); display:flex; }
  .name { min-width:0; display:flex; flex-direction:column; overflow:hidden; text-align:left; font-size:12px; }
  .name small { color:var(--ink-3); font-size:10px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .row strong { font:12px var(--mono); white-space:nowrap; }
  .empty { color:var(--ink-3); padding:8px 4px; font-style:italic; }
</style>

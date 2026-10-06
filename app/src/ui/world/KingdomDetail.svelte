<script lang="ts">
  import type { Feature } from '../../gen/protocol';
  import type { KingdomRecord } from '../../gen/kingdoms';
  import Icon from '../Icon.svelte';

  interface Props {
    kingdom: KingdomRecord;
    allKingdoms: KingdomRecord[];
    colour: string;
    onBack: () => void;
    onSelectKingdom: (id: number) => void;
    onSelectFeature: (feature: Feature) => void;
  }

  let { kingdom, allKingdoms, colour, onBack, onSelectKingdom, onSelectFeature }: Props = $props();
  const fmt = (n: number) => n.toLocaleString();
  const neighbour = (id: number) => allKingdoms.find((item) => item.id === id) ?? null;
  const settlementType = (feature: Feature) => feature.kind === 'metropolis' ? 'Metropolis' : feature.kind[0]?.toUpperCase() + feature.kind.slice(1);
</script>

<div class="detail">
  <button class="back" type="button" onclick={onBack}><Icon name="chevron-left" size={14} /> Kingdoms</button>

  <header class="hero">
    <div class="crest" style="background:{colour}"><Icon name="building" size={21} /></div>
    <div class="title">
      <span class="eyebrow">KINGDOM</span>
      <h3>{kingdom.name}</h3>
      <p>Generated political realm · {fmt(kingdom.areaCells)} territory cells</p>
    </div>
  </header>

  <div class="stats">
    <div><strong>{fmt(kingdom.population)}</strong><span>population</span></div>
    <div><strong>{fmt(kingdom.areaCells)}</strong><span>territory cells</span></div>
    <div><strong>{fmt(kingdom.settlements.length)}</strong><span>settlements</span></div>
    <div><strong>{fmt(kingdom.neighbours.length)}</strong><span>neighbours</span></div>
  </div>

  {#if kingdom.capital}
    <section>
      <h4>Capital</h4>
      <button class="feature-card" type="button" onclick={() => onSelectFeature(kingdom.capital!)}>
        <span class="feature-icon"><Icon name="castle" size={16} /></span>
        <span><strong>{kingdom.capital.name}</strong><small>{settlementType(kingdom.capital)} · {fmt(kingdom.capital.detail ? Number(/pop\. ([\d,]+)/.exec(kingdom.capital.detail)?.[1]?.replace(/,/g, '') ?? 0) : 0)} people</small></span>
        <Icon name="chevron-right" size={14} />
      </button>
    </section>
  {:else}
    <section><h4>Capital</h4><p class="muted">No generated capital is assigned to this kingdom.</p></section>
  {/if}

  <section>
    <h4>Settlements</h4>
    <div class="settlements">
      {#each kingdom.settlements.slice().sort((a, b) => {
        const pa = Number(/pop\. ([\d,]+)/.exec(a.detail ?? '')?.[1]?.replace(/,/g, '') ?? 0);
        const pb = Number(/pop\. ([\d,]+)/.exec(b.detail ?? '')?.[1]?.replace(/,/g, '') ?? 0);
        return pb - pa || a.name.localeCompare(b.name);
      }).slice(0, 12) as settlement (settlement.id)}
        <button class="feature-card" type="button" onclick={() => onSelectFeature(settlement)}>
          <span class="feature-icon"><Icon name="castle" size={15} /></span>
          <span><strong>{settlement.name}</strong><small>{settlementType(settlement)}</small></span>
          <Icon name="chevron-right" size={14} />
        </button>
      {:else}<p class="muted">No named settlements are attached to this realm.</p>{/each}
    </div>
  </section>

  <section>
    <h4>Neighbours</h4>
    {#if kingdom.neighbours.length}
      <div class="neighbours">
        {#each kingdom.neighbours as id (id)}
          {@const other = neighbour(id)}
          {#if other}
            <button class="neighbour" type="button" onclick={() => onSelectKingdom(other.id)}>
              <span class="dot" style="background:{other.id === kingdom.id ? colour : ''}></span>
              <span>{other.name}</span><Icon name="chevron-right" size={13} />
            </button>
          {/if}
        {/each}
      </div>
    {:else}<p class="muted">No neighbouring realm boundary is recorded.</p>{/if}
  </section>

  <section class="note">
    <Icon name="help" size={14} />
    <span>These values come from the generated political overlay. No history, diplomacy, or lore is invented here.</span>
  </section>
</div>

<style>
  .detail { display:flex; flex-direction:column; gap:10px; }
  .back { all:unset; display:flex; align-items:center; gap:3px; color:var(--ink-3); font-size:11px; cursor:pointer; width:max-content; padding:2px 0; }
  .back:hover { color:var(--ink); }
  .hero { display:flex; gap:10px; align-items:center; padding:3px 0 4px; }
  .crest { width:42px; height:48px; border-radius:7px; display:flex; align-items:center; justify-content:center; color:#fff; box-shadow:inset 0 0 0 1px rgba(0,0,0,.14); }
  .title { min-width:0; }
  .eyebrow { color:var(--ink-3); font-size:9px; letter-spacing:.1em; }
  h3 { margin:1px 0 2px; font-size:19px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .title p { margin:0; color:var(--ink-3); font-size:10px; }
  section { border-top:1px solid var(--line-faint); padding-top:8px; }
  h4 { margin:0 0 6px; font-size:10px; text-transform:uppercase; letter-spacing:.06em; color:var(--ink-2); }
  .stats { display:grid; grid-template-columns:1fr 1fr; gap:4px; }
  .stats > div { padding:8px; border:1px solid var(--line-faint); border-radius:var(--radius-sm); background:var(--btn-hover); }
  .stats strong,.stats span { display:block; }
  .stats strong { font:15px var(--mono); }
  .stats span { margin-top:2px; color:var(--ink-3); font-size:9px; }
  .feature-card,.neighbour { all:unset; box-sizing:border-box; display:grid; grid-template-columns:24px 1fr 15px; gap:7px; align-items:center; width:100%; padding:7px 6px; border:1px solid var(--line-faint); border-radius:var(--radius-sm); cursor:pointer; }
  .feature-card:hover,.neighbour:hover { background:var(--btn-hover); border-color:var(--accent); }
  .feature-icon { display:flex; align-items:center; justify-content:center; color:var(--ink-2); }
  .feature-card > span:nth-child(2) { min-width:0; display:flex; flex-direction:column; }
  .feature-card strong,.neighbour span { font-size:11px; text-align:left; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .feature-card small { color:var(--ink-3); font-size:9px; }
  .settlements,.neighbours { display:flex; flex-direction:column; gap:3px; }
  .neighbour { grid-template-columns:10px 1fr 15px; }
  .dot { width:8px; height:8px; border-radius:50%; background:var(--ink-3); }
  .muted { margin:0; color:var(--ink-3); font-size:10px; line-height:1.4; }
  .note { display:flex; gap:6px; align-items:flex-start; color:var(--ink-3); font-size:9px; line-height:1.4; }
</style>

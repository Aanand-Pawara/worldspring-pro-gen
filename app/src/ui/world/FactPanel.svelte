<script lang="ts">
  import type { Feature } from '../../gen/protocol';
  import Icon from '../Icon.svelte';
  import type { AlmanacFact } from './almanac/model';

  interface Props { facts: AlmanacFact[]; shown: number; onSelect: (feature: Feature) => void; onRandom: () => void; }
  let { facts, shown, onSelect, onRandom }: Props = $props();
  const current = $derived(facts.length ? facts[((shown % facts.length) + facts.length) % facts.length] : null);
</script>

<div class="facts">
  {#if current}
    <article class="fact-card">
      <div class="tag">{current.tag}</div>
      <div class="fact">{current.text}</div>
      {#if current.feature}
        <button class="show" type="button" onclick={(e) => { e.stopPropagation(); onSelect(current!.feature!); }}>
          <Icon name="pin" size={14} /> Show on map
        </button>
      {/if}
    </article>
    <div class="counter">Fact {((shown % facts.length) + facts.length) % facts.length + 1} of {facts.length}</div>
    <button class="next" type="button" onclick={(e) => { e.stopPropagation(); onRandom(); }} disabled={facts.length < 2}>
      <Icon name="dice" size={14} /> Generate another fact
    </button>
  {:else}
    <div class="empty">Generate a world to populate the fact reference.</div>
  {/if}
</div>

<style>
  .facts{display:flex;flex-direction:column;gap:8px}
  .fact-card{padding:14px;border:1px solid var(--line);border-radius:var(--radius);background:var(--btn-hover)}
  .tag{margin-bottom:8px;font-size:10px;letter-spacing:.08em;font-weight:bold;color:var(--ink-3)}
  .fact{font-size:16px;line-height:1.5}
  .show,.next{display:inline-flex;align-items:center;gap:5px;border:0;color:var(--ink-2);font:11px var(--font);cursor:pointer}
  .show{margin-top:12px;background:none;padding:3px 0}
  .next{justify-content:center;background:var(--btn-hover);border:1px solid var(--line);border-radius:var(--radius-sm);padding:7px 9px}
  .show:hover,.next:hover{color:var(--ink)}
  .next:disabled{opacity:.5;cursor:default}
  .counter{color:var(--ink-3);font:10px var(--mono);text-align:center}
  .empty{display:flex;gap:8px;align-items:center;color:var(--ink-3);padding:12px 4px;font-size:12px}
</style>

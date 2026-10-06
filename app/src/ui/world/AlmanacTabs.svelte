<script lang="ts">
  import type { IconName } from '../icons';
  import Icon from '../Icon.svelte';

  export interface TabItem { key: string; label: string; icon?: IconName; }
  interface Props { items: TabItem[]; value: string; ariaLabel: string; onChange: (key: string) => void; }
  let { items, value, ariaLabel, onChange }: Props = $props();
</script>

<nav class="tabs" aria-label={ariaLabel}>
  {#each items as item (item.key)}
    <button type="button" class:on={value === item.key} aria-pressed={value === item.key} onclick={(e) => { e.stopPropagation(); onChange(item.key); }}>
      {#if item.icon}<Icon name={item.icon} size={14} />{/if}
      <span>{item.label}</span>
    </button>
  {/each}
</nav>

<style>
  .tabs { display:flex; gap:3px; overflow:auto; padding-bottom:1px; }
  .tabs button { display:inline-flex; align-items:center; justify-content:center; gap:4px; border:1px solid var(--line); background:transparent; color:var(--ink-2); border-radius:var(--radius-sm); padding:6px 8px; white-space:nowrap; cursor:pointer; font:11px var(--font); }
  .tabs button.on { background:var(--accent); color:var(--accent-ink); border-color:var(--accent); }
</style>

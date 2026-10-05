<script lang="ts">
  // Bottom left: the scale bar, and on screens with a pointer, where the cursor is (the square
  // under it on a battlemap). Panels stacked above it read its height from --readout-h.
  import type { HudState } from '../../render/MapView';

  interface Props {
    hud: HudState;
    /** Inside a building or site (no buildings to go into). */
    inside?: boolean;
  }
  let { hud, inside = false }: Props = $props();

  const MILE = 5280;

  // Scale bar: the largest 1/2/5 × 10^k length that fits in ~140 px.
  const scale = $derived.by(() => {
    const maxFt = 140 * hud.ftPerPx;
    const useMiles = maxFt >= MILE;
    const unit = useMiles ? MILE : 1;
    const max = maxFt / unit;
    const pow = 10 ** Math.floor(Math.log10(max));
    const nice = [5, 2, 1].map((m) => m * pow).find((v) => v <= max) ?? pow;
    return { px: (nice * unit) / hud.ftPerPx, label: `${nice.toLocaleString()} ${useMiles ? 'mi' : 'ft'}` };
  });

  let height = $state(0);
  $effect(() => {
    document.documentElement.style.setProperty('--readout-h', `${height}px`);
  });

  const fmt = (n: number, d = 0) => n.toLocaleString(undefined, { maximumFractionDigits: d, minimumFractionDigits: d });
</script>

<div class="readout" bind:clientHeight={height}>
  <div class="scalebar" style:width="{scale.px}px"></div>
  <div class="mono">{scale.label}</div>
  {#if hud.cursor}
    <div class="cursor mono">
      {fmt(hud.cursor.x / MILE, 2)} mi, {fmt(hud.cursor.y / MILE, 2)} mi
      {#if hud.cursor.elev !== null}· {fmt(hud.cursor.elev)} ft{/if}
    </div>
    {#if hud.cursor.square}
      {@const sq = hud.cursor.square}
      <div class="square">
        <b>{fmt(sq.elevationFt)} ft</b> (tier {sq.tier}) · {sq.surface}
        {#if sq.object}<br /><b>{sq.object.name}</b>: {sq.object.cover}{#if sq.object.notes} · {sq.object.notes}{/if}{/if}
      </div>
    {/if}
  {/if}
  {#if hud.tier === 'Battlemap' && !inside}<div class="hint">Double-click a building to go in</div>{/if}
</div>

<style>
  .readout {
    position: fixed;
    left: 12px;
    bottom: 12px;
    z-index: var(--z-hud);
    max-width: 380px;
    padding: 5px 9px;
    background: var(--paper);
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow);
    color: var(--ink);
    font: 12px/1.45 var(--font);
    pointer-events: none;
  }
  .mono {
    font: 11px/1.45 var(--mono);
  }
  .scalebar {
    height: 6px;
    border: 1.5px solid var(--ink);
    border-top: none;
    margin-bottom: 2px;
  }
  .square {
    margin-top: 3px;
  }
  .hint {
    font-size: 11px;
    color: var(--ink-3);
  }
  /* Touch screens have no cursor to report (and double-tap is in the shortcuts list). */
  @media (hover: none) {
    .cursor,
    .square,
    .hint {
      display: none;
    }
  }
  :global([data-layout='phone']) .readout {
    left: 8px;
    bottom: calc(var(--tabbar-h, 0px) + var(--sheet-h, 0px) + 8px);
    padding: 3px 7px;
  }
</style>

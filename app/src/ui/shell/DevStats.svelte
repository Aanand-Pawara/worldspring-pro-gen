<script lang="ts">
  // Performance stats for development (off unless asked for, or a benchmark is running): frame
  // rate, tiles, the generator's queue, Run benchmark, and the last benchmark's results.
  import type { BenchResult } from '../../dev/bench';
  import type { HudState } from '../../render/MapView';

  interface Props {
    hud: HudState;
    bench: BenchResult | null;
    benchRunning: boolean;
    onBench: () => void;
    onClose: () => void;
  }
  let { hud, bench, benchRunning, onBench, onClose }: Props = $props();

  const fmt = (n: number, d = 0) => n.toLocaleString(undefined, { maximumFractionDigits: d, minimumFractionDigits: d });
</script>

<div class="stats">
  <button class="x" onclick={onClose} aria-label="Hide performance stats" title="Hide performance stats (`)">×</button>
  <div>{fmt(hud.fps)} fps</div>
  <div>{hud.tier} · L{hud.layer.level} · {fmt(hud.layer.targetReady)}/{fmt(hud.layer.targetTiles)} tiles</div>
  {#if hud.gen}
    <div>gen {hud.gen.pending} queued · {hud.gen.inflight} busy · {fmt(hud.gen.avgMs, 1)} ms/tile</div>
  {/if}
  <div>gpu tiles {hud.layer.gpuTiles}</div>
  <button class="bench" onclick={onBench} disabled={benchRunning}>{benchRunning ? 'Benchmarking…' : 'Run benchmark'}</button>
</div>

{#if bench}
  <div class="results">
    <div class="title">Benchmark</div>
    <div>{fmt(bench.avgFps, 1)} fps avg · {fmt(bench.low1Fps, 1)} fps 1% low</div>
    <div>p95 {fmt(bench.p95Ms, 1)} ms · max {fmt(bench.maxMs, 1)} ms</div>
    <div>detail after stop: {bench.detailLatencyMs.join(' / ')} ms</div>
    {#each bench.stops as s, i (i)}
      <div class="small">
        stop {i + 1}: {s.missingAtStop}/{s.tiles} missing · recv {s.receivedMs} · upload {s.uploadedMs} · visible {s.visibleMs} ms
      </div>
    {/each}
    <div>{bench.viewport} @{bench.resolution}x · L{bench.maxLevel}</div>
    <div class="small">{bench.gpu}</div>
  </div>
{/if}

<style>
  .stats,
  .results {
    position: fixed;
    z-index: var(--z-hud);
    padding: 7px 10px;
    background: var(--paper);
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    box-shadow: var(--shadow);
    color: var(--ink);
    font: 12px/1.45 var(--mono);
  }
  .stats {
    top: calc(12px + var(--bar-h, 44px) + 8px);
    right: calc(var(--dock-w, 0px) + 12px);
    padding-right: 26px;
  }
  .results {
    bottom: 12px;
    right: calc(var(--dock-w, 0px) + var(--controls-w, 0px) + 12px);
    max-width: min(420px, calc(100vw - 24px));
  }
  /* Phones: under the search bar (the section tabs are at the bottom), and above the sheet. */
  :global([data-layout='phone']) .stats {
    top: calc(max(8px, env(safe-area-inset-top)) + var(--top-h, 60px) + 8px);
    right: 8px;
    font-size: 11px;
  }
  :global([data-layout='phone']) .results {
    right: 8px;
    bottom: calc(var(--tabbar-h, 0px) + var(--sheet-h, 0px) + 8px);
    max-width: calc(100vw - 16px);
    font-size: 11px;
  }
  .x {
    position: absolute;
    top: 2px;
    right: 4px;
    border: none;
    background: none;
    font-size: 16px;
    line-height: 1;
    cursor: pointer;
    color: var(--ink-2);
  }
  .bench {
    margin-top: 4px;
    font: inherit;
    font-size: 11px;
    background: var(--btn);
    border: 1px solid var(--line);
    border-radius: 3px;
    padding: 3px 8px;
    cursor: pointer;
  }
  .bench:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .title {
    font-weight: bold;
    letter-spacing: 0.03em;
  }
  .small {
    font-size: 11px;
    opacity: 0.8;
  }
</style>

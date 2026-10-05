<script lang="ts">
  // Asked before work would be dropped: new sketch strokes (generate them, drop them, or keep
  // sketching), or a site's unsaved design (save it, drop it, or keep designing).
  import type { Ask } from './layout.svelte';

  interface Props {
    ask: Ask;
    /** Save is refused: the design breaks a rule a site must keep. */
    blocked?: boolean;
    onKeep: () => void;
    onDrop: () => void;
    onCommit: () => void;
  }
  let { ask, blocked = false, onKeep, onDrop, onCommit }: Props = $props();
</script>

<div class="ask" role="alertdialog" aria-label="Unsaved changes">
  <div class="text">
    {#if ask.kind === 'sketch'}<b>Your sketch isn't generated yet.</b> Its new strokes would be lost.
    {:else}<b>This site's design isn't saved.</b> The changes would be lost.{/if}
  </div>
  <div class="ws-row buttons">
    <button class="ws-btn quiet" onclick={onKeep}>{ask.kind === 'sketch' ? 'Keep sketching' : 'Keep designing'}</button>
    <button class="ws-btn" onclick={onDrop}>Discard</button>
    {#if ask.kind === 'design'}
      <button class="ws-btn primary" onclick={onCommit} disabled={blocked} title={blocked ? 'Fix the problems marked ✕ first' : ''}>Save</button>
    {:else if ask.generate !== false}
      <button class="ws-btn primary" onclick={onCommit}>Generate</button>
    {/if}
  </div>
</div>

<style>
  .ask {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px 12px;
    background: #fff4e0;
    border: 1px solid var(--warn);
    border-radius: var(--radius);
    color: var(--ink);
    font: 13px/1.4 var(--font);
    box-shadow: var(--shadow);
  }
  .buttons {
    justify-content: flex-end;
    flex-wrap: wrap;
  }
</style>

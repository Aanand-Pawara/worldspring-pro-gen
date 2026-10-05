// The world settings as edited in the World panel, not generated yet (the sketch preview uses
// them too). Kept in App, so leaving the panel doesn't throw them away.
import type { WorldFile, WorldParams } from '../../gen/protocol';
import { DEFAULT_PARAMS } from '../../world/world';

export class WorldDraft {
  seed = $state(0);
  p = $state<WorldParams>({ ...DEFAULT_PARAMS });
  private synced = '';

  /** Follow the world when it is a different one (not on every edit: that would throw away
   * settings changed and not generated yet). */
  follow(world: WorldFile) {
    const key = JSON.stringify([world.seed, world.params]);
    if (key === this.synced) return;
    this.synced = key;
    this.seed = world.seed;
    this.p = { ...DEFAULT_PARAMS, ...world.params, biome_weights: { ...world.params.biome_weights } };
  }

  /** The settings differ from the world's (Generate would make another world). */
  changed(world: WorldFile): boolean {
    return JSON.stringify([this.seed >>> 0, $state.snapshot(this.p)]) !== JSON.stringify([world.seed, { ...DEFAULT_PARAMS, ...world.params, biome_weights: { ...world.params.biome_weights } }]);
  }
}

# The Underdark — requirements (tabled)

Status: **tabled.** It was built across five commits, then reverted from `master` at the user's
request. The full implementation is preserved on the branch **`underdark-wip`** (tip `9587882`,
branched from `b078702`). The notes on that build below are what to reuse or avoid when it is
picked up again.

## 1. Original requirements (implementation plan)

From the plan (`~/.claude/plans/pasted-content-id-4ec8-help-me-lexical-raccoon.md`):

- **Scope decision:** underground means local sites (dungeons, caves, mines, sewers, catacombs,
  lava tubes) **plus a continent-scale Underdark layer**.
- **Architecture:** `UnderdarkTile(L, x, y)` is a job like `Terrain` and `Battlemap`: a pure
  function of (world file, job key) in the dependency DAG, depending only on coarser levels or
  parent features.
- **Generator** (`under/underdark`): the same tile/LOD pipeline as the surface, with
  `Layer::Underdark`. It has:
  - a floor height field;
  - a passability field (ridged + Worley caverns);
  - underground rivers and lakes;
  - fungal-forest biomes.

  Deep caves and mines link to the nearest Underdark node.
- **Theme 22, Underdark** (Appendix A):
  - Cover: giant stalagmites, fungal trees.
  - Elevation: multi-level ledges, lakes.
  - Hazards, props and atmosphere: spores, bioluminescence.

  Theme 23 (volcanic/lava: basalt, obsidian, ledges over lava, vents) also applies down there.
- **Milestone M6:** "Underdark layer; depth selector + surface ghost; asset batch 4". Exit gate:
  every site ends in a boss chamber, and themes 1, 2, 11, 20, 22, 23 and 24 are visible.
- **Standing rules that apply:**
  - Determinism: native output is byte-identical to WASM.
  - Seams: tiles are a function of global lattice indices.
  - Bump `GEN_VERSION` when output changes.
  - Only vital tests.
  - The perf floor is ≥ 30 fps 1% lows on the Iris Xe laptop.

## 2. The user's requests and feedback, in order

1. "Go ahead and start on underdark."
2. "Can we make the underdark a toggleable option in the menu?" This was first read as a
   surface/Underdark **view** switch.
3. "How do players access the underdark? Can you give me an overworld location?" The answer
   was: through surface caves, mines and lava tubes, down a passage from their deepest level.
   Players need a clear way down from the surface.
4. **Switch generation off per world:** "toggle Underdark generation completely off. There may
   be instances where I don't want to generate the underdark layer at all."
5. Feedback batch:
   - **Fix the shimmer:** "zooming in and out of the underdark has a shimmering effect on the
     rock/ground textures. Happens around L10–L14, it almost looks like static."
   - **Biomes:** "Add different biomes for the underdark."
   - **Settlements:** "Include cities and villages like the overworld but stylized for
     underdark setting."
   - **Branching:** "For the connecting passages, add more side caves and branching."
   - **Water:** "Add underdark lakes and rivers that can connect multiple named locations
     together."
6. Feedback batch:
   - **Surface generators:** "the villages and cities should mimic how we create villages and
     cities on the surface world. The visual assets should just be different."
   - **Passage widths:** "The cavern passages should vary in size, not stay consistent for long
     stretches."

## 3. Consolidated requirements for a future implementation

**World and generation**
- R1. A world-wide network of caverns and tunnels miles below the surface (about 4,000–11,000
  ft down), mostly under land. A seed decides it, together with the surface it lies under.
- R2. **Per-world switch** ("Generate the Underdark", on by default). It is saved in the world
  file and share links. Off means nothing is generated below the local sites, and no ways down
  exist.
- R3. **Access from the surface.** Every surface cave, mine and lava tube has a way down from
  its deepest level to a landing in the Underdark, and the landing has a way straight back.
  Double-click follows either link.
- R4. **Regional biomes**, chosen partly by the climate above: fungal forests, crystal grottos,
  ice caves, magma, spider warrens, stone forests, bone fields, drowned ruins and plain caverns.
  Each needs distinct floors, colours, props and hazards, and regions are named on the map.
- R5. **Passages:**
  - many side caves and branches (second ways out, loops, dead-end spurs, alcoves);
  - widths that swell and narrow every few hundred feet, never uniform for long, and never
    narrower than about 12 ft (narrower diagonal runs break apart on the 5-ft grid).
- R6. **Water:**
  - dark lakes;
  - named underground rivers that link lakes with settlements and other lakes;
  - floors graded downstream;
  - canals through the caverns they pass;
  - lava pools and channels in magma regions.
- R7. **Settlements:** cities and villages of Underdark peoples (drow, duergar, deep gnomes,
  myconids, kuo-toa, goblins, troglodytes).
  - **Laid out by the same town and village generators as the surface**: wards, walls and
    gates, streets, plazas, blocks and lots; or lanes with houses along them.
  - The cavern's rock and water act as the land's edges, and the tunnels as its roads.
  - Only the visual style differs per people.
  - Surface towns must stay byte-identical.

**Views and UI**
- R8. **A map view at every zoom** (continent to battlemap). The menu has a
  **View: Surface / Underdark** switch (and the `U` key), hidden when the world has no
  Underdark. The surface ghosts through at continent zoom.
- R9. **Battlemap tier:** walkable sections on the 5-ft grid with:
  - ledges at 5-ft tiers, water, lava, props and buildings that block movement;
  - hover info (elevation, terrain, hazards, names);
  - seamless joins between sections.
- R10. **Labels:** cities, villages, lakes, great caverns, regions and rivers, with each river
  labelled by what it connects.
- R11. **No shimmer:** textures must stay put while zooming (world-anchored pattern octaves
  crossfaded by zoom, noise finer than a pixel faded out, pattern origins wrapped on whole
  periods so tiles and sections don't seam).

**Quality gates (vital test)**
- R12. The main network is connected (≥ 80% of caverns), and every shaft reaches it.
- R13. Every tunnel is open along its middle at **every** zoom level.
- R14. Tile edges are identical between neighbours.
- R15. Every landing leads out of its section; blocking props never cut a passage.
- R16. Settlements have their buildings, at least 90% standing on open, dry floor.
- R17. Rivers carry water along their whole course, from lake to named place.
- R18. With the switch off: no network, labels, ways down or sections.
- R19. Native output equals WASM (`npm run det`). Performance stays above the floor.

## 4. Open items noted at the time

- Underdark buildings can't be entered (no interiors yet).
- Search doesn't find Underdark places.
- A click on the map in Underdark view shows the surface's info.
- A big city's first layout takes about 0.5 s per worker (cached after that).
- The asset batch for the Underdark (codex image generation) was never done; everything was
  drawn procedurally.

## 5. Notes from the tabled build (`underdark-wip`)

**Design**
- One field, `underdark::Net::sample(p, spacing)`, returns the signed distance to rock, the
  floor height, the zone and the liquid, band-limited by sample spacing. Tiles
  (`tile_rgba`, 257² RGBA: sdf, depth, zone|liquid, liquid alpha) and 640-ft sections
  (`d:<sx>:<sy>`, interiors with a 3-square margin) both sample it.
- The network is built once per worker (thread-local) in about 0.3–0.4 s for seed 1:
  - about 1,000 main caverns on a hex-jittered lattice;
  - about 7,000 side caverns and about 30,000 spurs;
  - a spanning tree of tunnels plus loops, meandered by midpoint displacement.
- Coarse levels:
  - widen narrow tunnels (0.45 × spacing) so they stay visible;
  - drop side caverns and passages at sample spacings above 700 ft;
  - sample tunnel polylines at strides of 1, 8 or 64, using chunked bounding-box indices.

  Tile cost was 4–70 ms.
- Settlements ran `town::generate_deep` with a `Deep` context (level floor, a rock/water probe,
  tunnel directions as roads). Buildings were drawn as `Level.structures`.

**Pitfalls**
- **Shimmer:** pattern scale was tied to the zoom, so textures re-rolled every frame.
- **Liquid encoding:** filtering 0/128/255 values made shores pass through the lava band.
  Encode none = 128, water = 255, lava = 0.
- **Blocking props:** diagonal blocking props sealed narrow tunnels. Only place blockers
  where every neighbouring square is open floor.
- **Wall-noise cap:** capping wall noise by the tunnel's nominal width rather than its local
  width closed narrow stretches.
- **Empty regions:** an empty spatial query must return "far from anything", not a small
  distance (it caused faint rectangles on the regional map).
- **Version bump on revert:** a revert must still bump `GEN_VERSION` (the app refuses
  newer-version files).

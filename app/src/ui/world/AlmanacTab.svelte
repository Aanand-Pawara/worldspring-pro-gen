  const facts = $derived.by<Fact[]>(() => {
    if (!overlay) return [];
    const out: Fact[] = [
      { text: `The generated world contains ${fmt(features.length)} named features across ${fmt(namedKinds)} feature types.`, tag: 'WORLD' },
      { text: `The map covers ${fmt(world.params.width_mi ?? 0)} × ${fmt(world.params.height_mi ?? 0)} miles.`, tag: 'WORLD' },
      { text: `Its named settlements account for an estimated ${fmt(totalPopulation)} people across ${fmt(settlements.length)} settlements.`, tag: 'PEOPLE' },
      { text: `${fmt(cities.length)} cities or metropolises, ${fmt(towns.length)} towns and ${fmt(villages.length)} villages make up the settlement network.`, tag: 'PEOPLE' },
      { text: `${fmt(capitals.length)} generated settlement${capitals.length === 1 ? '' : 's'} ${capitals.length === 1 ? 'is' : 'are'} marked as capital${capitals.length === 1 ? '' : 's'}.`, tag: 'CIVILIZATION' },
      { text: `Cities and metropolises contain about ${fmt(cityPopulationShare)}% of the generated population.`, tag: 'CIVILIZATION' },
      { text: capitals.length ? `Capital settlements contain about ${fmt(capitalPopulationShare)}% of the generated population.` : 'No generated settlement is marked as a capital.', tag: 'CIVILIZATION' },
      { text: `${fmt(volcanoes.length)} named volcanoes are present: ${fmt(activeVolcanoes.length)} active, ${fmt(dormantVolcanoes.length)} dormant and ${fmt(extinctVolcanoes.length)} extinct.`, tag: 'GEOLOGY' },
      { text: `${fmt(regions.length)} named biome regions were generated; ${leadingRegion ? `${leadingRegion[1]} are ${leadingRegion[0].replace('_', ' ')} regions, the most represented named region type.` : 'no large named biome region was extracted.'}`, tag: 'CLIMATE' },
    ];
    if (largestSettlement) out.push({ text: `${name(largestSettlement)} is the largest settlement, with an estimated population of ${fmt(population(largestSettlement))}.`, tag: 'PEOPLE', feature: largestSettlement });
    if (highestPeak) out.push({ text: `${name(highestPeak)} is the highest named summit at ${fmt(Math.round(highestPeak.elev_ft ?? 0))} ft above sea level.`, tag: 'TERRAIN', feature: highestPeak });
    if (longestRiver) out.push({ text: `${name(longestRiver)} is the longest named river at about ${fmtMiles(longestRiver.length_mi ?? miles(longestRiver.extent_ft))} miles.`, tag: 'WATER', feature: longestRiver });
    if (largestRiverBasin) out.push({ text: `${name(largestRiverBasin)} drains the largest modeled catchment, about ${fmtMiles(largestRiverBasin.drainage_area_mi2 ?? 0)} square miles, at stream order ${largestRiverBasin.stream_order ?? 1}.`, tag: 'HYDROLOGY', feature: largestRiverBasin });
    if (largestLake) out.push({ text: `${name(largestLake)} is the largest generated lake-type feature by stored extent, about ${fmtMiles(largestLake.area_mi2 ?? areaSqMi(largestLake.extent_ft))} square miles.`, tag: 'WATER', feature: largestLake });
    if (largestLandmass) out.push({ text: `${name(largestLandmass)} is the largest generated landmass by stored extent, about ${fmtMiles(areaSqMi(largestLandmass.extent_ft))} square miles.`, tag: 'GEOGRAPHY', feature: largestLandmass });
    if (highestWaterfall) out.push({ text: `${name(highestWaterfall)} has the greatest named waterfall drop at about ${fmt(drop(highestWaterfall))} ft.`, tag: 'WATER', feature: highestWaterfall });
    if (largestRange) out.push({ text: `${name(largestRange)} is the longest generated mountain-range feature by its stored extent.`, tag: 'TERRAIN', feature: largestRange });
    if (longestRiver && settlementsNearLongestRiver.length) out.push({ text: `${fmt(settlementsNearLongestRiver.length)} settlements have generated label anchors within 50 miles of ${name(longestRiver)}.`, tag: 'RELATION', feature: longestRiver });
    if (highestPeak && settlementsNearHighestPeak.length) out.push({ text: `${fmt(settlementsNearHighestPeak.length)} settlements have generated label anchors within 50 miles of ${name(highestPeak)}.`, tag: 'RELATION', feature: highestPeak });
    if (largestSettlement && nearbyPeak) out.push({ text: `${name(largestSettlement)} is about ${fmtMiles(nearbyPeak.d)} miles from the named peak ${name(nearbyPeak.f)}.`, tag: 'RELATION', feature: largestSettlement });
    if (largestSettlement && featuresNearLargestCity.length) out.push({ text: `${name(largestSettlement)} has ${fmt(featuresNearLargestCity.length)} other generated feature anchors within 25 miles.`, tag: 'RELATION', feature: largestSettlement });
    return out;
  });

  const current = $derived(facts.length ? facts[shown % facts.length] : null);
  function randomFact() {
    if (facts.length < 2) return;
    let next = Math.floor(Math.random() * facts.length);
    if (next === shown % facts.length) next = (next + 1) % facts.length;
    shown = next;
  }

  function selectFeature(f: Feature) {
    onSelect(f);
  }

  function selectMode(next: 'overview' | 'facts' | 'rankings') {
    mode = next;
  }

  function selectRanking(next: 'settlements' | 'terrain' | 'water' | 'regions') {
    ranking = next;
  }

  function handleModePointer(e: PointerEvent) {
    e.stopPropagation();
    const button = (e.target as HTMLElement).closest<HTMLButtonElement>('button[data-mode]');
    if (!button || button.disabled) return;
    const next = button.dataset.mode;
    if (next === 'overview' || next === 'facts' || next === 'rankings') selectMode(next);
  }

  function handleRankingPointer(e: PointerEvent) {
    e.stopPropagation();
    const button = (e.target as HTMLElement).closest<HTMLButtonElement>('button[data-ranking]');
    if (!button || button.disabled) return;
    const next = button.dataset.ranking;
    if (next === 'settlements' || next === 'terrain' || next === 'water' || next === 'regions') selectRanking(next);
  }

  const settlementRank = $derived([...settlements].sort((a, b) => population(b) - population(a) || a.name.localeCompare(b.name)));
  const peakRank = $derived([...peaks].sort((a, b) => (b.elev_ft ?? 0) - (a.elev_ft ?? 0)));
  const riverRank = $derived([...rivers].sort((a, b) => (b.length_mi ?? miles(b.extent_ft)) - (a.length_mi ?? miles(a.extent_ft))));
  const lakeRank = $derived([...lakes].sort((a, b) => b.extent_ft - a.extent_ft));
  const regionRank = $derived([...regionCounts]);

  $effect(() => {
    void overlay;
    shown = 0;
  });
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
        <button class="ws-btn primary" onclick={randomFact} disabled={facts.length < 2}><Icon name="dice" size={15} /> Random</button>
      {/if}
    </div>
  </header>

  <nav class="subtabs" aria-label="Almanac views" onpointerup={handleModePointer}>
    <button type="button" data-mode="overview" aria-pressed={mode === 'overview'} class:on={mode === 'overview'} onclick={() => selectMode('overview')}><Icon name="globe" size={14} /> Overview</button>
    <button type="button" data-mode="facts" aria-pressed={mode === 'facts'} class:on={mode === 'facts'} onclick={() => selectMode('facts')}><Icon name="dice" size={14} /> Facts</button>
    <button type="button" data-mode="rankings" aria-pressed={mode === 'rankings'} class:on={mode === 'rankings'} onclick={() => selectMode('rankings')}><Icon name="activity" size={14} /> Rankings</button>
  </nav>

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
  {:else if mode === 'facts'}
    {#if current}
      <article class="fact-card">
        <div class="tag">{current.tag}</div>
        <div class="fact">{current.text}</div>
        {#if current.feature}<button class="show" onclick={() => selectFeature(current.feature!)}><Icon name="pin" size={14} /> Show on map</button>{/if}
      </article>
      <div class="counter">Fact {((shown % facts.length) + 1).toLocaleString()} of {facts.length.toLocaleString()}</div>
    {:else}
      <div class="empty">No generated facts are available yet.</div>
    {/if}
    <div class="fact-note">Facts are calculated from generated data. Proximity facts use named feature anchors, so they never pretend we have a road network or historical record when we don't.</div>
  {:else}
    <nav class="rank-tabs" aria-label="Ranking categories" onpointerup={handleRankingPointer}>
      <button type="button" data-ranking="settlements" aria-pressed={ranking === 'settlements'} class:on={ranking === 'settlements'} onclick={() => selectRanking('settlements')}>Settlements</button>
      <button type="button" data-ranking="terrain" aria-pressed={ranking === 'terrain'} class:on={ranking === 'terrain'} onclick={() => selectRanking('terrain')}>Terrain</button>
      <button type="button" data-ranking="water" aria-pressed={ranking === 'water'} class:on={ranking === 'water'} onclick={() => selectRanking('water')}>Water</button>
      <button type="button" data-ranking="regions" aria-pressed={ranking === 'regions'} class:on={ranking === 'regions'} onclick={() => selectRanking('regions')}>Biomes</button>
    </nav>

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
  .subtabs,.rank-tabs { display:flex; gap:3px; overflow:auto; padding-bottom:1px; }
  .subtabs button,.rank-tabs button { display:inline-flex; align-items:center; gap:4px; border:1px solid var(--line); background:transparent; color:var(--ink-2); border-radius:var(--radius-sm); padding:6px 8px; white-space:nowrap; cursor:pointer; font:11px var(--font); }
  .subtabs button.on,.rank-tabs button.on { background:var(--accent); color:var(--accent-ink); border-color:var(--accent); }
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
  .fact-card { padding:14px; border:1px solid var(--line); border-radius:var(--radius); background:var(--btn-hover); }
  .tag { margin-bottom:8px; font-size:10px; letter-spacing:.08em; font-weight:bold; color:var(--ink-3); }
  .fact { font-size:16px; line-height:1.5; }
  .show { display:inline-flex; align-items:center; gap:5px; margin-top:12px; border:0; background:none; color:var(--ink-2); font:11px var(--font); cursor:pointer; padding:3px 0; }
  .show:hover { color:var(--ink); }
  .counter { color:var(--ink-3); font:10px var(--mono); text-align:center; }
  .fact-note { color:var(--ink-3); font-size:10px; line-height:1.4; padding:3px; }
  .row { all:unset; box-sizing:border-box; display:grid; grid-template-columns:22px 20px 1fr auto; align-items:center; gap:6px; width:100%; min-height:38px; padding:3px 4px; cursor:pointer; border-radius:var(--radius-sm); }
  .row:hover,.row:focus-visible { background:var(--btn-hover); }
  .rank { color:var(--ink-3); font:10px var(--mono); text-align:right; }
  .icon { color:var(--ink-2); display:flex; }
  .name { min-width:0; display:flex; flex-direction:column; overflow:hidden; text-align:left; font-size:11px; }
  .name small { color:var(--ink-3); font-size:10px; }
  .empty { display:flex; gap:8px; align-items:center; color:var(--ink-3); padding:12px 4px; font-size:12px; }
  .muted { color:var(--ink-3); font-size:11px; padding:4px; }
</style>
  const facts = $derived.by<Fact[]>(() => {
    if (!overlay) return [];
    const out: Fact[] = [
      { text: `The generated world contains ${fmt(features.length)} named features across ${fmt(namedKinds)} feature types.`, tag: 'WORLD' },
      { text: `The map covers ${fmt(world.params.width_mi ?? 0)} × ${fmt(world.params.height_mi ?? 0)} miles.`, tag: 'WORLD' },
      { text: `Its named settlements account for an estimated ${fmt(totalPopulation)} people across ${fmt(settlements.length)} settlements.`, tag: 'PEOPLE' },
      { text: `${fmt(cities.length)} cities or metropolises, ${fmt(towns.length)} towns and ${fmt(villages.length)} villages make up the settlement network.`, tag: 'PEOPLE' },
      { text: `${fmt(capitals.length)} generated settlement${capitals.length === 1 ? '' : 's'} ${capitals.length === 1 ? 'is' : 'are'} marked as capital${capitals.length === 1 ? '' : 's'}.`, tag: 'CIVILIZATION' },
      { text: `Cities and metropolises contain about ${fmt(cityPopulationShare)}% of the generated population.`, tag: 'CIVILIZATION' },
      { text: capitals.length ? `Capital settlements contain about ${fmt(capitalPopulationShare)}% of the generated population.` : 'No generated settlement is marked as a capital.', tag: 'CIVILIZATION' },
      { text: `${fmt(volcanoes.length)} named volcanoes are present: ${fmt(activeVolcanoes.length)} active, ${fmt(dormantVolcanoes.length)} dormant and ${fmt(extinctVolcanoes.length)} extinct.`, tag: 'GEOLOGY' },
      { text: `${fmt(regions.length)} named biome regions were generated; ${leadingRegion ? \`${leadingRegion[1]} are ${leadingRegion[0].replace('_', ' ')} regions, the most represented named region type.\` : 'no large named biome region was extracted.'}`, tag: 'CLIMATE' },
      { text: `${fmt(rivers.length)} named rivers carry the modeled drainage network, with ${fmt(totalRiverMiles)} total named river miles.`, tag: 'HYDROLOGY' },
      { text: `${fmt(lakes.length)} lake-type features were named; ${fmt(terminalLakes.length)} are modeled without an outlet and ${fmt(flowThroughLakes.length)} have an outlet.`, tag: 'HYDROLOGY' },
      { text: `${fmt(ranges.length)} mountain-range features and ${fmt(passes.length)} named passes shape the terrain.`, tag: 'TERRAIN' },
      { text: `${fmt(waterfalls.length)} named waterfalls were extracted from the modeled terrain and drainage.`, tag: 'WATER' },
      { text: `${fmt(landmasses.length)} named continents and islands form the generated landmass inventory.`, tag: 'GEOGRAPHY' },
      { text: `The highest modeled river stream order represented by a named river is ${fmt(highestOrderRiver?.stream_order ?? 0)}.`, tag: 'HYDROLOGY', feature: highestOrderRiver ?? undefined },
    ];

    // Every generated anchor contributes its own fact. This makes the Almanac a real gazetteer
    // rather than a tiny hand-picked list, while keeping every sentence traceable to generator data.
    for (const f of settlements) {
      const pop = population(f);
      const detail = f.detail ?? '';
      const rank = f.political_rank === 'capital' ? 'capital' : f.kind;
      const traits = [detail.includes('coastal') && 'coastal', detail.includes('river') && 'river-connected', f.kingdom_name && `part of ${f.kingdom_name}`].filter(Boolean).join(', ');
      out.push({ text: `${name(f)} is a ${rank} with an estimated population of ${fmt(pop)}${traits ? `, ${traits}` : ''}.`, tag: 'SETTLEMENT', feature: f });
    }
    for (const f of rivers) {
      const length = f.length_mi ?? miles(f.extent_ft);
      const mouth = f.river_mouth ? f.river_mouth.replace('_', ' ') : 'modeled';
      const lake = f.source_lake_id !== undefined ? ' It is lake-fed.' : '';
      out.push({ text: `${name(f)} runs for about ${fmtMiles(length)} miles, reaches stream order ${fmt(f.stream_order ?? 1)}, and has a ${mouth} terminal type.${lake}`, tag: 'RIVER', feature: f });
      if ((f.drainage_area_mi2 ?? 0) > 0) out.push({ text: `${name(f)} drains approximately ${fmtMiles(f.drainage_area_mi2 ?? 0)} square miles of modeled catchment.`, tag: 'BASIN', feature: f });
      if ((f.tributary_count ?? 0) > 0) out.push({ text: `${name(f)} has ${fmt(f.tributary_count ?? 0)} named or modeled tributary connections in its generated network.`, tag: 'RIVER', feature: f });
    }
    for (const f of lakes) {
      const area = f.area_mi2 ?? areaSqMi(f.extent_ft);
      out.push({ text: `${name(f)} covers about ${fmtMiles(area)} square miles in the generated hydrology model.`, tag: 'LAKE', feature: f });
      if (f.max_depth_ft !== undefined) out.push({ text: `${name(f)} has a modeled maximum depth of about ${fmt(Math.round(f.max_depth_ft))} ft.`, tag: 'LAKE', feature: f });
      if (f.inlet_count !== undefined) out.push({ text: `${name(f)} receives ${fmt(f.inlet_count)} modeled inlet connection${f.inlet_count === 1 ? '' : 's'}.`, tag: 'LAKE', feature: f });
      out.push({ text: `${name(f)} is modeled as a ${f.has_outlet === false ? 'terminal' : f.has_outlet === true ? 'flow-through' : 'lake-type'} water body.`, tag: 'HYDROLOGY', feature: f });
    }
    for (const f of peaks) {
      out.push({ text: `${name(f)} rises to approximately ${fmt(Math.round(f.elev_ft ?? 0))} ft above sea level.`, tag: 'PEAK', feature: f });
      if (f.kind === 'volcano') out.push({ text: `${name(f)} is a named volcano recorded as ${(f.detail ?? 'status unknown').split(' ')[0]} in the generated geology.`, tag: 'VOLCANO', feature: f });
    }
    for (const f of ranges) out.push({ text: `${name(f)} is a generated mountain-range feature spanning roughly ${fmtMiles(miles(f.extent_ft))} miles of stored extent.`, tag: 'RANGE', feature: f });
    for (const f of passes) out.push({ text: `${name(f)} is a named mountain pass generated as a lower-cost route through difficult terrain.`, tag: 'PASS', feature: f });
    for (const f of waterfalls) out.push({ text: `${name(f)} has an estimated generated drop of about ${fmt(drop(f))} ft.`, tag: 'WATERFALL', feature: f });
    for (const f of regions) out.push({ text: `${name(f)} is a named ${f.kind.replace('_', ' ')} region in the generated world.`, tag: 'REGION', feature: f });
    for (const f of landmasses) out.push({ text: `${name(f)} is a named ${f.kind} landmass with about ${fmtMiles(areaSqMi(f.extent_ft))} square miles of stored extent.`, tag: 'GEOGRAPHY', feature: f });

    // A second layer of facts gives the player useful comparisons without fabricating history.
    const topCities = [...settlements].sort((a, b) => population(b) - population(a) || a.name.localeCompare(b.name)).slice(0, 20);
    for (let i = 0; i < topCities.length; i++) out.push({ text: `${name(topCities[i])} ranks #${i + 1} among the generated settlements by estimated population.`, tag: 'RANKING', feature: topCities[i] });
    const topRivers = [...rivers].sort((a, b) => (b.length_mi ?? miles(b.extent_ft)) - (a.length_mi ?? miles(a.extent_ft))).slice(0, 20);
    for (let i = 0; i < topRivers.length; i++) out.push({ text: `${name(topRivers[i])} ranks #${i + 1} among named rivers by modeled length.`, tag: 'RANKING', feature: topRivers[i] });
    const topPeaks = [...peaks].sort((a, b) => (b.elev_ft ?? 0) - (a.elev_ft ?? 0)).slice(0, 20);
    for (let i = 0; i < topPeaks.length; i++) out.push({ text: `${name(topPeaks[i])} ranks #${i + 1} among named summits by elevation.`, tag: 'RANKING', feature: topPeaks[i] });
    const topLakes = [...lakes].sort((a, b) => (b.area_mi2 ?? areaSqMi(b.extent_ft)) - (a.area_mi2 ?? areaSqMi(a.extent_ft))).slice(0, 20);
    for (let i = 0; i < topLakes.length; i++) out.push({ text: `${name(topLakes[i])} ranks #${i + 1} among lake-type features by modeled area.`, tag: 'RANKING', feature: topLakes[i] });

    if (longestRiver && settlementsNearLongestRiver.length) out.push({ text: `${fmt(settlementsNearLongestRiver.length)} settlements have generated label anchors within 50 miles of ${name(longestRiver)}.`, tag: 'RELATION', feature: longestRiver });
    if (highestPeak && settlementsNearHighestPeak.length) out.push({ text: `${fmt(settlementsNearHighestPeak.length)} settlements have generated label anchors within 50 miles of ${name(highestPeak)}.`, tag: 'RELATION', feature: highestPeak });
    if (largestSettlement && nearbyPeak) out.push({ text: `${name(largestSettlement)} is about ${fmtMiles(nearbyPeak.d)} miles from the named peak ${name(nearbyPeak.f)}.`, tag: 'RELATION', feature: largestSettlement });
    if (largestSettlement && featuresNearLargestCity.length) out.push({ text: `${name(largestSettlement)} has ${fmt(featuresNearLargestCity.length)} other generated feature anchors within 25 miles.`, tag: 'RELATION', feature: largestSettlement });
    return out;
  });

  const current = $derived(facts.length ? facts[shown % facts.length] : null);
  function randomFact() {
    if (facts.length < 2) return;
    let next = Math.floor(Math.random() * facts.length);
    if (next === shown % facts.length) next = (next + 1) % facts.length;
    shown = next;
  }

  function selectFeature(f: Feature) {
    onSelect(f);
  }

  function selectMode(next: 'overview' | 'facts' | 'rankings') {
    mode = next;
  }

  function selectRanking(next: 'settlements' | 'terrain' | 'water' | 'regions') {
    ranking = next;
  }

  function handleModePointer(e: PointerEvent) {
    e.stopPropagation();
    const button = (e.target as HTMLElement).closest<HTMLButtonElement>('button[data-mode]');
    if (!button || button.disabled) return;
    const next = button.dataset.mode;
    if (next === 'overview' || next === 'facts' || next === 'rankings') selectMode(next);
  }

  function handleRankingPointer(e: PointerEvent) {
    e.stopPropagation();
    const button = (e.target as HTMLElement).closest<HTMLButtonElement>('button[data-ranking]');
    if (!button || button.disabled) return;
    const next = button.dataset.ranking;
    if (next === 'settlements' || next === 'terrain' || next === 'water' || next === 'regions') selectRanking(next);
  }

  const settlementRank = $derived([...settlements].sort((a, b) => population(b) - population(a) || a.name.localeCompare(b.name)));
  const peakRank = $derived([...peaks].sort((a, b) => (b.elev_ft ?? 0) - (a.elev_ft ?? 0)));
  const riverRank = $derived([...rivers].sort((a, b) => (b.length_mi ?? miles(b.extent_ft)) - (a.length_mi ?? miles(a.extent_ft))));
  const lakeRank = $derived([...lakes].sort((a, b) => b.extent_ft - a.extent_ft));
  const regionRank = $derived([...regionCounts]);

  $effect(() => {
    void overlay;
    shown = 0;
  });
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
        <button class="ws-btn primary" onclick={randomFact} disabled={facts.length < 2}><Icon name="dice" size={15} /> Random</button>
      {/if}
    </div>
  </header>

  <nav class="subtabs" aria-label="Almanac views" onpointerup={handleModePointer}>
    <button type="button" data-mode="overview" aria-pressed={mode === 'overview'} class:on={mode === 'overview'} onclick={() => selectMode('overview')}><Icon name="globe" size={14} /> Overview</button>
    <button type="button" data-mode="facts" aria-pressed={mode === 'facts'} class:on={mode === 'facts'} onclick={() => selectMode('facts')}><Icon name="dice" size={14} /> Facts</button>
    <button type="button" data-mode="rankings" aria-pressed={mode === 'rankings'} class:on={mode === 'rankings'} onclick={() => selectMode('rankings')}><Icon name="activity" size={14} /> Rankings</button>
  </nav>

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
  {:else if mode === 'facts'}
    {#if current}
      <article class="fact-card">
        <div class="tag">{current.tag}</div>
        <div class="fact">{current.text}</div>
        {#if current.feature}<button class="show" onclick={() => selectFeature(current.feature!)}><Icon name="pin" size={14} /> Show on map</button>{/if}
      </article>
      <div class="counter">Fact {((shown % facts.length) + 1).toLocaleString()} of {facts.length.toLocaleString()}</div>
    {:else}
      <div class="empty">No generated facts are available yet.</div>
    {/if}
    <div class="fact-note">Facts are calculated from generated data. Proximity facts use named feature anchors, so they never pretend we have a road network or historical record when we don't.</div>
  {:else}
    <nav class="rank-tabs" aria-label="Ranking categories" onpointerup={handleRankingPointer}>
      <button type="button" data-ranking="settlements" aria-pressed={ranking === 'settlements'} class:on={ranking === 'settlements'} onclick={() => selectRanking('settlements')}>Settlements</button>
      <button type="button" data-ranking="terrain" aria-pressed={ranking === 'terrain'} class:on={ranking === 'terrain'} onclick={() => selectRanking('terrain')}>Terrain</button>
      <button type="button" data-ranking="water" aria-pressed={ranking === 'water'} class:on={ranking === 'water'} onclick={() => selectRanking('water')}>Water</button>
      <button type="button" data-ranking="regions" aria-pressed={ranking === 'regions'} class:on={ranking === 'regions'} onclick={() => selectRanking('regions')}>Biomes</button>
    </nav>

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
  .subtabs,.rank-tabs { display:flex; gap:3px; overflow:auto; padding-bottom:1px; }
  .subtabs button,.rank-tabs button { display:inline-flex; align-items:center; gap:4px; border:1px solid var(--line); background:transparent; color:var(--ink-2); border-radius:var(--radius-sm); padding:6px 8px; white-space:nowrap; cursor:pointer; font:11px var(--font); }
  .subtabs button.on,.rank-tabs button.on { background:var(--accent); color:var(--accent-ink); border-color:var(--accent); }
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
  .fact-card { padding:14px; border:1px solid var(--line); border-radius:var(--radius); background:var(--btn-hover); }
  .tag { margin-bottom:8px; font-size:10px; letter-spacing:.08em; font-weight:bold; color:var(--ink-3); }
  .fact { font-size:16px; line-height:1.5; }
  .show { display:inline-flex; align-items:center; gap:5px; margin-top:12px; border:0; background:none; color:var(--ink-2); font:11px var(--font); cursor:pointer; padding:3px 0; }
  .show:hover { color:var(--ink); }
  .counter { color:var(--ink-3); font:10px var(--mono); text-align:center; }
  .fact-note { color:var(--ink-3); font-size:10px; line-height:1.4; padding:3px; }
  .row { all:unset; box-sizing:border-box; display:grid; grid-template-columns:22px 20px 1fr auto; align-items:center; gap:6px; width:100%; min-height:38px; padding:3px 4px; cursor:pointer; border-radius:var(--radius-sm); }
  .row:hover,.row:focus-visible { background:var(--btn-hover); }
  .rank { color:var(--ink-3); font:10px var(--mono); text-align:right; }
  .icon { color:var(--ink-2); display:flex; }
  .name { min-width:0; display:flex; flex-direction:column; overflow:hidden; text-align:left; font-size:11px; }
  .name small { color:var(--ink-3); font-size:10px; }
  .empty { display:flex; gap:8px; align-items:center; color:var(--ink-3); padding:12px 4px; font-size:12px; }
  .muted { color:var(--ink-3); font-size:11px; padding:4px; }
</style>

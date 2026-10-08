<script lang="ts">
  import Icon from '../Icon.svelte';
  import type { ClimateDebugField } from '../../gen/protocol';
  import type { GenClient } from '../../gen/client';

  interface Props { gen: GenClient; ready: boolean; }
  let { gen, ready }: Props = $props();

  type Field = { key: ClimateDebugField; label: string; unit: string; desc: string };
  const FIELDS: Field[] = [
    { key: 'temperature', label: 'Temperature', unit: '°C', desc: 'Monthly mean surface temperature' },
    { key: 'precipitation', label: 'Precipitation', unit: 'mm', desc: 'Monthly precipitation' },
    { key: 'humidity', label: 'Humidity', unit: '%', desc: 'Atmospheric moisture' },
    { key: 'evaporation', label: 'Evaporation', unit: 'mm', desc: 'Surface evaporation' },
    { key: 'soil_moisture', label: 'Soil moisture', unit: '%', desc: 'Stored surface water' },
    { key: 'snow', label: 'Snowpack', unit: 'mm', desc: 'Persistent snow water equivalent' },
    { key: 'continentality', label: 'Continentality', unit: '', desc: 'Maritime → continental influence' },
    { key: 'orographic', label: 'Orographic rain', unit: '', desc: 'Mountain lift contribution' },
    { key: 'wind', label: 'Wind speed', unit: '', desc: 'Circulation magnitude' },
    { key: 'climate_class', label: 'Climate class', unit: '', desc: 'Simulated climate classification' },
  ];
  const MONTHS = ['January','February','March','April','May','June','July','August','September','October','November','December'];
  let field = $state<ClimateDebugField>('temperature');
  let month = $state(12);
  let data = $state<{w:number;h:number;min:number;max:number;values:number[]}|null>(null);
  let loading = $state(false);
  let error = $state<string|null>(null);
  let canvas = $state<HTMLCanvasElement|null>(null);

  const current = $derived(FIELDS.find((f) => f.key === field)!);
  const title = $derived(month === 12 ? 'Annual' : MONTHS[month]);

  async function load() {
    if (!ready) return;
    loading = true; error = null;
    try {
      const d = await gen.climateDebug(field, month);
      data = d;
      requestAnimationFrame(draw);
    } catch (e) {
      error = e instanceof Error ? e.message : String(e);
    } finally { loading = false; }
  }

  function draw() {
    if (!canvas || !data) return;
    const rect = canvas.getBoundingClientRect();
    const scale = Math.min(1.5, devicePixelRatio || 1);
    canvas.width = Math.max(1, Math.round(rect.width * scale));
    canvas.height = Math.max(1, Math.round(rect.height * scale));
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    ctx.imageSmoothingEnabled = false;
    const maxCells = 180;
    const sx = Math.max(1, Math.ceil(data.w / maxCells));
    const sy = Math.max(1, Math.ceil(data.h / maxCells));
    const outW = Math.ceil(data.w / sx), outH = Math.ceil(data.h / sy);
    const img = ctx.createImageData(outW, outH);
    const span = Math.max(1e-9, data.max - data.min);
    for (let y=0; y<outH; y++) for (let x=0; x<outW; x++) {
      let sum=0,count=0;
      const y0=y*sy, x0=x*sx;
      for(let yy=y0; yy<Math.min(data.h,y0+sy); yy++) for(let xx=x0; xx<Math.min(data.w,x0+sx); xx++){const v=data.values[yy*data.w+xx]; if(Number.isFinite(v)){sum+=v;count++;}}
      const t=count ? (sum/count-data.min)/span : 0;
      const q=Math.max(0,Math.min(1,t));
      const r=Math.round(255*Math.min(1,Math.max(0,2*q)));
      const g=Math.round(255*(1-Math.abs(q-.5)*2));
      const b=Math.round(255*(1-q));
      const o=(y*outW+x)*4; img.data[o]=r; img.data[o+1]=g; img.data[o+2]=b; img.data[o+3]=255;
    }
    const tmp = document.createElement('canvas'); tmp.width = outW; tmp.height = outH;
    tmp.getContext('2d')!.putImageData(img, 0, 0);
    ctx.clearRect(0, 0, canvas.width, canvas.height);
    ctx.imageSmoothingEnabled = false;
    ctx.drawImage(tmp, 0, 0, canvas.width, canvas.height);
  }

  $effect(() => { void field; void month; if (ready) void load(); });
</script>

<div class="climate">
  <header>
    <div>
      <div class="eyebrow">CLIMATE LAB</div>
      <h2>Climate simulation</h2>
      <p>Inspect the physical fields driving precipitation, hydrology and biomes.</p>
    </div>
    <button class="ws-btn" onclick={load} disabled={!ready || loading}><Icon name="reset" size={14}/>{loading ? 'Sampling…' : 'Refresh'}</button>
  </header>

  <div class="field-grid">
    {#each FIELDS as f (f.key)}
      <button class:on={field===f.key} onclick={() => field=f.key}><span>{f.label}</span><small>{f.unit}</small></button>
    {/each}
  </div>

  <div class="months">
    <button class:on={month===12} onclick={() => month=12}>Annual</button>
    {#each MONTHS as m,i}<button class:on={month===i} onclick={() => month=i}>{m.slice(0,3)}</button>{/each}
  </div>

  <section class="preview">
    <div class="preview-head"><div><b>{current.label}</b><small>{current.desc} · {title}</small></div>{#if data}<code>{data.min.toFixed(2)} → {data.max.toFixed(2)} {current.unit}</code>{/if}</div>
    {#if !ready}<div class="empty">Generate a world first.</div>
    {:else if error}<div class="empty error">{error}</div>
    {:else if loading}<div class="empty">Sampling climate field…</div>
    {:else}<canvas bind:this={canvas} aria-label="{current.label} climate map"></canvas>{/if}
  </section>

  <div class="hint"><Icon name="activity" size={14}/><span>Fields are sampled from the coarse deterministic climate grid. The viewer downsamples only for display, so the UI stays cheap even on large worlds.</span></div>
</div>

<style>
  .climate{display:flex;flex-direction:column;gap:9px}.climate header{display:flex;gap:8px;align-items:flex-start}.climate header>div{flex:1;min-width:0}.eyebrow{font:10px var(--mono);letter-spacing:.12em;color:var(--gold)}h2{margin:2px 0;font-size:18px}.climate p{margin:0;color:var(--ink-3);font-size:11px;line-height:1.35}
  .field-grid{display:grid;grid-template-columns:1fr 1fr;gap:3px}.field-grid button,.months button{border:1px solid var(--line);background:transparent;color:var(--ink-2);border-radius:var(--radius-sm);font:11px var(--font);padding:6px;cursor:pointer}.field-grid button{display:flex;justify-content:space-between;gap:4px}.field-grid button.on,.months button.on{background:var(--accent);color:var(--accent-ink);border-color:var(--accent)}.field-grid small{opacity:.7}
  .months{display:grid;grid-template-columns:repeat(7,1fr);gap:3px}.months button{padding:5px 2px}.preview{border:1px solid var(--line);border-radius:var(--radius-sm);overflow:hidden;background:var(--ink)}.preview-head{background:var(--paper-solid);padding:7px 8px;display:flex;justify-content:space-between;gap:8px}.preview-head div{display:flex;flex-direction:column}.preview-head small{font-size:10px;color:var(--ink-3)}code{font:10px var(--mono);white-space:nowrap}canvas{display:block;width:100%;height:210px;image-rendering:pixelated}.empty{padding:35px 10px;text-align:center;color:var(--ink-3);font-size:11px}.error{color:#9b3b30}.hint{display:flex;gap:6px;padding:7px 8px;background:var(--btn-hover);border:1px solid var(--line-faint);border-radius:var(--radius-sm);font-size:10px;color:var(--ink-3);line-height:1.35}
</style>

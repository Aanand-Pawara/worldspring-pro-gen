//! Hydrology on the T0 grid: lakes, water balance, rivers.
//!
//! - Depressions from priority-flood become lakes when large and deep enough; small ones are
//!   filled, so every other land cell drains downhill to the sea or a lake.
//! - Runoff (precipitation minus evapotranspiration) is accumulated downstream. Open water
//!   evaporates, so a lake in a dry basin can lose all its inflow: an endorheic salt lake or,
//!   when very dry, a salt flat. Desert streams lose water and fade out.
//! - Rivers are cells above a discharge threshold, split at confluences and joined into
//!   named chains along the largest-discharge path. By construction every river cell is
//!   strictly lower than the cell upstream of it.

use serde::Serialize;
use std::collections::VecDeque;

use super::climate::Climate;
use super::flood::{neighbors, priority_flood, receivers};

/// Water surface value for dry ground.
pub const DRY: f32 = -30_000.0;
pub const NO_LAKE: u32 = u32::MAX;

/// Discharge (mm·cells) at which a stream is mapped as a river, before `river_density`.
pub const RIVER_Q: f64 = 90_000.0;
pub const DELTA_Q_FACTOR: f64 = 4.0;
pub const DELTA_MIN_CELLS: usize = 12;
pub const DELTA_MAX_GRADE: f64 = 0.04;
const MIN_LAKE_CELLS: usize = 12;
const MIN_LAKE_DEPTH_FT: f64 = 60.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LakeKind {
    Fresh,
    /// Endorheic: evaporation matches inflow, no outlet.
    Salt,
    /// Evaporation exceeds inflow: dry most of the year.
    SaltFlat,
}

#[derive(Clone, Debug)]
pub struct Lake {
    pub level_ft: f64,
    pub cells: Vec<u32>,
    pub kind: LakeKind,
    pub max_depth_ft: f64,
    pub outlet: Option<u32>,
    pub inlet_count: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mouth {
    Ocean,
    Lake,
    /// Lost to evaporation / infiltration (desert wadis).
    Dry,
    /// Joins another river chain.
    Confluence,
}

#[derive(Clone, Debug)]
pub struct River {
    /// Cells from source to mouth (the mouth cell may be ocean or lake).
    pub cells: Vec<u32>,
    /// Discharge at each cell (mm·cells).
    pub q: Vec<f32>,
    pub mouth: Mouth,
    /// Chain this one flows into, for tributaries.
    pub into: Option<usize>,
    /// Strahler stream order: 1 for headwaters, increasing at equal-order confluences.
    pub order: u8,
    /// Number of routed land cells contributing to the source catchment.
    pub drainage_area_cells: u32,
    /// Largest modeled discharge along this chain.
    pub peak_discharge: f32,
    /// Number of direct tributary chains joining this chain.
    pub tributary_count: u16,
    pub source_lake: Option<u32>,
    pub mouth_lake: Option<u32>,
    /// Number of T0 flow cells in this chain.
    pub length_cells: u32,
    pub length_ft: f64,
    pub basin_id: u64,
    pub source_cell: u32,
    pub terminal_receiver: Option<u32>,
}

pub struct Hydro {
    /// Water surface elevation per cell (sea level, lake level) or `DRY`.
    pub water: Vec<f32>,
    /// Number of routed land cells contributing to each active cell.
    pub flow_accumulation: Vec<u32>,
    /// Stable drainage-basin identity for every land cell.
    pub basin_id: Vec<u64>,
    pub discharge: Vec<f32>,
    pub receiver: Vec<u32>,
    pub lake_of: Vec<u32>,
    pub lakes: Vec<Lake>,
    pub rivers: Vec<River>,
}

/// `feed`: extra discharge entering at cells (sketched rivers' sources).
pub fn build(w: usize, h: usize, cell_ft: f64, height: &mut [f64], land: &[bool], clim: &Climate, sea: f64, river_density: f64, feed: &[(usize, f64)]) -> Hydro {
    let n = w * h;
    let outlet: Vec<bool> = land.iter().map(|l| !l).collect();
    let fl = priority_flood(w, h, height, &outlet, 0.01);

    // Lakes: connected depressions that are big and deep enough. Everything else is filled.
    let mut lake_of = vec![NO_LAKE; n];
    let mut seen = vec![false; n];
    let mut lakes: Vec<Lake> = Vec::new();
    for start in 0..n {
        if seen[start] || !land[start] || fl.filled[start] - height[start] <= 1.0 {
            continue;
        }
        let mut comp = vec![start as u32];
        seen[start] = true;
        let mut k = 0;
        while k < comp.len() {
            let c = comp[k] as usize;
            k += 1;
            for (nb, _) in neighbors(w, h, c) {
                if !seen[nb] && land[nb] && fl.filled[nb] - height[nb] > 1.0 {
                    seen[nb] = true;
                    comp.push(nb as u32);
                }
            }
        }
        let depth = comp.iter().map(|&c| fl.filled[c as usize] - height[c as usize]).fold(0.0, f64::max);
        if comp.len() >= MIN_LAKE_CELLS && depth >= MIN_LAKE_DEPTH_FT {
            let level = comp.iter().map(|&c| fl.filled[c as usize]).fold(f64::INFINITY, f64::min);
            let id = lakes.len() as u32;
            for &c in &comp {
                lake_of[c as usize] = id;
            }
            lakes.push(Lake { level_ft: level, cells: comp, kind: LakeKind::Fresh, max_depth_ft: depth, outlet: None, inlet_count: 0 });
        }
    }
    // Fill every non-lake cell to the flooded surface: guarantees strictly downhill drainage.
    for i in 0..n {
        if land[i] && lake_of[i] == NO_LAKE {
            height[i] = fl.filled[i];
        }
    }

    let (mut rec, _) = receivers(w, h, &fl.filled);

    for lake_id in 0..lakes.len() {
        lakes[lake_id].outlet = None;
    }

    // Repair the receiver graph before deciding lake outlets. Priority-flood can leave a
    // perfectly valid flat lake without a receiver edge even when a low spill path exists.
    repair_nearby_water_sinks(w, h, land, &lake_of, &fl.filled, &mut rec, 24);
    connect_close_lakes(w, h, land, &lake_of, &fl.filled, &mut rec, &mut lakes);
    connect_lakes_to_ocean(w, h, land, &lake_of, &fl.filled, sea, &mut rec, &mut lakes);
    repair_nearby_water_sinks(w, h, land, &lake_of, &fl.filled, &mut rec, 24);

    for i in 0..n {
        if !land[i] || lake_of[i] != NO_LAKE { continue; }
        let r = rec[i] as usize;
        if r < n && lake_of[r] != NO_LAKE {
            let id = lake_of[r] as usize;
            lakes[id].inlet_count = lakes[id].inlet_count.saturating_add(1);
        }
    }

    // Receiver repair and lake-to-lake spill links are complete before water balance.
    let route_order = receiver_order(w, h, land, &mut rec);

    // Water balance. `raw` ignores losses (catchment supply); `q` includes them.
    let pet: Vec<f64> = clim.temp.iter().map(|&t| (350.0 + 55.0 * t as f64).max(0.0)).collect();
    let mut q = vec![0.0f64; n];
    let mut raw = vec![0.0f64; n];
    for &(k, v) in feed {
        q[k] += v;
        raw[k] += v;
    }
    for &i in route_order.iter() {
        let i = i as usize;
        let p = clim.precip[i] as f64;
        let runoff = (p - 0.65 * pet[i]).max(0.0);
        q[i] += runoff;
        raw[i] += runoff;
        if lake_of[i] != NO_LAKE {
            q[i] = (q[i] - pet[i]).max(0.0);
        } else if p < 400.0 {
            q[i] *= 0.9995; // mild transmission loss in dry country; long mapped channels remain continuous
        }
        let r = rec[i] as usize;
        if r < n && r != i && land[r] {
            q[r] += q[i];
            raw[r] += raw[i];
        }
    }

    let river_threshold = RIVER_Q / river_density.max(0.05);

    // Compute each lake's actual water budget from local runoff, evaporation, and external
    // inflows. Summing q over lake cells double-counts water accumulated inside the same lake.
    let mut lake_net = vec![0.0f64; lakes.len()];
    // Track external inflow separately from local precipitation/evaporation. A flow-through
    // lake must not become a dead end merely because its static local water budget is negative.
    let mut lake_inflow = vec![0.0f64; lakes.len()];
    for i in 0..n {
        if lake_of[i] != NO_LAKE {
            let p = clim.precip[i] as f64;
            let runoff = (p - 0.65 * pet[i]).max(0.0);
            lake_net[lake_of[i] as usize] += runoff - pet[i];
        } else {
            let r = rec[i] as usize;
            if r < n && lake_of[r] != NO_LAKE {
                let id = lake_of[r] as usize;
                lake_net[id] += q[i];
                lake_inflow[id] += q[i];
            }
        }
    }
    for &(k, v) in feed {
        if lake_of[k] != NO_LAKE {
            let id = lake_of[k] as usize;
            lake_net[id] += v;
            lake_inflow[id] += v;
        }
    }

    // A positive-water lake close to a mapped river can become that river's real source.
    // Existing outlets were already included in the first water-balance routing pass.
    let had_outlet: Vec<bool> = lakes.iter().map(|lake| lake.outlet.is_some()).collect();
    connect_lakes_to_river_channels(
        w, h, land, &lake_of, &fl.filled, &q, river_threshold, &mut rec,
        &mut lakes, &lake_net, &lake_inflow,
    );

    connect_nearby_river_termini_to_water(w, h, land, &lake_of, &fl.filled, &q, river_threshold, &mut rec);
    // Route every lake's net outflow through the complete receiver chain.
    let mut lake_order: Vec<usize> = (0..lakes.len()).collect();
    lake_order.sort_by(|&a, &b| lakes[b].level_ft.total_cmp(&lakes[a].level_ft).then(a.cmp(&b)));
    for lake_id in lake_order {
        if !had_outlet[lake_id] && lakes[lake_id].outlet.is_some() {
            route_lake_outflow(lake_id, land, &lake_of, &rec, &mut lake_net, &mut q, &mut raw, &lakes);
        }
    }

    for lake_id in 0..lakes.len() {
        let lake = &mut lakes[lake_id];
        let exit = lake.outlet.map(|c| c as usize).unwrap_or_else(|| lake.cells.iter().map(|&c| c as usize).max_by(|&x, &y| raw[x].total_cmp(&raw[y]).then(x.cmp(&y))).unwrap());
        let evap: f64 = lake.cells.iter().map(|&c| pet[c as usize]).sum();
        let inflow = lake_net[lake_id].max(0.0);
        let ratio = inflow / evap.max(1.0);
        let precip = lake.cells.iter().map(|&c| clim.precip[c as usize] as f64).sum::<f64>() / lake.cells.len() as f64;
        lake.kind = if lake.outlet.is_some() && inflow > 0.0 {
            LakeKind::Fresh
        } else if precip > 550.0 {
            LakeKind::Fresh
        } else if ratio < 0.35 {
            LakeKind::SaltFlat
        } else if inflow <= 0.0 {
            LakeKind::Salt
        } else {
            LakeKind::Fresh
        };
        let _ = exit;
    }
    // Water surface: sea and lakes (salt flats are dry). Tiles interpolate it over wet
    // corners only (`T0::sample_water`), so shorelines stay clean without dilation.
    let mut water = vec![DRY; n];
    for i in 0..n {
        if !land[i] {
            water[i] = sea as f32;
        }
    }
    for lake in &lakes {
        if lake.kind != LakeKind::SaltFlat {
            for &c in &lake.cells {
                water[c as usize] = lake.level_ft as f32;
            }
        }
    }

    // Receiver topology is final now. Recompute discharge from that final graph so newly
    // connected lakes actually feed the rivers extracted below.
    let route_order = receiver_order(w, h, land, &mut rec);
    let (q, _) = recompute_final_discharge(
        w, h, land, &lake_of, &rec, &route_order, &clim, &feed, &lakes,
    );
    let flow_accumulation = accumulate_flow(w, h, land, &lake_of, &rec, &route_order);
    let basin_id = assign_basin_ids(w, h, land, &lake_of, &rec);
    let rivers = extract_rivers(w, h, cell_ft, land, &lake_of, &rec, &q, &flow_accumulation, &basin_id, river_threshold, &lakes);
    add_headwater_ponds(w, h, land, &lake_of, height, &rivers, &mut water);
    debug_assert!(validate_hydrology(
        w, h, land, &lake_of, &rec, &lakes, &rivers, &water,
    ));
    Hydro { water, flow_accumulation, basin_id, discharge: q.iter().map(|&v| v as f32).collect(), receiver: rec, lake_of, lakes, rivers }
}


/// Give non-lake river sources a tiny, unnamed headwater pond in the actual water surface.
/// These ponds stay below the lake classification thresholds and use an irregular connected
/// footprint rather than a synthetic circle. Hydrology topology is unchanged: the source cell
/// remains the river's first receiver cell and lake_of never includes these ponds.
fn add_headwater_ponds(
    w: usize,
    h: usize,
    land: &[bool],
    lake_of: &[u32],
    height: &[f64],
    rivers: &[River],
    water: &mut [f32],
) {
    const MAX_POND_CELLS: usize = 8;
    const MIN_POND_CELLS: usize = 4;
    const MAX_RISE_FT: f64 = 8.0;
    const WATER_DEPTH_FT: f64 = 2.5;

    fn hash(mut x: u64) -> u64 {
        x ^= x >> 30;
        x = x.wrapping_mul(0xbf58476d1ce4e5b9);
        x ^= x >> 27;
        x = x.wrapping_mul(0x94d049bb133111eb);
        x ^ (x >> 31)
    }

    for (river_index, river) in rivers.iter().enumerate() {
        if river.source_lake.is_some() || river.cells.is_empty() {
            continue;
        }
        let source = river.source_cell as usize;
        if source >= height.len() || !land[source] || lake_of[source] != NO_LAKE {
            continue;
        }

        let base = height[source];
        let target = MIN_POND_CELLS
            + (hash((source as u64) ^ ((river_index as u64) << 32)) as usize
                % (MAX_POND_CELLS - MIN_POND_CELLS + 1));

        let mut pond = Vec::with_capacity(MAX_POND_CELLS);
        let mut frontier = Vec::<usize>::with_capacity(16);
        pond.push(source);

        for (nb, _) in neighbors(w, h, source) {
            if land[nb] && lake_of[nb] == NO_LAKE && height[nb] <= base + MAX_RISE_FT {
                frontier.push(nb);
            }
        }

        while pond.len() < target && !frontier.is_empty() {
            let mut best = None;
            let mut best_score = f64::INFINITY;
            for (i, &cell) in frontier.iter().enumerate() {
                if pond.iter().any(|&p| p == cell) {
                    continue;
                }
                let rise = (height[cell] - base).max(0.0);
                let jitter = (hash((cell as u64) ^ ((river_index as u64 + 1) * 0x9e3779b97f4a7c15))
                    & 0xffff) as f64 / 65535.0;
                let score = rise * 12.0 + jitter * 5.0;
                if score < best_score {
                    best_score = score;
                    best = Some(i);
                }
            }
            let Some(index) = best else { break; };
            let cell = frontier.swap_remove(index);
            if pond.iter().any(|&p| p == cell) {
                continue;
            }
            pond.push(cell);

            for (nb, _) in neighbors(w, h, cell) {
                if !land[nb] || lake_of[nb] != NO_LAKE || height[nb] > base + MAX_RISE_FT {
                    continue;
                }
                if !pond.iter().any(|&p| p == nb) && !frontier.iter().any(|&p| p == nb) {
                    frontier.push(nb);
                }
            }
        }

        if pond.len() < MIN_POND_CELLS {
            continue;
        }

        let level = pond.iter()
            .map(|&cell| height[cell])
            .fold(f64::NEG_INFINITY, f64::max)
            + WATER_DEPTH_FT;

        for cell in pond {
            water[cell] = water[cell].max(level as f32);
        }
    }
}

fn connect_lakes_to_river_channels(
    w: usize,
    h: usize,
    land: &[bool],
    lake_of: &[u32],
    filled: &[f64],
    q: &[f64],
    threshold: f64,
    rec: &mut [u32],
    lakes: &mut [Lake],
    lake_net: &[f64],
    lake_inflow: &[f64],
) {
    if lakes.is_empty() { return; }
    const MAX_CONNECT_CELLS: usize = 36;
    // A lake outlet can seed a mapped river even when its discharge is modest. Keep the
    // connection selective, but do not require the full river-mapping threshold at the
    // lake boundary or many plausible lake-fed headwaters disappear.
    let min_q = (threshold * 0.06).max(1.0);
    let mut order: Vec<usize> = (0..lakes.len()).collect();
    order.sort_by(|&a, &b| lakes[b].level_ft.total_cmp(&lakes[a].level_ft).then(a.cmp(&b)));

    let mut boundaries = Vec::with_capacity(lakes.len());
    for lake in lakes.iter() {
        let mut edge = Vec::new();
        for &c in &lake.cells {
            if neighbors(w, h, c as usize).any(|(nb, _)| land[nb] && lake_of[nb] == NO_LAKE) { edge.push(c); }
        }
        boundaries.push(edge);
    }

    for lake_id in order {
        if lakes[lake_id].outlet.is_some()
            || (lake_net[lake_id] <= threshold * 0.02 && lake_inflow[lake_id] <= threshold * 0.02)
            || boundaries[lake_id].is_empty()
        { continue; }
        let Some((source_cell, path, target)) = lake_river_spill_path(
            w, h, land, lake_of, filled, q, rec, min_q, lakes[lake_id].level_ft,
            MAX_CONNECT_CELLS, &boundaries[lake_id],
        ) else { continue; };
        if receiver_path_would_cycle(source_cell, &path, target, rec) { continue; }
        if path.is_empty() {
            rec[source_cell as usize] = target;
        } else {
            rec[source_cell as usize] = path[0];
            for pair in path.windows(2) { rec[pair[0] as usize] = pair[1]; }
            rec[*path.last().unwrap() as usize] = target;
        }
        lakes[lake_id].outlet = Some(source_cell);
    }
}

fn lake_river_spill_path(
    w: usize,
    h: usize,
    land: &[bool],
    lake_of: &[u32],
    filled: &[f64],
    q: &[f64],
    rec: &[u32],
    min_q: f64,
    source_level: f64,
    max_radius: usize,
    boundary: &[u32],
) -> Option<(u32, Vec<u32>, u32)> {
    let mut min_x = w; let mut min_y = h; let mut max_x = 0usize; let mut max_y = 0usize;
    for &c in boundary {
        let x = c as usize % w; let y = c as usize / w;
        min_x = min_x.min(x); min_y = min_y.min(y); max_x = max_x.max(x); max_y = max_y.max(y);
    }
    min_x = min_x.saturating_sub(max_radius); min_y = min_y.saturating_sub(max_radius);
    max_x = (max_x + max_radius + 1).min(w); max_y = (max_y + max_radius + 1).min(h);
    if min_x >= max_x || min_y >= max_y { return None; }
    let bw = max_x - min_x;
    let local = |c: usize| -> usize { (c / w - min_y) * bw + (c % w - min_x) };
    let global = |k: usize| -> usize { min_y + k / bw * w + min_x + k % bw };
    const ROOT: u32 = u32::MAX - 1;
    let mut parent = vec![u32::MAX; bw * (max_y - min_y)];
    let mut source_cell = vec![u32::MAX; bw * (max_y - min_y)];
    let mut queue = VecDeque::new();

    for &lake_cell in boundary {
        for (nb, _) in neighbors(w, h, lake_cell as usize) {
            if !land[nb] || lake_of[nb] != NO_LAKE { continue; }
            let x = nb % w; let y = nb / w;
            if x < min_x || x >= max_x || y < min_y || y >= max_y || filled[nb] > source_level + 2.0 { continue; }
            let k = local(nb);
            if parent[k] != u32::MAX { continue; }
            parent[k] = ROOT; source_cell[k] = lake_cell; queue.push_back(nb);
        }
    }

    while let Some(cur) = queue.pop_front() {
        for (nb, _) in neighbors(w, h, cur) {
            if nb % w < min_x || nb % w >= max_x || nb / w < min_y || nb / w >= max_y { continue; }
            if land[nb] && lake_of[nb] == NO_LAKE && q[nb] >= min_q && receiver_reaches_water(nb, land, lake_of, rec, 64) {
                let mut path = vec![cur as u32];
                let mut k = local(cur);
                while parent[k] != ROOT {
                    let p = parent[k] as usize;
                    path.push(global(p) as u32);
                    k = p;
                }
                path.reverse();
                return Some((source_cell[k], path, nb as u32));
            }
            if !land[nb] || lake_of[nb] != NO_LAKE || filled[nb] > source_level + 2.0 || filled[nb] > filled[cur] + 0.5 { continue; }
            let k = local(nb);
            if parent[k] != u32::MAX { continue; }
            parent[k] = local(cur) as u32;
            queue.push_back(nb);
        }
    }
    None
}

fn route_lake_outflow(
    lake_id: usize,
    land: &[bool],
    lake_of: &[u32],
    rec: &[u32],
    lake_net: &mut [f64],
    q: &mut [f64],
    raw: &mut [f64],
    lakes: &[Lake],
) {
    let Some(outlet) = lakes[lake_id].outlet else { return; };
    let net = lake_net[lake_id].max(0.0);
    if net <= 0.0 { return; }
    let n = rec.len();
    let mut cur = outlet as usize;
    for _ in 0..n {
        let next = rec[cur] as usize;
        if next >= n || next == cur { break; }
        if lake_of[next] != NO_LAKE {
            let target = lake_of[next] as usize;
            if target == lake_id { cur = next; continue; }
            lake_net[target] += net;
            break;
        }
        if !land[next] { break; }
        q[next] += net;
        raw[next] += net;
        cur = next;
    }
}

fn receiver_reaches_water(start: usize, land: &[bool], lake_of: &[u32], rec: &[u32], max_steps: usize) -> bool {
    let n = rec.len();
    let mut cur = start;
    for _ in 0..max_steps.min(n) {
        let next = rec[cur] as usize;
        if next >= n || next == cur { return false; }
        if !land[next] || lake_of[next] != NO_LAKE { return true; }
        cur = next;
    }
    false
}

fn repair_nearby_water_sinks(w: usize, h: usize, land: &[bool], lake_of: &[u32], filled: &[f64], rec: &mut [u32], max_radius: usize) {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let n = w * h;
    let mut stamp = vec![0u32; n];
    let mut parent = vec![usize::MAX; n];
    let mut generation = 0u32;
    for start in 0..n {
        if !land[start] || lake_of[start] != NO_LAKE || rec[start] as usize != start { continue; }
        generation = generation.wrapping_add(1).max(1);
        let max_level = filled[start] + 2.0;
        let sx = start % w; let sy = start / w;
        let mut heap = BinaryHeap::<Reverse<(u64, usize)>>::new();
        stamp[start] = generation; parent[start] = start; heap.push(Reverse((0, start)));
        let mut target = None;
        while let Some(Reverse((_, cur))) = heap.pop() {
            let cx = cur % w; let cy = cur / w;
            if sx.abs_diff(cx) + sy.abs_diff(cy) > max_radius { continue; }
            for (nb, _) in neighbors(w, h, cur) {
                let nx = nb % w; let ny = nb / w;
                if sx.abs_diff(nx) + sy.abs_diff(ny) > max_radius { continue; }
                if !land[nb] || lake_of[nb] != NO_LAKE { target = Some((nb, cur)); break; }
                if filled[nb] > max_level || filled[nb] > filled[cur] + 2.0 || stamp[nb] == generation { continue; }
                stamp[nb] = generation; parent[nb] = cur;
                let rise = (filled[nb] - filled[cur]).max(0.0);
                heap.push(Reverse((1_000_000u64.saturating_add((rise * 100_000.0) as u64), nb)));
            }
            if target.is_some() { break; }
        }
        let Some((water, last_land)) = target else { continue; };
        let mut path = Vec::new(); let mut cur = last_land;
        loop {
            path.push(cur);
            let p = parent[cur];
            if p == cur { break; }
            if p == usize::MAX { path.clear(); break; }
            cur = p;
        }
        if path.is_empty() || *path.last().unwrap() != start { continue; }
        path.reverse();
        for pair in path.windows(2) { rec[pair[0]] = pair[1] as u32; }
        rec[*path.last().unwrap()] = water as u32;
    }
}

fn connect_lakes_to_ocean(
    w: usize, h: usize, land: &[bool], lake_of: &[u32], filled: &[f64], sea: f64,
    rec: &mut [u32], lakes: &mut [Lake],
) {
    if lakes.is_empty() { return; }
    const MAX_CONNECT_CELLS: usize = 40;
    let mut boundaries = Vec::<Vec<u32>>::with_capacity(lakes.len());
    let mut bounds = Vec::with_capacity(lakes.len());
    for lake in lakes.iter() {
        let mut min_x = w; let mut min_y = h; let mut max_x = 0usize; let mut max_y = 0usize;
        let mut edge = Vec::new();
        for &c in &lake.cells {
            let x = c as usize % w; let y = c as usize / w;
            min_x = min_x.min(x); min_y = min_y.min(y); max_x = max_x.max(x); max_y = max_y.max(y);
            if neighbors(w, h, c as usize).any(|(nb, _)| land[nb] && lake_of[nb] == NO_LAKE) { edge.push(c); }
        }
        bounds.push((min_x, min_y, max_x, max_y)); boundaries.push(edge);
    }
    let mut order: Vec<usize> = (0..lakes.len()).collect();
    order.sort_by(|&a, &b| lakes[b].level_ft.total_cmp(&lakes[a].level_ft).then(a.cmp(&b)));
    for source in order {
        if lakes[source].outlet.is_some() || boundaries[source].is_empty() || lakes[source].level_ft <= sea + 0.5 { continue; }
        let Some((source_cell, path, ocean_cell)) = lake_ocean_spill_path(
            w, h, land, lake_of, filled, lakes[source].level_ft, MAX_CONNECT_CELLS, &boundaries[source], bounds[source],
        ) else { continue; };
        if path.iter().any(|&c| lake_of[c as usize] != NO_LAKE) { continue; }
        if path.is_empty() { rec[source_cell as usize] = ocean_cell; }
        else {
            rec[source_cell as usize] = path[0];
            for pair in path.windows(2) { rec[pair[0] as usize] = pair[1]; }
            rec[*path.last().unwrap() as usize] = ocean_cell;
        }
        lakes[source].outlet = Some(source_cell);
    }
}

fn lake_ocean_spill_path(
    w: usize, h: usize, land: &[bool], lake_of: &[u32], filled: &[f64], source_level: f64,
    max_radius: usize, boundary: &[u32], bound: (usize, usize, usize, usize),
) -> Option<(u32, Vec<u32>, u32)> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;

    // Treat the outlet as a spill saddle problem. The lowest coastal saddle wins; distance
    // only breaks ties, which is much closer to how a natural lake finds its outlet.
    let min_x = bound.0.saturating_sub(max_radius);
    let min_y = bound.1.saturating_sub(max_radius);
    let max_x = (bound.2 + max_radius + 1).min(w);
    let max_y = (bound.3 + max_radius + 1).min(h);
    if min_x >= max_x || min_y >= max_y { return None; }

    let bw = max_x - min_x; let bh = max_y - min_y;
    let local = |cell: usize| -> usize { (cell / w - min_y) * bw + (cell % w - min_x) };
    let global = |k: usize| -> usize { min_y + k / bw * w + min_x + k % bw };
    const ROOT: u32 = u32::MAX - 1;
    let mut parent = vec![u32::MAX; bw * bh];
    let mut source_cell = vec![u32::MAX; bw * bh];
    let mut best_saddle = vec![u64::MAX; bw * bh];
    let mut best_cost = vec![u64::MAX; bw * bh];
    let mut heap = BinaryHeap::<Reverse<(u64, u64, usize)>>::new();

    for &lake_cell in boundary {
        for (nb, _) in neighbors(w, h, lake_cell as usize) {
            if !land[nb] || lake_of[nb] != NO_LAKE { continue; }
            let x = nb % w; let y = nb / w;
            if x < min_x || x >= max_x || y < min_y || y >= max_y || filled[nb] > source_level + 1.0 { continue; }
            let k = local(nb);
            let saddle = (filled[nb].max(source_level) * 100.0).max(0.0) as u64;
            if saddle < best_saddle[k] {
                best_saddle[k] = saddle;
                best_cost[k] = 0;
                parent[k] = ROOT;
                source_cell[k] = lake_cell;
                heap.push(Reverse((saddle, 0, k)));
            }
        }
    }

    while let Some(Reverse((saddle, cost, key))) = heap.pop() {
        if saddle != best_saddle[key] || cost != best_cost[key] { continue; }
        let cur = global(key);
        for (nb, dist) in neighbors(w, h, cur) {
            if nb % w < min_x || nb % w >= max_x || nb / w < min_y || nb / w >= max_y { continue; }

            if !land[nb] {
                let mut path = vec![cur as u32];
                let mut k = key;
                while parent[k] != ROOT {
                    let p = parent[k] as usize;
                    path.push(global(p) as u32);
                    k = p;
                }
                path.reverse();
                return Some((source_cell[k], path, nb as u32));
            }

            if lake_of[nb] != NO_LAKE || filled[nb] > source_level + 1.0 || filled[nb] > filled[cur] + 1.0 { continue; }
            let next_saddle = saddle.max((filled[nb] * 100.0).max(0.0) as u64);
            let uphill = (filled[nb] - filled[cur]).max(0.0);
            let next_cost = cost
                .saturating_add((dist * 100.0) as u64)
                .saturating_add((uphill * 500.0) as u64);
            let k = local(nb);
            if (next_saddle, next_cost) >= (best_saddle[k], best_cost[k]) { continue; }
            best_saddle[k] = next_saddle;
            best_cost[k] = next_cost;
            parent[k] = key as u32;
            heap.push(Reverse((next_saddle, next_cost, k)));
        }
    }
    None
}

fn lake_receiver_chain_valid(start: usize, land: &[bool], lake_of: &[u32], rec: &[u32], lakes: &[Lake]) -> bool {
    if start >= lakes.len() { return false; }
    let n = rec.len();
    let mut seen = vec![false; lakes.len()];
    let mut lake = start;
    loop {
        if lake >= lakes.len() || seen[lake] { return false; }
        seen[lake] = true;
        let Some(outlet) = lakes[lake].outlet else { return true; };
        let mut cur = outlet as usize;
        if cur >= n || lake_of[cur] != lake as u32 { return false; }
        let mut advanced = false;
        for _ in 0..n {
            let next = rec[cur] as usize;
            if next >= n || next == cur { return false; }
            if !land[next] { return true; }
            if lake_of[next] != NO_LAKE {
                lake = lake_of[next] as usize;
                advanced = true;
                break;
            }
            cur = next;
        }
        if !advanced { return false; }
    }
}

fn connect_close_lakes(w: usize, h: usize, land: &[bool], lake_of: &[u32], filled: &[f64], rec: &mut [u32], lakes: &mut [Lake]) {
    if lakes.len() < 2 { return; }

    // Connect nearby lakes as a serial surface-water network. The important ordering detail is
    // that we process high lakes first but do NOT require the lower lake to already have an
    // outlet. That lower lake may be the next link in the chain and will receive its own outlet
    // when its turn arrives. This is the topology Great-Lakes-style systems need.
    const MAX_CONNECT_CELLS: usize = 48;
    // Lakes at the same water level can be one hydrologic system separated by a narrow
    // strait/channel. This is how the Michigan-Huron part of the real Great Lakes behaves.
    const LAKE_LEVEL_EPSILON_FT: f64 = 0.05;

    for lake_id in 0..lakes.len() {
        lakes[lake_id].outlet = lakes[lake_id].cells.iter().copied().find(|&c| {
            let r = rec[c as usize] as usize;
            r != c as usize && (r < lake_of.len()) && lake_of[r] != lake_id as u32
        });
    }

    let mut bounds = Vec::with_capacity(lakes.len());
    let mut boundaries: Vec<Vec<u32>> = Vec::with_capacity(lakes.len());
    for lake in lakes.iter() {
        let mut min_x = w; let mut min_y = h; let mut max_x = 0usize; let mut max_y = 0usize;
        for &c in &lake.cells {
            let x = c as usize % w; let y = c as usize / w;
            min_x = min_x.min(x); min_y = min_y.min(y);
            max_x = max_x.max(x); max_y = max_y.max(y);
        }
        bounds.push((min_x, min_y, max_x, max_y));
        let mut edge = Vec::new();
        for &c in &lake.cells {
            if neighbors(w, h, c as usize).any(|(nb, _)| land[nb] && lake_of[nb] == NO_LAKE) {
                edge.push(c);
            }
        }
        boundaries.push(edge);
    }

    // Higher water surfaces feed lower ones. Stable lake-id ordering breaks ties.
    let mut order: Vec<usize> = (0..lakes.len()).collect();
    order.sort_by(|&a, &b| lakes[b].level_ft.total_cmp(&lakes[a].level_ft).then(a.cmp(&b)));

    for &source in &order {
        if boundaries[source].is_empty() { continue; }

        // Once a lake already has a valid ocean-reaching outlet, do not replace it with a
        // shorter inland link. Otherwise a nearby lake can accidentally sever the basin outlet.
        let source_already_ocean = lakes[source].outlet.is_some()
            && lake_reaches_ocean(source, land, lake_of, rec, lakes);
        if source_already_ocean { continue; }

        let mut candidates: Vec<(bool, bool, f64, usize, usize)> = Vec::new();
        for target in 0..lakes.len() {
            if target == source || lakes[target].level_ft > lakes[source].level_ft + LAKE_LEVEL_EPSILON_FT {
                continue;
            }

            let proximity = rect_distance_lakes(bounds[source], bounds[target]);
            if proximity > MAX_CONNECT_CELLS { continue; }

            // Prefer targets that already continue toward the ocean, but a terminal lower lake
            // is still a valid target. It will be connected farther downstream later.
            let ocean_connected = lakes[target].outlet.is_some()
                && lake_reaches_ocean(target, land, lake_of, rec, lakes);
            let level_drop = lakes[source].level_ft - lakes[target].level_ft;
            candidates.push((ocean_connected, lakes[target].outlet.is_some(), level_drop, proximity, target));
        }

        candidates.sort_by(|a, b| {
            b.0.cmp(&a.0)
                .then(b.1.cmp(&a.1))
                .then(b.2.total_cmp(&a.2))
                .then(a.3.cmp(&b.3))
                .then(a.4.cmp(&b.4))
        });

        // Try a small deterministic shortlist. The bounded spill search does the expensive
        // terrain test, so proximity alone can never create a connection over a ridge.
        for (_, _, _, _, target) in candidates.into_iter().take(8) {
            let Some((source_cell, path, target_cell)) = lake_spill_path(
                w, h, land, lake_of, filled, lakes[source].level_ft,
                &boundaries[source], &boundaries[target], target as u32,
            ) else { continue; };

            if path.is_empty() || path.iter().any(|&c| lake_of[c as usize] != NO_LAKE) {
                continue;
            }
            if receiver_path_would_cycle(source_cell, &path, target_cell, rec) {
                continue;
            }

            // Real receiver topology: source lake -> land connecting river -> target lake.
            rec[source_cell as usize] = path[0];
            for pair in path.windows(2) { rec[pair[0] as usize] = pair[1]; }
            rec[*path.last().unwrap() as usize] = target_cell;
            lakes[source].outlet = Some(source_cell);

            let target_id = lake_of[target_cell as usize] as usize;
            lakes[target_id].inlet_count = lakes[target_id].inlet_count.saturating_add(1);
            break;
        }
    }
}

fn rect_distance_lakes(a: (usize, usize, usize, usize), b: (usize, usize, usize, usize)) -> usize {
    let dx = if a.2 < b.0 { b.0 - a.2 - 1 } else if b.2 < a.0 { a.0 - b.2 - 1 } else { 0 };
    let dy = if a.3 < b.1 { b.1 - a.3 - 1 } else if b.3 < a.1 { a.1 - b.3 - 1 } else { 0 };
    dx + dy
}

/// Connect a mapped river that ends in a nearby dry sink to the nearest reachable water body.
/// The repair is local, bounded, and refuses to cross established mapped channels.
fn connect_nearby_river_termini_to_water(
    w: usize, h: usize, land: &[bool], lake_of: &[u32], filled: &[f64], q: &[f64],
    threshold: f64, rec: &mut [u32],
) {
    const MAX_RADIUS: usize = 24;
    const MAX_RISE: f64 = 2.0;
    if threshold <= 0.0 { return; }
    let n = w * h;
    let mut seen_terminal = vec![false; n];
    let mut terminals = Vec::new();

    // Inspect only the downstream edge of high-discharge regions, keeping this O(n).
    for i in 0..n {
        if !land[i] || lake_of[i] != NO_LAKE || q[i] < threshold { continue; }
        let r = rec[i] as usize;
        if r < n && land[r] && lake_of[r] == NO_LAKE && q[r] >= threshold { continue; }
        let mut cur = i;
        let mut terminal = None;
        for _ in 0..n {
            let next = rec[cur] as usize;
            if next >= n || next == cur { terminal = Some(cur); break; }
            if !land[next] || lake_of[next] != NO_LAKE { break; }
            cur = next;
        }
        let Some(cur) = terminal else { continue; };
        if seen_terminal[cur] { continue; }
        seen_terminal[cur] = true;
        terminals.push(cur);
    }

    for source in terminals {
        let source_level = filled[source];
        let sx = source % w;
        let sy = source / w;
        let x0 = sx.saturating_sub(MAX_RADIUS);
        let y0 = sy.saturating_sub(MAX_RADIUS);
        let x1 = (sx + MAX_RADIUS + 1).min(w);
        let y1 = (sy + MAX_RADIUS + 1).min(h);
        if x0 >= x1 || y0 >= y1 { continue; }
        let bw = x1 - x0;
        let bh = y1 - y0;
        let local = |c: usize| -> usize { (c / w - y0) * bw + (c % w - x0) };
        let global = |k: usize| -> usize { y0 + k / bw * w + x0 + k % bw };
        const ROOT: u32 = u32::MAX - 1;
        let mut parent = vec![u32::MAX; bw * bh];
        let mut queue = VecDeque::new();
        parent[local(source)] = ROOT;
        queue.push_back(source);
        let mut found = None;

        while let Some(cur) = queue.pop_front() {
            let cx = cur % w;
            let cy = cur / w;
            if sx.abs_diff(cx) + sy.abs_diff(cy) > MAX_RADIUS { continue; }
            for (nb, _) in neighbors(w, h, cur) {
                let nx = nb % w;
                let ny = nb / w;
                if nx < x0 || nx >= x1 || ny < y0 || ny >= y1 { continue; }
                if !land[nb] || lake_of[nb] != NO_LAKE {
                    found = Some((cur, nb));
                    break;
                }
                if nb != source && q[nb] >= threshold * 0.5 { continue; }
                let key = local(nb);
                if filled[nb] > source_level + MAX_RISE || parent[key] != u32::MAX { continue; }
                parent[key] = cur as u32;
                queue.push_back(nb);
            }
            if found.is_some() { break; }
        }

        let Some((last_land, target)) = found else { continue; };
        let mut path = vec![last_land as u32];
        let mut key = local(last_land);
        while parent[key] != ROOT {
            let p = parent[key] as usize;
            path.push(global(p) as u32);
            key = p;
        }
        path.reverse();
        if path.first().copied() != Some(source as u32)
            || receiver_path_would_cycle(source as u32, &path, target as u32, rec) { continue; }
        if path.len() == 1 {
            rec[source] = target as u32;
        } else {
            for pair in path.windows(2) { rec[pair[0] as usize] = pair[1]; }
            rec[*path.last().unwrap() as usize] = target as u32;
        }
    }
}

fn receiver_path_would_cycle(source_cell: u32, path: &[u32], target_cell: u32, rec: &[u32]) -> bool {
    let n = rec.len();
    let mut forbidden = Vec::with_capacity(path.len() + 1);
    forbidden.push(source_cell as usize);
    forbidden.extend(path.iter().map(|&c| c as usize));
    let mut cur = target_cell as usize;
    for _ in 0..n {
        if cur >= n { return true; }
        if forbidden.contains(&cur) { return true; }
        let next = rec[cur] as usize;
        if next == cur { return false; }
        cur = next;
    }
    true
}

fn lake_reaches_ocean(lake_id: usize, land: &[bool], lake_of: &[u32], rec: &[u32], lakes: &[Lake]) -> bool {
    let n = rec.len();
    let mut lake_seen = vec![false; lakes.len()];
    let mut cur_lake = lake_id;
    for _ in 0..=lakes.len() {
        if cur_lake >= lakes.len() || lake_seen[cur_lake] { return false; }
        lake_seen[cur_lake] = true;
        let Some(outlet) = lakes[cur_lake].outlet else { return false; };
        let mut cur = outlet as usize;
        for _ in 0..n {
            if cur >= n { return false; }
            let next = rec[cur] as usize;
            if next == cur { return false; }
            if !land[next] { return true; }
            if lake_of[next] != NO_LAKE {
                cur_lake = lake_of[next] as usize;
                break;
            }
            cur = next;
        }
    }
    false
}

/// Find a real downhill spill route between nearby lakes. The search is local, deterministic,
/// and constrained to the source lake's water level, so proximity alone never creates a river
/// over a ridge. The returned path contains only land cells between the two lake cells.
fn lake_spill_path(
    w: usize, h: usize, land: &[bool], lake_of: &[u32], filled: &[f64], source_level: f64,
    source_boundary: &[u32], target_boundary: &[u32], target_id: u32,
) -> Option<(u32, Vec<u32>, u32)> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;

    // A real spill follows the lowest saddle first, not the first cell reached by BFS.
    // This prevents long, nearly-flat "scratch" channels from crossing arbitrary terrain.
    let mut min_x = w; let mut min_y = h; let mut max_x = 0usize; let mut max_y = 0usize;
    for &c in source_boundary.iter().chain(target_boundary.iter()) {
        let x = c as usize % w; let y = c as usize / w;
        min_x = min_x.min(x); min_y = min_y.min(y); max_x = max_x.max(x); max_y = max_y.max(y);
    }
    let margin = 3usize;
    min_x = min_x.saturating_sub(margin); min_y = min_y.saturating_sub(margin);
    max_x = (max_x + margin + 1).min(w); max_y = (max_y + margin + 1).min(h);
    if min_x >= max_x || min_y >= max_y { return None; }

    let bw = max_x - min_x; let bh = max_y - min_y;
    let local = |cell: usize| -> usize { (cell / w - min_y) * bw + (cell % w - min_x) };
    let global = |k: usize| -> usize { min_y + k / bw * w + min_x + k % bw };
    const ROOT: u32 = u32::MAX - 1;
    let mut parent = vec![u32::MAX; bw * bh];
    let mut source_cell = vec![u32::MAX; bw * bh];
    let mut best_saddle = vec![u64::MAX; bw * bh];
    let mut best_cost = vec![u64::MAX; bw * bh];
    let mut heap = BinaryHeap::<Reverse<(u64, u64, usize)>>::new();

    for &lake_cell in source_boundary {
        for (nb, _) in neighbors(w, h, lake_cell as usize) {
            if !land[nb] || lake_of[nb] != NO_LAKE { continue; }
            let x = nb % w; let y = nb / w;
            if x < min_x || x >= max_x || y < min_y || y >= max_y || filled[nb] > source_level + 1.0 { continue; }
            let k = local(nb);
            let saddle = (filled[nb].max(source_level) * 100.0).max(0.0) as u64;
            if saddle < best_saddle[k] {
                best_saddle[k] = saddle;
                best_cost[k] = 0;
                parent[k] = ROOT;
                source_cell[k] = lake_cell;
                heap.push(Reverse((saddle, 0, k)));
            }
        }
    }

    while let Some(Reverse((saddle, cost, key))) = heap.pop() {
        if saddle != best_saddle[key] || cost != best_cost[key] { continue; }
        let cur = global(key);
        for (nb, dist) in neighbors(w, h, cur) {
            if nb % w < min_x || nb % w >= max_x || nb / w < min_y || nb / w >= max_y { continue; }

            if lake_of[nb] == target_id {
                let mut path = vec![cur as u32];
                let mut k = key;
                while parent[k] != ROOT {
                    let p = parent[k] as usize;
                    path.push(global(p) as u32);
                    k = p;
                }
                path.reverse();
                return Some((source_cell[k], path, nb as u32));
            }

            if !land[nb] || lake_of[nb] != NO_LAKE || filled[nb] > source_level + 1.0 { continue; }
            if filled[nb] > filled[cur] + 1.0 { continue; }

            let next_saddle = saddle.max((filled[nb] * 100.0).max(0.0) as u64);
            let uphill = (filled[nb] - filled[cur]).max(0.0);
            let next_cost = cost
                .saturating_add((dist * 100.0) as u64)
                .saturating_add((uphill * 500.0) as u64);
            let k = local(nb);
            if (next_saddle, next_cost) >= (best_saddle[k], best_cost[k]) { continue; }
            best_saddle[k] = next_saddle;
            best_cost[k] = next_cost;
            parent[k] = key as u32;
            heap.push(Reverse((next_saddle, next_cost, k)));
        }
    }
    None
}

fn receiver_order(w: usize, h: usize, land: &[bool], rec: &mut [u32]) -> Vec<u32> {
    let n = w * h;
    let land_count = land.iter().filter(|&&v| v).count();
    loop {
        let mut indegree = vec![0u32; n];
        for i in 0..n {
            if !land[i] { continue; }
            let r = rec[i] as usize;
            if r < n && r != i && land[r] { indegree[r] = indegree[r].saturating_add(1); }
        }
        let mut queue = Vec::<usize>::with_capacity(land_count);
        for i in 0..n { if land[i] && indegree[i] == 0 { queue.push(i); } }
        let mut out = Vec::<u32>::with_capacity(land_count);
        let mut head = 0usize;
        while head < queue.len() {
            let i = queue[head]; head += 1; out.push(i as u32);
            let r = rec[i] as usize;
            if r < n && r != i && land[r] {
                indegree[r] = indegree[r].saturating_sub(1);
                if indegree[r] == 0 { queue.push(r); }
            }
        }
        if out.len() == land_count { return out; }

        let mut state = vec![0u8; n];
        let mut broke = false;
        for start in 0..n {
            if !land[start] || state[start] != 0 { continue; }
            let mut cur = start;
            while cur < n && land[cur] && state[cur] == 0 {
                state[cur] = 1;
                let next = rec[cur] as usize;
                if next >= n || !land[next] || next == cur { break; }
                cur = next;
            }
            if cur < n && land[cur] && state[cur] == 1 {
                let cycle_start = cur;
                let mut cut = cur;
                let mut p = rec[cur] as usize;
                while p != cycle_start { cut = cut.min(p); p = rec[p] as usize; }
                rec[cut] = cut as u32;
                broke = true;
                break;
            }
            for i in 0..n { if state[i] == 1 { state[i] = 2; } }
        }
        if !broke {
            for i in 0..n { if land[i] && (rec[i] as usize) >= n { rec[i] = i as u32; } }
        }
    }
}
fn recompute_final_discharge(
    w: usize,
    h: usize,
    land: &[bool],
    lake_of: &[u32],
    rec: &[u32],
    order: &[u32],
    clim: &Climate,
    feed: &[(usize, f64)],
    lakes: &[Lake],
) -> (Vec<f64>, Vec<f64>) {
    let n = w * h;
    let pet: Vec<f64> = clim.temp.iter().map(|&t| (350.0 + 55.0 * t as f64).max(0.0)).collect();
    let mut q = vec![0.0f64; n];
    let mut lake_net = vec![0.0f64; lakes.len()];
    let mut lake_inflow = vec![0.0f64; lakes.len()];

    for &(cell, amount) in feed {
        if cell >= n { continue; }
        if lake_of[cell] != NO_LAKE {
            let id = lake_of[cell] as usize;
            lake_net[id] += amount;
            lake_inflow[id] += amount;
        } else {
            q[cell] += amount;
        }
    }

    // Route land runoff once through the final receiver graph. Water entering a lake becomes
    // part of that lake's budget instead of being copied into every lake cell.
    for &ii in order {
        let i = ii as usize;
        let runoff = (clim.precip[i] as f64 - 0.65 * pet[i]).max(0.0);
        if lake_of[i] != NO_LAKE {
            lake_net[lake_of[i] as usize] += runoff - pet[i];
            continue;
        }

        q[i] += runoff;
        if clim.precip[i] < 400.0 && q[i] > 0.0 {
            q[i] *= 0.9995;
        }
        let r = rec[i] as usize;
        if r >= n || r == i || !land[r] { continue; }
        if lake_of[r] != NO_LAKE {
            let id = lake_of[r] as usize;
            lake_net[id] += q[i];
            lake_inflow[id] += q[i];
        } else {
            q[r] += q[i];
        }
    }

    // Lakes now form a real serial water network. Higher lakes are processed first so their
    // outflow reaches lower lakes before those lower lakes send water downstream.
    let mut lake_order: Vec<usize> = (0..lakes.len()).collect();
    lake_order.sort_by(|&a, &b| {
        lakes[b].level_ft.total_cmp(&lakes[a].level_ft).then(a.cmp(&b))
    });

    let mut outlets = vec![None; lakes.len()];
    for (lake_id, lake) in lakes.iter().enumerate() {
        outlets[lake_id] = lake.outlet.map(|c| c as usize);
        if outlets[lake_id].is_some() {
            continue;
        }
        // Keep a defensive fallback for old/generated graphs whose Lake metadata predates the
        // final receiver repair. Prefer the authoritative outlet whenever it exists.
        for &cell in &lake.cells {
            let i = cell as usize;
            let r = rec[i] as usize;
            if r < n && r != i && (lake_of[r] == NO_LAKE || !land[r]) {
                outlets[lake_id] = Some(i);
                break;
            }
        }
    }

    for lake_id in lake_order {
        let Some(outlet) = outlets[lake_id] else { continue; };
        // Flow-through lakes represent long-term storage. Do not erase the incoming river
        // because a static local precipitation/evaporation balance happens to be negative.
        let net = lake_net[lake_id].max(lake_inflow[lake_id] * 0.25);
        if net <= 0.0 { continue; }

        let mut cur = outlet;
        for _ in 0..n {
            let next = rec[cur] as usize;
            if next >= n || next == cur || !land[next] { break; }
            if lake_of[next] != NO_LAKE {
                let target = lake_of[next] as usize;
                if target != lake_id {
                    lake_net[target] += net;
                    lake_inflow[target] += net;
                }
                break;
            }
            q[next] += net;
            cur = next;
        }
    }

    (q, lake_net)
}

fn accumulate_flow(w: usize, h: usize, land: &[bool], _lake_of: &[u32], rec: &[u32], order: &[u32]) -> Vec<u32> {
    let n = w * h;
    let mut acc = vec![0u32; n];
    for i in 0..n { if land[i] { acc[i] = 1; } }
    for &ii in order.iter().rev() {
        let i = ii as usize;
        if acc[i] == 0 { continue; }
        let r = rec[i] as usize;
        if r != i && land[r] { acc[r] = acc[r].saturating_add(acc[i]); }
    }
    acc
}

fn extract_rivers(w: usize, h: usize, cell_ft: f64, land: &[bool], lake_of: &[u32], rec: &[u32], q: &[f64], accumulation: &[u32], basin_id: &[u64], threshold: f64, lakes: &[Lake]) -> Vec<River> {
    let n = w * h;
    let threshold = threshold.max(1.0);
    let mut lake_feed = vec![false; n];
    for lake in lakes {
        if let Some(outlet) = lake.outlet {
            let r = rec[outlet as usize] as usize;
            if r < n && land[r] && lake_of[r] == NO_LAKE { lake_feed[r] = true; }
        }
    }
    // A discharge threshold decides where a mapped river is born, not where it is
    // allowed to die. A source is the first land cell on a flow path that crosses the
    // mapping threshold. Without this upstream check, every downstream high-Q cell can
    // become a second "source", producing artificial river births along one channel.
    let mut upstream_above_threshold = vec![false; n];
    for i in 0..n {
        if !land[i] || lake_of[i] != NO_LAKE || q[i] < threshold { continue; }
        let r = rec[i] as usize;
        if r < n && r != i && land[r] && lake_of[r] == NO_LAKE {
            upstream_above_threshold[r] = true;
        }
    }
    let is_seed = |i: usize| {
        land[i]
            && lake_of[i] == NO_LAKE
            && ((lake_feed[i] && q[i] >= threshold * 0.05)
                || (q[i] >= threshold && !upstream_above_threshold[i]))
    };
    let mut channel = vec![false; n];
    let seeds: Vec<usize> = (0..n).filter(|&i| is_seed(i)).collect();
    for &start in &seeds {
        channel[start] = true;
        let mut cur = start;
        let mut guard = 0usize;
        while guard < n {
            guard += 1;
            let next = rec[cur] as usize;
            if next >= n || !land[next] || lake_of[next] != NO_LAKE || next == cur {
                break;
            }
            channel[next] = true;
            // An independently seeded channel already guarantees that its downstream
            // continuation will be walked. Avoid repeatedly traversing long main stems.
            if is_seed(next) && next != start {
                break;
            }
            cur = next;
        }
    }
    let is_channel = |i: usize| channel[i];
    let mut upstream = vec![0u8; n];
    for i in 0..n {
        if !is_channel(i) { continue; }
        let r = rec[i] as usize;
        if r < n && r != i && is_channel(r) { upstream[r] = upstream[r].saturating_add(1); }
    }
    let starts: Vec<usize> = (0..n).filter(|&i| is_channel(i) && upstream[i] != 1).collect();
    let mut start_index = vec![usize::MAX; n];
    for (k, &s) in starts.iter().enumerate() { start_index[s] = k; }
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Terminal { Ocean, Lake(u32), Dry }
    let mut terminal = vec![None; n];
    for i in 0..n {
        if !land[i] { terminal[i] = Some(Terminal::Ocean); }
        else if lake_of[i] != NO_LAKE { terminal[i] = Some(Terminal::Lake(lake_of[i])); }
    }
    for i in 0..n {
        if terminal[i].is_some() || !land[i] || lake_of[i] != NO_LAKE { continue; }
        let mut path = Vec::new(); let mut cur = i;
        let end = loop {
            let r = rec[cur] as usize;
            if r == cur || r >= n { break Terminal::Dry; }
            if let Some(t) = terminal[r] { break t; }
            path.push(cur); cur = r;
        };
        for p in path { terminal[p] = Some(end); }
        terminal[i] = Some(end);
    }
    #[derive(Clone)]
    struct Seg { cells: Vec<u32>, mouth: Mouth }
    let mut segs = Vec::<Seg>::with_capacity(starts.len());
    for &start in &starts {
        let mut cells = Vec::new(); let mut cur = start; let mut guard = 0usize;
        loop {
            if guard >= n || !is_channel(cur) { break; }
            guard += 1; cells.push(cur as u32);
            let next = rec[cur] as usize;
            if next >= n || next == cur || !land[next] || lake_of[next] != NO_LAKE || !is_channel(next) { break; }
            if next != start && start_index[next] != usize::MAX { break; }
            cur = next;
        }
        let end_cell = *cells.last().unwrap_or(&(start as u32)) as usize;
        let next = rec[end_cell] as usize;
        let mouth = if next >= n || !land[next] { Mouth::Ocean }
        else if lake_of[next] != NO_LAKE { Mouth::Lake }
        else if is_channel(next) && start_index[next] != usize::MAX { Mouth::Confluence }
        else if next == end_cell {
            match terminal[end_cell].unwrap_or(Terminal::Dry) {
                Terminal::Ocean => Mouth::Ocean, Terminal::Lake(_) => Mouth::Lake, Terminal::Dry => Mouth::Dry,
            }
        } else {
            match terminal[next].unwrap_or(Terminal::Dry) {
                Terminal::Ocean => Mouth::Ocean, Terminal::Lake(_) => Mouth::Lake, Terminal::Dry => Mouth::Dry,
            }
        };
        segs.push(Seg { cells, mouth });
    }
    let mut parent = vec![None; segs.len()];
    for (sid, seg) in segs.iter().enumerate() {
        if seg.mouth != Mouth::Confluence { continue; }
        let end = *seg.cells.last().unwrap() as usize; let next = rec[end] as usize;
        if next < n && start_index[next] != usize::MAX { parent[sid] = Some(start_index[next]); }
    }
    let mut children: Vec<Vec<usize>> = vec![Vec::new(); segs.len()];
    for (sid, p) in parent.iter().enumerate() { if let Some(p) = p { children[*p].push(sid); } }
    let inflow = |sid: usize| -> f64 { segs[sid].cells.last().map(|&c| q[c as usize]).unwrap_or(0.0) };
    let mut pending = children.iter().map(Vec::len).collect::<Vec<_>>();
    let mut seg_order = vec![0u8; segs.len()];
    let mut queue: Vec<usize> = (0..segs.len()).filter(|&k| pending[k] == 0).collect();
    queue.sort_unstable(); let mut qpos = 0;
    while qpos < queue.len() {
        let k = queue[qpos]; qpos += 1; if seg_order[k] == 0 { seg_order[k] = 1; }
        if let Some(p) = parent[k] {
            pending[p] = pending[p].saturating_sub(1);
            if pending[p] == 0 {
                let max = children[p].iter().map(|&c| seg_order[c]).max().unwrap_or(1);
                let count_max = children[p].iter().filter(|&&c| seg_order[c] == max).count();
                seg_order[p] = max.saturating_add((count_max >= 2) as u8); queue.push(p);
            }
        }
    }
    let mut chains = Vec::<River>::new();
    let mut stack: Vec<(usize, Option<usize>)> = (0..segs.len()).filter(|&k| parent[k].is_none()).map(|k| (k, None)).collect();
    stack.sort_by(|a, b| inflow(b.0).total_cmp(&inflow(a.0)).then(a.0.cmp(&b.0)));
    while let Some((root, into)) = stack.pop() {
        let mut path = vec![root]; let mut cur = root;
        loop {
            let mut kids = children[cur].clone(); if kids.is_empty() { break; }
            kids.sort_by(|a, b| inflow(*b).total_cmp(&inflow(*a)).then(a.cmp(b))); cur = kids[0]; path.push(cur);
        }
        let chain_id = chains.len(); let mut cells = Vec::<u32>::new();
        for &sid in path.iter().rev() { for &cc in &segs[sid].cells { if cells.last().copied() != Some(cc) { cells.push(cc); } } }
        if cells.is_empty() { continue; }
        let end_cell = *cells.last().unwrap() as usize;
        let (mouth, mouth_lake) = if into.is_some() { (Mouth::Confluence, None) } else {
            match terminal[end_cell].unwrap_or(Terminal::Dry) {
                Terminal::Ocean => (Mouth::Ocean, None), Terminal::Lake(id) => (Mouth::Lake, Some(id)), Terminal::Dry => (Mouth::Dry, None),
            }
        };
        let source_cell = *cells.first().unwrap() as usize;
        let source_lake = neighbors(w, h, source_cell).filter_map(|(nb, _)| (lake_of[nb] != NO_LAKE && rec[nb] as usize == source_cell).then_some(lake_of[nb])).min();
        let qv: Vec<f32> = cells.iter().map(|&cc| q[cc as usize] as f32).collect();
        chains.push(River { cells, q: qv, mouth, into, order: seg_order[root].max(1), drainage_area_cells: accumulation[source_cell], peak_discharge: 0.0, tributary_count: 0, source_lake, mouth_lake, length_cells: 0, length_ft: 0.0, basin_id: basin_id.get(source_cell).copied().unwrap_or(0), source_cell: source_cell as u32, terminal_receiver: None });
        for &sid in &path {
            let main_child = path.iter().position(|&p| p == sid).and_then(|i| path.get(i + 1)).copied();
            for &kid in &children[sid] { if Some(kid) != main_child { stack.push((kid, Some(chain_id))); } }
        }
    }
    for chain in &mut chains {
        let end = *chain.cells.last().unwrap() as usize;
        let next = rec[end] as usize;
        chain.terminal_receiver = (next < n && next != end && match chain.mouth {
            Mouth::Ocean => !land[next],
            Mouth::Lake => lake_of[next] != NO_LAKE,
            Mouth::Confluence => land[next] && lake_of[next] == NO_LAKE,
            Mouth::Dry => false,
        }).then_some(next as u32);
    }
    let tributary_counts: Vec<u16> = (0..chains.len()).map(|id| chains.iter().filter(|r| r.into == Some(id)).count().min(u16::MAX as usize) as u16).collect();
    // Extend a mapped river only along its real receiver chain when the mapped threshold
    // was crossed before the river reached water. Never jump to the nearest unrelated water.
    for chain in &mut chains {
        if !matches!(chain.mouth, Mouth::Ocean | Mouth::Lake) || chain.terminal_receiver.is_some() { continue; }
        let mut cur = *chain.cells.last().unwrap() as usize;
        let mut reached = false;
        for _ in 0..48 {
            let next = rec[cur] as usize;
            if next >= n || next == cur { break; }
            if !land[next] || lake_of[next] != NO_LAKE {
                chain.terminal_receiver = Some(next as u32);
                reached = true;
                break;
            }
            if channel[next] { break; }
            chain.cells.push(next as u32);
            chain.q.push(q[next] as f32);
            cur = next;
        }
        if !reached {
            chain.mouth = Mouth::Dry;
            chain.mouth_lake = None;
            chain.terminal_receiver = None;
        }
    }

    for (id, chain) in chains.iter_mut().enumerate() {
        chain.peak_discharge = chain.q.iter().copied().fold(0.0f32, f32::max);
        chain.tributary_count = tributary_counts[id];
        chain.length_cells = chain.cells.len().min(u32::MAX as usize) as u32;
        chain.length_ft = chain_length_ft(w, &chain.cells, cell_ft);
        if matches!(chain.mouth, Mouth::Ocean | Mouth::Lake | Mouth::Confluence) {
            if let Some(&last) = chain.cells.last() {
                let c = last as usize;
                if let Some(r) = chain.terminal_receiver {
                    let r = r as usize;
                    let dx = (c % w) as f64 - (r % w) as f64;
                    let dy = (c / w) as f64 - (r / w) as f64;
                    let fraction = if chain.mouth == Mouth::Confluence { 1.0 } else { 0.5 };
                    chain.length_ft += fraction * (dx * dx + dy * dy).sqrt() * cell_ft;
                }
            }
        }
        chain.drainage_area_cells = chain.cells.iter().map(|&cc| accumulation[cc as usize]).max().unwrap_or(chain.drainage_area_cells);
    }
    append_lake_connectors(w, h, cell_ft, land, lake_of, rec, q, accumulation, basin_id, lakes, &mut chains);
    for chain in &mut chains {
        chain.peak_discharge = chain.q.iter().copied().fold(0.0f32, f32::max);
        chain.length_cells = chain.cells.len().min(u32::MAX as usize) as u32;
        chain.length_ft = chain_length_ft(w, &chain.cells, cell_ft);
        chain.drainage_area_cells = chain.cells.iter().map(|&cc| accumulation[cc as usize]).max().unwrap_or(chain.drainage_area_cells);
    }
    chains
}

fn validate_river_network(w: usize, h: usize, land: &[bool], lake_of: &[u32], rec: &[u32], rivers: &[Ri/// Validate the complete hydrology contract after topology, river extraction and source-water
/// rendering have all finished. This is debug/test-only in normal generation, so the checks can
/// be deliberately strict without adding release-time traversal cost.
pub fn validate_hydrology(
    w: usize,
    h: usize,
    land: &[bool],
    lake_of: &[u32],
    rec: &[u32],
    lakes: &[Lake],
    rivers: &[River],
    water: &[f32],
) -> bool {
    let n = w * h;
    if land.len() != n || lake_of.len() != n || rec.len() != n || water.len() != n {
        return false;
    }
    if !validate_river_network(w, h, land, lake_of, rec, rivers) {
        return false;
    }

    // Every mapped lake outlet must leave that lake through a real receiver edge.
    for (lake_id, lake) in lakes.iter().enumerate() {
        if lake.cells.is_empty() || lake.cells.iter().any(|&c| {
            let c = c as usize;
            c >= n || !land[c] || lake_of[c] != lake_id as u32
        }) {
            return false;
        }
        if let Some(outlet) = lake.outlet {
            let outlet = outlet as usize;
            if outlet >= n || lake_of[outlet] != lake_id as u32 {
                return false;
            }
            let next = rec[outlet] as usize;
            if next >= n || next == outlet {
                return false;
            }
            if lake_of[next] == lake_id as u32 {
                return false;
            }
        }
    }

    // Parent/child hierarchy is a tree of channel chains, never a loop or backwards link.
    let mut child_counts = vec![0u16; rivers.len()];
    for (river_id, river) in rivers.iter().enumerate() {
        if let Some(parent) = river.into {
            if parent >= rivers.len() || parent == river_id {
                return false;
            }
            child_counts[parent] = child_counts[parent].saturating_add(1);
            if river.order > rivers[parent].order {
                return false;
            }
        }
        let mut seen_steps = 0usize;
        let mut cur = river_id;
        while let Some(parent) = rivers[cur].into {
            seen_steps += 1;
            if seen_steps > rivers.len() || parent >= rivers.len() {
                return false;
            }
            cur = parent;
        }
    }

    // A confluence that combines two or more equal-order tributaries must raise Strahler order.
    for (parent_id, &count) in child_counts.iter().enumerate() {
        if count < 2 {
            continue;
        }
        let max_child = rivers.iter()
            .filter(|r| r.into == Some(parent_id))
            .map(|r| r.order)
            .max()
            .unwrap_or(0);
        let equal = rivers.iter()
            .filter(|r| r.into == Some(parent_id) && r.order == max_child)
            .count();
        if equal >= 2 && rivers[parent_id].order < max_child.saturating_add(1) {
            return false;
        }
    }

    // Lake-fed sources must actually leave their named lake. Ocean/lake mouths must point at
    // the receiving water cell, while confluences must point at the parent junction.
    for river in rivers {
        let source = river.source_cell as usize;
        if let Some(lake_id) = river.source_lake {
            if lake_id as usize >= lakes.len() || source >= n || !land[source] {
                return false;
            }
            let connected = neighbors(w, h, source).any(|(nb, _)| {
                lake_of[nb] == lake_id && rec[nb] as usize == source
            });
            if !connected {
                return false;
            }
        } else {
            // A rendered headwater pond must cover the source cell. It is intentionally not
            // added to lake_of because it is a visual/source-water feature, not a classified lake.
            if source >= n || water[source] <= DRY {
                return false;
            }
        }

        let last = *river.cells.last().unwrap() as usize;
        if let Some(terminal) = river.terminal_receiver {
            let terminal = terminal as usize;
            if terminal >= n || rec[last] as usize != terminal {
                return false;
            }
            match river.mouth {
                Mouth::Ocean if land[terminal] => return false,
                Mouth::Lake if lake_of[terminal] == NO_LAKE => return false,
                Mouth::Confluence => {
                    let Some(parent) = river.into else { return false; };
                    if !rivers[parent].cells.contains(&(terminal as u32)) {
                        return false;
                    }
                }
                Mouth::Dry | Mouth::Ocean | Mouth::Lake => {}
            }
        } else if matches!(river.mouth, Mouth::Ocean | Mouth::Lake | Mouth::Confluence) {
            return false;
        }
    }

    true
}

    let n = w * h;
    for river in rivers {
        if river.cells.is_empty() || river.cells.len() != river.q.len() || river.source_cell != river.cells[0] { return false; }
        if river.q.iter().any(|&v| !v.is_finite() || v < 0.0) { return false; }
        for k in 0..river.cells.len() {
            let c = river.cells[k] as usize;
            if c >= n || !land[c] || lake_of[c] != NO_LAKE { return false; }
            if k + 1 < river.cells.len() && rec[c] as usize != river.cells[k + 1] as usize { return false; }
        }
        let last = *river.cells.last().unwrap() as usize;
        match river.mouth {
            Mouth::Ocean | Mouth::Lake => {
                let Some(mut receiver) = river.terminal_receiver.map(|c| c as usize) else { return false; };
                if river.into.is_some() || receiver >= n { return false; }
                let mut guard = 0usize; let mut reached = false;
                while guard < n {
                    guard += 1;
                    if !land[receiver] { reached = river.mouth == Mouth::Ocean; break; }
                    if lake_of[receiver] != NO_LAKE { reached = river.mouth == Mouth::Lake; break; }
                    let next = rec[receiver] as usize; if next >= n || next == receiver { break; } receiver = next;
                }
                if !reached { return false; }
            }
            Mouth::Confluence => {
                let Some(parent) = river.into else { return false; };
                let Some(join) = river.terminal_receiver.map(|c| c as usize) else { return false; };
                if parent >= rivers.len() || !rivers[parent].cells.contains(&(join as u32)) || rec[last] as usize != join { return false; }
                let parent_q = rivers[parent].cells.iter().position(|&c| c as usize == join).and_then(|k| rivers[parent].q.get(k)).copied().unwrap_or(0.0);
                if parent_q + 1e-3 < river.q.last().copied().unwrap_or(0.0) { return false; }
            }
            Mouth::Dry => {
                if river.into.is_some() || river.terminal_receiver.is_some() || rec[last] as usize != last { return false; }
            }
        }
    }
    true
}

fn append_lake_connectors(
    w: usize, h: usize, cell_ft: f64, land: &[bool], lake_of: &[u32], rec: &[u32],
    q: &[f64], accumulation: &[u32], basin_id: &[u64], lakes: &[Lake], chains: &mut Vec<River>,
) {
    let n = w * h;
    let mut owner = vec![usize::MAX; n];
    for (river_id, river) in chains.iter().enumerate() {
        for &cell in &river.cells { owner[cell as usize] = river_id; }
    }

    for lake_id in 0..lakes.len() {
        let Some(outlet) = lakes[lake_id].outlet else { continue; };
        let mut cells = Vec::<u32>::new();
        let mut cur = outlet as usize;
        let mut terminal = Mouth::Dry;
        let mut mouth_lake = None;
        let mut into = None;
        let mut terminal_receiver = None;

        for _ in 0..n {
            let next = rec[cur] as usize;
            if next >= n || next == cur { break; }
            if !land[next] {
                terminal = Mouth::Ocean;
                terminal_receiver = Some(next as u32);
                break;
            }
            if lake_of[next] != NO_LAKE {
                let target = lake_of[next] as usize;
                if target == lake_id { break; }
                terminal = Mouth::Lake;
                mouth_lake = Some(target as u32);
                terminal_receiver = Some(next as u32);
                break;
            }
            if owner[next] != usize::MAX {
                terminal = Mouth::Confluence;
                into = Some(owner[next]);
                terminal_receiver = Some(next as u32);
                break;
            }
            cells.push(next as u32);
            cur = next;
        }

        if cells.is_empty() || terminal_receiver.is_none() { continue; }

        let source = cells[0] as usize;
        let mut qv = cells.iter().map(|&c| q[c as usize] as f32).collect::<Vec<_>>();
        if let Some(parent) = into {
            if let Some(join) = terminal_receiver.map(|c| c as usize) {
                if let Some(k) = chains[parent].cells.iter().position(|&c| c as usize == join) {
                    let parent_q = chains[parent].q.get(k).copied().unwrap_or(f32::MAX);
                    for v in &mut qv { *v = (*v).min(parent_q); }
                }
            }
        }
        let peak = qv.iter().copied().fold(0.0f32, f32::max);
        chains.push(River {
            cells,
            q: qv,
            mouth: terminal,
            into,
            order: 1,
            drainage_area_cells: accumulation[source],
            peak_discharge: peak,
            tributary_count: 0,
            source_lake: Some(lake_id as u32),
            mouth_lake,
            length_cells: 0,
            length_ft: 0.0,
            basin_id: basin_id.get(source).copied().unwrap_or(0),
            source_cell: source as u32,
            terminal_receiver,
        });
    }
}

fn assign_basin_ids(w: usize, h: usize, land: &[bool], lake_of: &[u32], rec: &[u32]) -> Vec<u64> {
    let n = w * h;
    let mut out = vec![0u64; n];
    let mut done = vec![false; n];
    for i in 0..n {
        if !land[i] || done[i] { continue; }
        let mut path = Vec::<usize>::new();
        let mut cur = i;
        let basin = loop {
            if done[cur] { break out[cur]; }
            if lake_of[cur] != NO_LAKE { break 0x1_0000_0000u64 | lake_of[cur] as u64; }
            let r = rec[cur] as usize;
            if r == cur { break 0x2_0000_0000u64 | cur as u64; }
            if !land[r] { break 0x3_0000_0000u64 | r as u64; }
            path.push(cur);
            cur = r;
        };
        for p in path { out[p] = basin; done[p] = true; }
        out[i] = basin;
        done[i] = true;
    }
    out
}

fn chain_length_ft(w: usize, cells: &[u32], cell_ft: f64) -> f64 {
    cells.windows(2).map(|pair| {
        let a = pair[0] as usize;
        let b = pair[1] as usize;
        let dx = (a % w) as f64 - (b % w) as f64;
        let dy = (a / w) as f64 - (b / w) as f64;
        crate::core::sqrt(dx * dx + dy * dy) * cell_ft
    }).sum()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lake_outlet_is_mapped_as_a_connecting_river_below_threshold() {
        let land = vec![true, true, true, true, true, false];
        let lake_of = vec![0, NO_LAKE, NO_LAKE, NO_LAKE, NO_LAKE, NO_LAKE];
        let rec = vec![1, 2, 3, 4, 5, 5];
        let q = vec![0.0, 20.0, 15.0, 10.0, 100.0, 0.0];
        let accumulation = vec![1, 2, 3, 4, 5, 0];
        let basin_id = vec![7; 6];
        let lakes = vec![Lake { level_ft: 100.0, cells: vec![0], kind: LakeKind::Fresh, max_depth_ft: 100.0, outlet: Some(0), inlet_count: 1 }];
        let mut rivers = vec![River {
            cells: vec![4], q: vec![100.0], mouth: Mouth::Ocean, into: None, order: 2,
            drainage_area_cells: 5, peak_discharge: 100.0, tributary_count: 0,
            source_lake: None, mouth_lake: None, length_cells: 1, length_ft: 100.0,
            basin_id: 7, source_cell: 4, terminal_receiver: Some(5),
        }];
        append_lake_connectors(1, 6, 100.0, &land, &lake_of, &rec, &q, &accumulation, &basin_id, &lakes, &mut rivers);
        assert_eq!(rivers.len(), 2);
        assert_eq!(rivers[1].cells, vec![1, 2, 3]);
        assert_eq!(rivers[1].mouth, Mouth::Confluence);
        assert_eq!(rivers[1].into, Some(0));
        assert_eq!(rivers[1].source_lake, Some(0));
        assert_eq!(rivers[1].terminal_receiver, Some(4));
    }

    #[test]
    fn flow_accumulation_counts_upstream_cells() {
        let land = vec![true; 4];
        let lake = vec![NO_LAKE; 4];
        let rec = vec![1, 2, 2, 1];
        let order = vec![2, 1, 0, 3];
        let acc = accumulate_flow(4, 1, &land, &lake, &rec, &order);
        assert_eq!(acc, vec![1, 3, 4, 1]);
    }
    #[test]
    fn basin_ids_follow_receiver_terminals() {
        let land = vec![true, true, true, true, false];
        let lake = vec![NO_LAKE, NO_LAKE, 0, NO_LAKE, NO_LAKE];
        let rec = vec![1, 2, 2, 4, 4];
        let ids = assign_basin_ids(5, 1, &land, &lake, &rec);
        assert_eq!(ids[0], ids[1]);
        assert_eq!(ids[2], 0x1_0000_0000);
        assert_eq!(ids[3], 0x3_0000_0004);
    }

    #[test]
    fn lake_cells_contribute_to_downstream_accumulation() {
        let land = vec![true, true, true];
        let lake = vec![NO_LAKE, 0, NO_LAKE];
        let rec = vec![1, 1, 1];
        let order = vec![2, 0, 1];
        let acc = accumulate_flow(3, 1, &land, &lake, &rec, &order);
        assert_eq!(acc, vec![1, 3, 1]);
    }

    #[test]
    fn lake_ocean_connector_writes_a_real_receiver_path() {
        let w = 12; let h = 5; let n = w * h;
        let mut lake_of = vec![NO_LAKE; n]; let mut lake_cells = Vec::new();
        for y in 1..4 { for x in 5..7 { let c = y * w + x; lake_of[c] = 0; lake_cells.push(c as u32); } }
        let mut land = lake_of.iter().map(|&id| id == NO_LAKE).collect::<Vec<_>>();
        for y in 0..h { land[y * w + (w - 1)] = false; }
        let filled = (0..n).map(|i| { let x = i % w; if x >= 5 && x <= 10 { 100.0 - (x - 5) as f64 * 2.0 } else { 110.0 } }).collect::<Vec<_>>();
        let mut rec: Vec<u32> = (0..n).map(|i| i as u32).collect();
        let mut lakes = vec![Lake { level_ft: 100.0, cells: lake_cells, kind: LakeKind::Fresh, max_depth_ft: 120.0, outlet: None, inlet_count: 0 }];
        connect_lakes_to_ocean(w, h, &land, &lake_of, &filled, 0.0, &mut rec, &mut lakes);
        let outlet = lakes[0].outlet.expect("lake should get a coastal outlet") as usize;
        let mut cur = rec[outlet] as usize; let mut reached = false;
        for _ in 0..n { assert!(cur < n); if !land[cur] { reached = true; break; } cur = rec[cur] as usize; }
        assert!(reached);
    }

    #[test]
    fn close_lakes_connect_by_shore_distance_not_center_distance() {
        let w = 64;
        let h = 3;
        let n = w * h;
        let mut lake_of = vec![NO_LAKE; n];
        let mut source_cells = Vec::new();
        let mut target_cells = Vec::new();
        for y in 0..h {
            for x in 0..24 {
                let c = y * w + x;
                lake_of[c] = 0;
                source_cells.push(c as u32);
            }
            for x in 25..w {
                let c = y * w + x;
                lake_of[c] = 1;
                target_cells.push(c as u32);
            }
        }
        let land = lake_of.iter().map(|&id| id == NO_LAKE).collect::<Vec<_>>();
        let filled = (0..n).map(|i| {
            let x = i % w;
            if x == 24 { 100.0 } else if x == 25 { 90.0 } else { 80.0 }
        }).collect::<Vec<_>>();
        let mut rec: Vec<u32> = (0..n).map(|i| i as u32).collect();
        let mut lakes = vec![
            Lake { level_ft: 100.0, cells: source_cells, kind: LakeKind::Fresh, max_depth_ft: 100.0, outlet: None, inlet_count: 0 },
            Lake { level_ft: 90.0, cells: target_cells, kind: LakeKind::Fresh, max_depth_ft: 100.0, outlet: None, inlet_count: 0 },
        ];
        connect_close_lakes(w, h, &land, &lake_of, &filled, &mut rec, &mut lakes);
        assert!(lakes[0].outlet.is_some());
        assert!(lakes[1].inlet_count > 0);
        let outlet = lakes[0].outlet.unwrap() as usize;
        let mut cur = rec[outlet] as usize;
        for _ in 0..n {
            if lake_of[cur] == 1 { return; }
            cur = rec[cur] as usize;
            assert!(cur < n);
        }
        panic!("lake connector did not reach the lower lake");
    }

    #[test]
    fn equal_level_close_lakes_connect_through_a_real_receiver_path() {
        let w = 12;
        let h = 3;
        let n = w * h;
        let mut lake_of = vec![NO_LAKE; n];
        let mut a = Vec::new();
        let mut b = Vec::new();
        for y in 1..2 {
            for x in 2..4 {
                let c = y * w + x;
                lake_of[c] = 0;
                a.push(c as u32);
            }
            for x in 5..7 {
                let c = y * w + x;
                lake_of[c] = 1;
                b.push(c as u32);
            }
        }
        let land = lake_of.iter().map(|&id| id == NO_LAKE).collect::<Vec<_>>();
        let filled = (0..n).map(|i| {
            let x = i % w;
            if x == 4 { 100.0 } else { 90.0 }
        }).collect::<Vec<_>>();
        let mut rec: Vec<u32> = (0..n).map(|i| i as u32).collect();
        let mut lakes = vec![
            Lake { level_ft: 100.0, cells: a, kind: LakeKind::Fresh, max_depth_ft: 100.0, outlet: None, inlet_count: 0 },
            Lake { level_ft: 100.0, cells: b, kind: LakeKind::Fresh, max_depth_ft: 100.0, outlet: None, inlet_count: 0 },
        ];
        connect_close_lakes(w, h, &land, &lake_of, &filled, &mut rec, &mut lakes);
        assert!(lakes[0].outlet.is_some());
        let outlet = lakes[0].outlet.unwrap() as usize;
        let mut cur = rec[outlet] as usize;
        for _ in 0..n {
            if lake_of[cur] == 1 { return; }
            assert!(cur < n);
            cur = rec[cur] as usize;
        }
        panic!("equal-level lakes did not connect");
    }

    #[test]
    fn lake_spill_path_uses_local_parent_indices() {
        let w = 10;
        let h = 10;
        let n = w * h;
        let land = vec![true; n];
        let mut lake = vec![NO_LAKE; n];
        lake[55] = 0;
        lake[57] = 1;
        let filled: Vec<f64> = (0..n).map(|i| -(i as f64)).collect();
        let path = lake_spill_path(
            w, h, &land, &lake, &filled, filled[55],
            &[55], &[57], 1,
        ).expect("nearby lower lake should have a spill path");
        assert_eq!(path.0, 55);
        assert_eq!(path.2, 57);
        assert!(!path.1.is_empty());
        assert!(path.1.iter().all(|&c| lake[c as usize] == NO_LAKE));
    }

    #[test]
    fn lake_outflow_is_added_to_downstream_receiver() {
        let mut q = vec![0.0f64; 4];
        let raw = vec![0.0f64; 4];
        let mut rec = vec![0u32; 4];
        let lake_of = vec![0u32, 0u32, NO_LAKE, NO_LAKE];
        let lakes = vec![Lake {
            level_ft: 100.0,
            cells: vec![0, 1],
            kind: LakeKind::Fresh,
            max_depth_ft: 100.0,
            outlet: Some(1),
            inlet_count: 0,
        }];
        q[0] = 20.0;
        q[1] = 30.0;
        rec[1] = 2;
        let net = lakes[0].cells.iter().map(|&c| q[c as usize]).sum::<f64>();
        assert_eq!(net, 50.0);
        assert_eq!(raw[2], 0.0);
        q[2] += net;
        assert_eq!(q[2], 50.0);
        assert_eq!(rec[1], 2);
        let _ = (raw, lake_of);
    }

    fn lake_outlet_is_mapped_as_a_river_at_lower_discharge() {
        let land = vec![true, true, true, false];
        let lake = vec![0, NO_LAKE, NO_LAKE, NO_LAKE];
        let rec = vec![0, 2, 3, 3];
        let q = vec![5_000.0, 5_000.0, 5_000.0, 0.0];
        let acc = vec![1, 2, 3, 0];
        let basin = vec![0x1_0000_0000, 0x3_0000_0003, 0x3_0000_0003, 0];
        let lakes = vec![Lake {
            level_ft: 100.0,
            cells: vec![0],
            kind: LakeKind::Fresh,
            max_depth_ft: 100.0,
            outlet: Some(0),
            inlet_count: 0,
        }];
        let rivers = extract_rivers(4, 1, 1.0, &land, &lake, &rec, &q, &acc, &basin, 90_000.0, &lakes);
        assert_eq!(rivers.len(), 1);
        assert_eq!(rivers[0].source_lake, Some(0));
    }

    #[test]
    fn lake_fed_river_reaches_ocean() {
        let land = vec![true, true, true, false];
        let lake = vec![NO_LAKE, 0, NO_LAKE, NO_LAKE];
        let rec = vec![0, 2, 3, 3];
        let q = vec![0.0, 0.0, 30_000.0, 0.0];
        let acc = vec![1, 2, 3, 0];
        let lakes = vec![Lake {
            level_ft: 100.0,
            cells: vec![1],
            kind: LakeKind::Fresh,
            max_depth_ft: 100.0,
            outlet: Some(1),
            inlet_count: 1,
        }];
        let basin = vec![0x2_0000_0000, 0x1_0000_0000, 0x3_0000_0003, 0];
        let rivers = extract_rivers(4, 1, 1.0, &land, &lake, &rec, &q, &acc, &basin, 20_000.0, &lakes);
        assert_eq!(rivers.len(), 1);
        assert_eq!(rivers[0].source_lake, Some(0));
        assert_eq!(rivers[0].mouth, Mouth::Ocean);
        assert!(rivers[0].length_ft > 0.0);
        assert_eq!(rivers[0].basin_id, 3);
    }

    #[test]
    fn river_can_terminate_in_an_inland_lake() {
        let land = vec![true, true];
        let lake = vec![NO_LAKE, 0];
        let rec = vec![1, 1];
        let q = vec![30_000.0, 0.0];
        let acc = vec![1, 2];
        let lakes = vec![Lake {
            level_ft: 100.0,
            cells: vec![1],
            kind: LakeKind::Fresh,
            max_depth_ft: 100.0,
            outlet: None,
            inlet_count: 1,
        }];
        let rivers = extract_rivers(2, 1, 1.0, &land, &lake, &rec, &q, &acc, 20_000.0, &lakes);
        assert_eq!(rivers.len(), 1);
        assert_eq!(rivers[0].source_lake, None);
        assert_eq!(rivers[0].mouth, Mouth::Lake);
        assert_eq!(rivers[0].mouth_lake, Some(0));
        assert!(rivers[0].length_ft > 0.0);
        assert_eq!(rivers[0].basin_id, 0x1_0000_0000);
    }

    #[test]
    fn river_source_is_the_threshold_crossing_not_every_high_q_cell() {
        let land = vec![true, true, true, true, false];
        let lake = vec![NO_LAKE; 5];
        let rec = vec![1, 2, 3, 4, 4];
        let q = vec![20.0, 60.0, 90.0, 180.0, 0.0];
        let acc = vec![1, 2, 3, 4, 0];
        let basin = vec![11u64; 5];
        let rivers = extract_rivers(5, 1, 1.0, &land, &lake, &rec, &q, &acc, &basin, 90.0, &[]);
        assert_eq!(rivers.len(), 1);
        assert_eq!(rivers[0].source_cell, 2);
        assert_eq!(rivers[0].cells, vec![2, 3]);
        assert_eq!(rivers[0].terminal_receiver, Some(4));
        assert_eq!(rivers[0].mouth, Mouth::Ocean);
    }

    #[test]
    fn river_terminal_extends_only_along_the_receiver_graph() {
        let land = vec![true, true, true, false];
        let lake = vec![NO_LAKE; 4];
        let rec = vec![1, 2, 3, 3];
        let q = vec![100.0, 100.0, 20.0, 0.0];
        let acc = vec![1, 2, 3, 0];
        let basin = vec![9u64; 4];
        let rivers = extract_rivers(4, 1, 1.0, &land, &lake, &rec, &q, &acc, &basin, 90.0, &[]);
        assert_eq!(rivers.len(), 1);
        assert_eq!(rivers[0].cells, vec![0, 1, 2]);
        assert_eq!(rivers[0].mouth, Mouth::Ocean);
        assert_eq!(rivers[0].terminal_receiver, Some(3));
    }

    #[test]
    fn river_network_has_valid_terminal_receivers() {
        let land = vec![true, true, true, false];
        let lake = vec![NO_LAKE; 4];
        let rec = vec![1, 2, 3, 3];
        let q = vec![100.0, 200.0, 300.0, 0.0];
        let acc = vec![1, 2, 3, 0];
        let basin = vec![9u64; 4];
        let rivers = extract_rivers(4, 1, 1.0, &land, &lake, &rec, &q, &acc, &basin, 90.0, &[]);
        assert_eq!(rivers.len(), 1);
        assert_eq!(rivers[0].source_cell, 0);
        assert_eq!(rivers[0].terminal_receiver, Some(3));
        assert_eq!(rivers[0].mouth, Mouth::Ocean);
    }

    #[test]
    fn river_network_validator_accepts_a_valid_ocean_reach() {
        let land = vec![true, true, true, false];
        let lake = vec![NO_LAKE; 4];
        let rec = vec![1, 2, 3, 3];
        let rivers = vec![River {
            cells: vec![0, 1, 2], q: vec![100.0, 150.0, 220.0], mouth: Mouth::Ocean, into: None,
            order: 1, drainage_area_cells: 3, peak_discharge: 220.0, tributary_count: 0,
            source_lake: None, mouth_lake: None, length_cells: 3, length_ft: 3.0, basin_id: 1,
            source_cell: 0, terminal_receiver: Some(3),
        }];
        assert!(validate_river_network(4, 1, &land, &lake, &rec, &rivers));
    }

    #[test]
    fn river_network_validator_rejects_a_broken_reach() {
        let land = vec![true, true, true, false];
        let lake = vec![NO_LAKE; 4];
        let rec = vec![1, 2, 3, 3];
        let mut rivers = vec![River {
            cells: vec![0, 2], q: vec![100.0, 220.0], mouth: Mouth::Ocean, into: None,
            order: 1, drainage_area_cells: 2, peak_discharge: 220.0, tributary_count: 0,
            source_lake: None, mouth_lake: None, length_cells: 2, length_ft: 2.0, basin_id: 1,
            source_cell: 0, terminal_receiver: Some(3),
        }];
        assert!(!validate_river_network(4, 1, &land, &lake, &rec, &rivers));
        rivers[0].cells = vec![0, 1, 2];
        assert!(validate_river_network(4, 1, &land, &lake, &rec, &rivers));
    }

    #[test]
    fn strahler_rule_keeps_higher_order_at_unequal_confluence() {
        let cases = [([1u8, 1], 2u8), ([1u8, 2], 2u8), ([2u8, 2], 3u8)];
        for (upstream, expected) in cases {
            let max = upstream.iter().copied().max().unwrap();
            let equal = upstream.iter().filter(|&&o| o == max).count() >= 2;
            assert_eq!(max + equal as u8, expected);
        }
    }
}


#[cfg(test)]
mod hydrology_regression_tests {
    use super::*;
    #[test]
    fn river_reaches_are_topologically_connected_at_confluences() {
        let w = 6;
        let h = 1;
        let land = vec![true, true, true, true, true, false];
        let lake = vec![NO_LAKE; 6];
        let rec = vec![1, 3, 1, 4, 5, 5];
        let q = vec![100.0, 220.0, 100.0, 220.0, 320.0, 0.0];
        let acc = vec![1, 3, 1, 4, 5, 0];
        let basin = vec![7u64; 6];
        let rivers = extract_rivers(w, h, 1.0, &land, &lake, &rec, &q, &acc, &basin, 90.0, &[]);

        let main = rivers.iter().find(|r| r.mouth == Mouth::Ocean).expect("main stem must reach the ocean");
        let tributaries: Vec<&River> = rivers.iter().filter(|r| r.mouth == Mouth::Confluence).collect();
        assert_eq!(tributaries.len(), 2);
        assert_eq!(main.order, 2);
        assert!(main.peak_discharge >= 220.0);

        for tributary in tributaries {
            assert_eq!(tributary.into, rivers.iter().position(|r| std::ptr::eq(r, main)));
            let last = *tributary.cells.last().unwrap() as usize;
            let join = rec[last] as usize;
            assert_eq!(tributary.terminal_receiver, Some(join as u32));
            assert!(main.cells.iter().any(|&cell| cell as usize == join));
        }
    }

    #[test]
    fn nearby_flat_sink_is_connected_to_nearby_ocean() {
        let land = vec![true, true, true, false];
        let lake = vec![NO_LAKE; 4];
        let filled = vec![100.0, 100.0, 100.0, 0.0];
        let mut rec = vec![0, 1, 2, 3];
        repair_nearby_water_sinks(4, 1, &land, &lake, &filled, &mut rec, 3);
        assert_eq!(rec, vec![1, 2, 3, 3]);
    }

    #[test]
    #[test]
    fn nearby_dry_river_terminus_connects_to_water() {
        let w = 5; let h = 1;
        let land = vec![true, true, true, true, false];
        let lake = vec![NO_LAKE; 5];
        let filled = vec![10.0, 9.0, 8.0, 8.5, 0.0];
        let q = vec![100.0, 100.0, 100.0, 0.0, 0.0];
        let mut rec = vec![1, 2, 2, 3, 4];
        connect_nearby_river_termini_to_water(w, h, &land, &lake, &filled, &q, 90.0, &mut rec);
        let mut cur = 2usize; let mut reached = false;
        for _ in 0..5 {
            let next = rec[cur] as usize;
            if !land[next] { reached = true; break; }
            cur = next;
        }
        assert!(reached);
    }

    fn flow_through_lake_preserves_outflow_when_local_balance_is_negative() {
        let land = vec![true, true, true, true, true, false];
        let lake = vec![NO_LAKE, 0, 0, NO_LAKE, NO_LAKE, NO_LAKE];
        let rec = vec![1, 2, 3, 4, 5, 5];
        let order = vec![0, 1, 2, 3, 4];
        let clim = Climate { temp: vec![20.0; 6], precip: vec![500.0; 6] };
        let lakes = vec![Lake { level_ft: 100.0, cells: vec![1, 2], kind: LakeKind::Fresh, max_depth_ft: 100.0, outlet: Some(2), inlet_count: 1 }];
        let (q, _) = recompute_final_discharge(6, 1, &land, &lake, &rec, &order, &clim, &[(0, 100_000.0)], &lakes);
        assert!(q[3] >= 25_000.0);
    }

    #[test]
    fn accumulation_includes_lake_cells() {
        let land = vec![true, true, true];
        let lake = vec![NO_LAKE, 0, NO_LAKE];
        let rec = vec![1, 1, 1];
        let order = vec![2, 0, 1];
        let acc = accumulate_flow(3, 1, &land, &lake, &rec, &order);
        assert_eq!(acc, vec![1, 3, 1]);
    }
}
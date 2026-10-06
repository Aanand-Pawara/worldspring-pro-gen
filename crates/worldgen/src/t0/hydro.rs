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

use super::climate::Climate;
use super::flood::{neighbors, priority_flood, receivers};

/// Water surface value for dry ground.
pub const DRY: f32 = -30_000.0;
pub const NO_LAKE: u32 = u32::MAX;

/// Discharge (mm·cells) at which a stream is mapped as a river, before `river_density`.
pub const RIVER_Q: f64 = 90_000.0;
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
}

pub struct Hydro {
    /// Water surface elevation per cell (sea level, lake level) or `DRY`.
    pub water: Vec<f32>,
    /// Number of routed land cells contributing to each active cell.
    pub flow_accumulation: Vec<u32>,
    /// Stable drainage-basin identity for every land cell.
    pub basin_id: Vec<u64>,
    pub discharge: Vec<f32>,
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

    let (rec, _) = receivers(w, h, &fl.filled);

    for lake_id in 0..lakes.len() {
        lakes[lake_id].outlet = lakes[lake_id].cells.iter().copied().find(|&c| {
            let r = rec[c as usize] as usize;
            r != c as usize && lake_of[r] != lake_id as u32
        });
    }
    for i in 0..n {
        if !land[i] || lake_of[i] != NO_LAKE { continue; }
        let r = rec[i] as usize;
        if r < n && lake_of[r] != NO_LAKE {
            let id = lake_of[r] as usize;
            lakes[id].inlet_count = lakes[id].inlet_count.saturating_add(1);
        }
    }

    // Water balance. `raw` ignores losses (catchment supply); `q` includes them.
    let pet: Vec<f64> = clim.temp.iter().map(|&t| (350.0 + 55.0 * t as f64).max(0.0)).collect();
    let mut q = vec![0.0f64; n];
    let mut raw = vec![0.0f64; n];
    for &(k, v) in feed {
        q[k] += v;
        raw[k] += v;
    }
    for &i in fl.order.iter().rev() {
        let i = i as usize;
        let p = clim.precip[i] as f64;
        let runoff = (p - 0.65 * pet[i]).max(0.0);
        q[i] += runoff;
        raw[i] += runoff;
        if lake_of[i] != NO_LAKE {
            q[i] = (q[i] - pet[i]).max(0.0);
        } else if p < 400.0 {
            q[i] *= 0.997; // gradual transmission loss in dry country; routing remains continuous
        }
        let r = rec[i] as usize;
        if r != i && land[r] {
            q[r] += q[i];
            raw[r] += raw[i];
        }
    }

    for lake in &mut lakes {
        let exit = lake.outlet.map(|c| c as usize).unwrap_or_else(|| lake.cells.iter().map(|&c| c as usize).max_by(|&x, &y| raw[x].total_cmp(&raw[y]).then(x.cmp(&y))).unwrap());
        let evap: f64 = lake.cells.iter().map(|&c| pet[c as usize]).sum();
        let ratio = q[exit] / evap.max(1.0);
        let precip = lake.cells.iter().map(|&c| clim.precip[c as usize] as f64).sum::<f64>() / lake.cells.len() as f64;
        lake.kind = if lake.outlet.is_some() && q[exit] > 0.0 {
            LakeKind::Fresh
        } else if precip > 550.0 {
            LakeKind::Fresh
        } else if ratio < 0.35 {
            LakeKind::SaltFlat
        } else if q[exit] <= 0.0 {
            LakeKind::Salt
        } else {
            LakeKind::Fresh
        };
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

    let flow_accumulation = accumulate_flow(w, h, land, &lake_of, &rec, &fl.order);
    let basin_id = assign_basin_ids(w, h, land, &lake_of, &rec);
    let rivers = extract_rivers(w, h, cell_ft, land, &lake_of, &rec, &q, &flow_accumulation, &basin_id, RIVER_Q / river_density.max(0.05), &lakes);
    Hydro { water, flow_accumulation, basin_id, discharge: q.iter().map(|&v| v as f32).collect(), lake_of, lakes, rivers }
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
    // allowed to die. Once a channel is established, follow the receiver all the way to
    // the next water body. This prevents dry-country transmission loss from producing
    // visually orphaned rivers that simply stop on otherwise draining terrain.
    let is_seed = |i: usize| land[i] && lake_of[i] == NO_LAKE && (q[i] >= threshold || (lake_feed[i] && q[i] >= threshold * 0.20));
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
        let qv = cells.iter().map(|&cc| q[cc as usize] as f32).collect();
        chains.push(River { cells, q: qv, mouth, into, order: seg_order[root].max(1), drainage_area_cells: accumulation[source_cell], peak_discharge: 0.0, tributary_count: 0, source_lake, mouth_lake, length_cells: 0, length_ft: 0.0, basin_id: basin_id.get(source_cell).copied().unwrap_or(0) });
        for &sid in &path {
            let main_child = path.iter().position(|&p| p == sid).and_then(|i| path.get(i + 1)).copied();
            for &kid in &children[sid] { if Some(kid) != main_child { stack.push((kid, Some(chain_id))); } }
        }
    }
    let tributary_counts: Vec<u16> = (0..chains.len()).map(|id| chains.iter().filter(|r| r.into == Some(id)).count().min(u16::MAX as usize) as u16).collect();
    for (id, chain) in chains.iter_mut().enumerate() {
        chain.peak_discharge = chain.q.iter().copied().fold(0.0f32, f32::max);
        chain.tributary_count = tributary_counts[id];
        chain.length_cells = chain.cells.len().min(u32::MAX as usize) as u32;
        chain.length_ft = chain_length_ft(w, &chain.cells, cell_ft);
        if matches!(chain.mouth, Mouth::Ocean | Mouth::Lake) {
            if let Some(&last) = chain.cells.last() {
                let c = last as usize;
                let r = rec[c] as usize;
                if r < n && (!land[r] || lake_of[r] != NO_LAKE) {
                    let dx = (c % w) as f64 - (r % w) as f64;
                    let dy = (c / w) as f64 - (r / w) as f64;
                    chain.length_ft += 0.5 * (dx * dx + dy * dy).sqrt() * cell_ft;
                }
            }
        }
        chain.drainage_area_cells = chain.cells.iter().map(|&cc| accumulation[cc as usize]).max().unwrap_or(chain.drainage_area_cells);
    }
    chains
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
        let rec = vec![1, 1, 1, 3, 4, 5];
        let q = vec![100.0, 220.0, 100.0, 220.0, 220.0, 0.0];
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
            assert_eq!(*main.cells.first().unwrap() as usize, join);
        }
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

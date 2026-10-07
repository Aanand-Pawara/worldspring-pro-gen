//! Deterministic political geography built from generated settlements.
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use serde::Serialize;
use super::flood::neighbors;
use super::hydro::{self, Hydro};
use super::names::{NameKind, Namer};
use super::settle::{Settlement, Tier};
use crate::World;

#[derive(Clone, Debug, Serialize)]
pub struct Kingdom { pub id: u16, pub name: String, pub capital: usize, pub population: u64, pub area_cells: u32, pub cities: u32, pub towns: u32, pub villages: u32 }
#[derive(Clone, Debug, Serialize)]
pub struct BorderSegment { pub kingdom: u16, pub other: u16, pub a: [f64; 2], pub b: [f64; 2] }
#[derive(Clone, Debug, Serialize)]
pub struct Politics { pub kingdoms: Vec<Kingdom>, pub kingdom_of: Vec<u16>, pub borders: Vec<BorderSegment> }

#[derive(Clone, Copy, Debug)]
struct Frontier { cost: u64, kingdom: u16, cell: usize }
impl PartialEq for Frontier { fn eq(&self, other: &Self) -> bool { self.cost == other.cost && self.kingdom == other.kingdom && self.cell == other.cell } }
impl Eq for Frontier {}
impl Ord for Frontier {
    fn cmp(&self, other: &Self) -> Ordering {
        other.cost.cmp(&self.cost).then_with(|| other.kingdom.cmp(&self.kingdom)).then_with(|| other.cell.cmp(&self.cell))
    }
}
impl PartialOrd for Frontier { fn partial_cmp(&self, other: &Self) -> Option<Ordering> { Some(self.cmp(other)) } }

fn edge_cost(a: usize, b: usize, w: usize, h: usize, cell_ft: f64, height: &[f64], hydro: &Hydro) -> u64 {
    // Quantize floating inputs before they enter the priority queue. This keeps political
    // territory decisions bit-identical between native and WASM floating-point backends.
    const SCALE: u64 = 1_000_000;
    // Frontiers are simulated as movement costs, not straight geometric partitions. Flat,
    // fertile-looking corridors are cheap; steep terrain and major waterways are expensive.
    let slope_m = (((height[a] - height[b]).abs() / cell_ft.max(1.0)).min(4.0) * SCALE as f64).round() as u64;
    let slope_term = (slope_m * 5).min(3 * SCALE);
    let mut cost = SCALE + 11 * slope_term * slope_term / SCALE;

    // Rivers are especially attractive as borders because crossing them is costly while moving
    // along the same river corridor is comparatively cheap.
    let river_a = hydro.discharge.get(a).copied().unwrap_or(0.0) as f64 > hydro::RIVER_Q * 0.20;
    let river_b = hydro.discharge.get(b).copied().unwrap_or(0.0) as f64 > hydro::RIVER_Q * 0.20;
    if river_a || river_b {
        cost += 52 * SCALE;
        if river_a && river_b { cost += 14 * SCALE; }
    }

    // Lakes are stronger barriers than rivers. Their shorelines naturally become political
    // frontiers without requiring a special "draw a lake border" rule.
    let lake_a = hydro.lake_of.get(a).copied().unwrap_or(hydro::NO_LAKE) != hydro::NO_LAKE;
    let lake_b = hydro.lake_of.get(b).copied().unwrap_or(hydro::NO_LAKE) != hydro::NO_LAKE;
    if lake_a || lake_b { cost += 110 * SCALE; }

    // Major waterways tend to become durable political frontiers. Treat strong drainage cells
    // as expensive to cross so borders naturally settle along rivers instead of cutting across
    // them. The underlying terrain still dominates, so this is a tendency, not a hard wall.
    let qa = hydro.discharge.get(a).copied().unwrap_or(0.0) as f64;
    let qb = hydro.discharge.get(b).copied().unwrap_or(0.0) as f64;
    let river_ratio = ((qa.max(qb)) / hydro::RIVER_Q).clamp(0.0, 1.0);
    let river_barrier = (crate::core::sqrt(river_ratio) * SCALE as f64).round() as u64;
    cost += 95 * river_barrier;

    // A cell that stands well above its neighbours behaves like a ridge. This makes mountain
    // chains and escarpments hard to cross while still allowing low saddles/passes to remain
    // usable, which is much closer to how historical frontiers tend to form.
    let mut ridge: f64 = 0.0;
    for k in [a, b] {
        for (nb, _) in neighbors(w, h, k) {
            ridge = ridge.max((height[k] - height[nb]).max(0.0) / cell_ft.max(1.0));
        }
    }
    let ridge = ridge.min(2.5);
    let ridge_m = (ridge * SCALE as f64).round() as u64;
    cost += 20 * ridge_m * ridge_m / SCALE;
    cost
}

pub fn assign(world: &World, w: usize, h: usize, cell_ft: f64, land: &[bool], height: &[f64], hydro: &Hydro, settlements: &mut [Settlement], enabled: bool) -> Politics {
    let n = w * h;
    if !enabled {
        for s in settlements { s.capital = false; s.kingdom_id = 0; }
        return Politics { kingdoms: Vec::new(), kingdom_of: vec![u16::MAX; n], borders: Vec::new() };
    }

    let mut capitals: Vec<usize> = settlements.iter().enumerate().filter_map(|(i,s)|(s.tier == Tier::Metropolis).then_some(i)).collect();
    capitals.sort_by(|&a,&b| settlements[b].population.cmp(&settlements[a].population).then(a.cmp(&b)));
    if capitals.is_empty() { if let Some((i,_)) = settlements.iter().enumerate().max_by_key(|(_,s)| s.population) { capitals.push(i); } }
    if capitals.is_empty() { return Politics { kingdoms: Vec::new(), kingdom_of: vec![u16::MAX; n], borders: Vec::new() }; }

    let mut comp = vec![u32::MAX; n];
    let mut next = 0u32;
    for start in 0..n {
        if !land[start] || comp[start] != u32::MAX { continue; }
        let mut q = vec![start]; comp[start] = next; let mut head = 0;
        while head < q.len() {
            let k = q[head]; head += 1;
            for (nb, _) in neighbors(w, h, k) {
                if land[nb] && comp[nb] == u32::MAX { comp[nb] = next; q.push(nb); }
            }
        }
        next += 1;
    }

    let cap_comp: Vec<u32> = capitals.iter().map(|&i| comp[settlements[i].cell]).collect();
    let mut has_cap = vec![false; next as usize];
    for &cc in &cap_comp { if cc != u32::MAX { has_cap[cc as usize] = true; } }

    // Political territory uses a weighted multi-source flood instead of a Euclidean Voronoi.
    // Terrain, drainage barriers, and mountain passes shape the frontier, while capitals provide
    // the human centre of gravity. This is intentionally a deterministic approximation of
    // historical territorial expansion, not a mathematically straight partition.
    let inf = u64::MAX;
    let mut dist = vec![inf; n];
    let mut kingdom_of = vec![u16::MAX; n];
    let mut heap = BinaryHeap::new();
    for (ki, &si) in capitals.iter().enumerate() {
        let cell = settlements[si].cell;
        dist[cell] = 0;
        kingdom_of[cell] = ki as u16;
        heap.push(Frontier { cost: 0, kingdom: ki as u16, cell });
    }
    while let Some(cur) = heap.pop() {
        if cur.cost > dist[cur.cell] || kingdom_of[cur.cell] != cur.kingdom { continue; }
        let cc = comp[cur.cell];
        for (nb, _) in neighbors(w, h, cur.cell) {
            if !land[nb] || comp[nb] != cc { continue; }
            let nd = cur.cost + edge_cost(cur.cell, nb, w, h, cell_ft, height, hydro);
            let better = nd < dist[nb] || nd == dist[nb] && cur.kingdom < kingdom_of[nb];
            if better {
                dist[nb] = nd;
                kingdom_of[nb] = cur.kingdom;
                heap.push(Frontier { cost: nd, kingdom: cur.kingdom, cell: nb });
            }
        }
    }

    // Small islands without a capital inherit the nearest capital deterministically.
    for cell in 0..n {
        if !land[cell] || has_cap.get(comp[cell] as usize).copied().unwrap_or(false) { continue; }
        let mut best: Option<(f64, u16)> = None;
        for (ki, &si) in capitals.iter().enumerate() {
            let a = settlements[si].cell;
            let dx = (a % w) as f64 - (cell % w) as f64;
            let dy = (a / w) as f64 - (cell / w) as f64;
            let d = dx * dx + dy * dy;
            if best.is_none_or(|(bd, bk)| d < bd || (d == bd && (ki as u16) < bk)) { best = Some((d, ki as u16)); }
        }
        if let Some((_, k)) = best { kingdom_of[cell] = k; }
    }

    let mut namer = Namer::new(world.stream("t0.kingdom.names"));
    let mut kingdoms = Vec::with_capacity(capitals.len());
    for (ki, &cap) in capitals.iter().enumerate() {
        let s = &settlements[cap];
        let culture = ((s.cell as u64).wrapping_mul(0x9e3779b97f4a7c15) as usize) % 5;
        kingdoms.push(Kingdom { id: ki as u16, name: namer.name(NameKind::Kingdom, culture), capital: cap, population: 0, area_cells: 0, cities: 0, towns: 0, villages: 0 });
    }

    for s in settlements.iter_mut() {
        s.capital = false;
        s.kingdom_id = kingdom_of[s.cell];
        if s.kingdom_id == u16::MAX { s.kingdom_id = 0; }
    }
    for k in &kingdoms { if let Some(s) = settlements.get_mut(k.capital) { s.capital = true; s.kingdom_id = k.id; } }
    for cell in 0..n { let id = kingdom_of[cell]; if id != u16::MAX { kingdoms[id as usize].area_cells += 1; } }
    for s in settlements.iter() {
        if let Some(k) = kingdoms.get_mut(s.kingdom_id as usize) {
            k.population += s.population as u64;
            match s.tier { Tier::Metropolis | Tier::City => k.cities += 1, Tier::Town => k.towns += 1, Tier::Village => k.villages += 1 }
        }
    }

    let mut borders = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let k = y * w + x;
            let a = kingdom_of[k];
            if a == u16::MAX { continue; }
            if x + 1 < w {
                let b = kingdom_of[k + 1];
                if b != u16::MAX && b != a { borders.push(BorderSegment { kingdom: a, other: b, a: [(x + 1) as f64, y as f64], b: [(x + 1) as f64, (y + 1) as f64] }); }
            }
            if y + 1 < h {
                let b = kingdom_of[k + w];
                if b != u16::MAX && b != a { borders.push(BorderSegment { kingdom: a, other: b, a: [x as f64, (y + 1) as f64], b: [(x + 1) as f64, (y + 1) as f64] }); }
            }
        }
    }

    Politics { kingdoms, kingdom_of, borders }
}

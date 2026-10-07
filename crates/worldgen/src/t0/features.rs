//! Named features (the T0 part of the gazetteer): continent, islands, oceans and seas, bays,
//! mountain ranges, peaks (by topographic prominence), passes (their key cols), volcanoes,
//! lakes, rivers and biome regions. Each gets a stable id, a name, and label placement
//! (anchor at the pole of inaccessibility, angle along the principal axis, size in feet).

use serde::Serialize;
use std::cmp::Reverse;
use std::collections::BinaryHeap;

use super::biome::Biome;
use super::climate::{Climate, distance_to};
use super::flood::neighbors;
use super::hydro::{Hydro, LakeKind, Mouth};
use super::names::{NameKind, Namer};
use super::settle::{Poi, PoiKind, Settlement, Tier};
use super::volcano::{Activity, Volcano, VolcanoKind};
use crate::World;
use crate::core::rng::{Pcg32, hash2};

#[derive(Clone, Debug, Serialize)]
pub struct Feature {
    pub id: String,
    pub kind: &'static str,
    pub name: String,
    /// Label anchor, ft.
    pub x: f64,
    pub y: f64,
    /// Label angle, radians (0 = horizontal, clockwise positive since y points down).
    pub angle: f64,
    /// Characteristic size, ft: decides the zoom band where the label shows.
    pub extent_ft: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub elev_ft: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Fine selection path for rivers only: x, y in ft and local channel width in ft.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub river_path: Option<Vec<[f64; 3]>>,
    /// World-space distributary centerlines used to render delta mouths.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delta_paths: Option<Vec<Vec<[f64; 2]>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream_order: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub drainage_area_mi2: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discharge_index: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tributary_count: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length_mi: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub basin_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub river_mouth: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_lake_id: Option<u32>,
    /// Source/headwater cell in world feet. This is the actual birth point of the river chain.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub river_source_x: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub river_source_y: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mouth_lake_id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub area_mi2: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_depth_ft: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inlet_count: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_outlet: Option<bool>,
    pub kingdom_id: Option<u16>,
    pub kingdom_name: Option<String>,
    pub political_rank: Option<&'static str>,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct Overlay {
    pub features: Vec<Feature>,
    pub kingdoms: Vec<super::politics::Kingdom>,
    pub kingdom_borders: Vec<super::politics::BorderSegment>,
    pub kingdom_cells: Vec<u16>,
    /// Naming culture of each settlement (same order as the settlements).
    #[serde(skip)]
    pub settlement_cultures: Vec<u8>,
    /// What the sketch asked for that the world could not follow exactly.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub conflicts: Vec<super::sketch::Conflict>,
}

pub struct Inputs<'a> {
    pub world: &'a World,
    pub w: usize,
    pub h: usize,
    pub cell_ft: f64,
    pub height: &'a [f64],
    pub land: &'a [bool],
    pub biome: &'a [u32],
    pub clim: &'a Climate,
    pub hydro: &'a Hydro,
    pub volcanoes: &'a [Volcano],
    pub settlements: &'a [Settlement],
    pub pois: &'a [Poi],
    pub politics: &'a super::politics::Politics,
}

struct Builder<'a> {
    inp: &'a Inputs<'a>,
    namer: Namer,
    cultures: Vec<(f64, f64, usize)>,
    out: Overlay,
    ids: std::collections::BTreeSet<String>,
}

pub fn extract(inp: &Inputs) -> Overlay {
    let mut b = Builder {
        inp,
        namer: Namer::new(inp.world.stream("t0.names")),
        cultures: culture_seeds(inp),
        out: Overlay::default(),
        ids: Default::default(),
    };
    b.landmasses();
    b.oceans();
    b.bays();
    b.ranges_peaks_passes();
    b.volcanoes();
    b.lakes();
    b.rivers();
    b.regions();
    b.sites();
    b.out.kingdoms = inp.politics.kingdoms.clone();
    b.out.kingdom_borders = inp.politics.borders.clone();
    b.out.kingdom_cells = inp.politics.kingdom_of.clone();
    b.out
}

fn culture_seeds(inp: &Inputs) -> Vec<(f64, f64, usize)> {
    let mut rng = Pcg32::new(inp.world.stream("t0.cultures"), 5);
    let land: Vec<usize> = (0..inp.w * inp.h).filter(|&k| inp.land[k]).collect();
    if land.is_empty() {
        return vec![(0.0, 0.0, 1)];
    }
    (0..6)
        .map(|_| {
            let k = land[rng.below(land.len() as u32) as usize];
            let (t, p) = (inp.clim.temp[k], inp.clim.precip[k]);
            let c = if t < 2.0 {
                0
            } else if t > 15.0 && p < 400.0 {
                3
            } else if p < 650.0 {
                4
            } else if t > 20.0 {
                2
            } else {
                1
            };
            ((k % inp.w) as f64, (k / inp.w) as f64, c)
        })
        .collect()
}

impl Builder<'_> {
    fn culture_at(&self, cx: f64, cy: f64) -> usize {
        self.cultures
            .iter()
            .min_by(|a, b| {
                let da = (a.0 - cx) * (a.0 - cx) + (a.1 - cy) * (a.1 - cy);
                let db = (b.0 - cx) * (b.0 - cx) + (b.1 - cy) * (b.1 - cy);
                da.total_cmp(&db)
            })
            .map(|c| c.2)
            .unwrap_or(1)
    }

    #[allow(clippy::too_many_arguments)]
    fn push(&mut self, kind: &'static str, nk: NameKind, cx: f64, cy: f64, angle: f64, extent_ft: f64, elev_ft: Option<f64>, detail: Option<String>) -> String {
        let culture = self.culture_at(cx, cy);
        let name = self.namer.name(nk, culture);
        let mut id = format!("{kind}:{:x}", hash2(self.inp.world.seed, (cx / 8.0) as i64, (cy / 8.0) as i64) & 0xffff_ffff);
        while !self.ids.insert(id.clone()) {
            id.push('b');
        }
        let c = self.inp.cell_ft;
        self.out.features.push(Feature { id: id.clone(), kind, name, x: cx * c, y: cy * c, angle, extent_ft, elev_ft, detail, river_path: None, delta_paths: None, stream_order: None, drainage_area_mi2: None, discharge_index: None, tributary_count: None, length_mi: None, basin_id: None, river_mouth: None, source_lake_id: None, river_source_x: None, river_source_y: None, mouth_lake_id: None, area_mi2: None, max_depth_ft: None, inlet_count: None, has_outlet: None, kingdom_id: None, kingdom_name: None, political_rank: None });
        id
    }

    fn above_sea(&self, k: usize) -> f64 {
        self.inp.height[k] - self.inp.world.params().sea_level_ft
    }

    fn landmasses(&mut self) {
        let inp = self.inp;
        let comps = components(inp.w, inp.h, |k| inp.land[k], true);
        let inside = distance_to(inp.w, inp.h, &inp.land.iter().map(|l| !l).collect::<Vec<_>>());
        let largest = comps.iter().map(Vec::len).max().unwrap_or(0);
        for comp in &comps {
            if comp.len() < 6 {
                continue;
            }
            let (ax, ay) = pole(inp.w, comp, &inside);
            let (angle, _) = principal_axis(inp.w, comp, 0.35);
            let extent = (comp.len() as f64).sqrt() * inp.cell_ft;
            if comp.len() == largest {
                self.push("continent", NameKind::Continent, ax, ay, angle, extent, None, None);
            } else {
                self.push("island", NameKind::Island, ax, ay, angle, extent, None, None);
            }
        }
    }

    fn oceans(&mut self) {
        let inp = self.inp;
        let (w, h) = (inp.w, inp.h);
        let comps = components(w, h, |k| !inp.land[k], false);
        let target: Vec<bool> = (0..w * h)
            .map(|k| inp.land[k] || k % w == 0 || k / w == 0 || k % w == w - 1 || k / w == h - 1)
            .collect();
        let dist = distance_to(w, h, &target);
        for comp in &comps {
            let border = comp.iter().any(|&k| {
                let k = k as usize;
                k % w == 0 || k / w == 0 || k % w == w - 1 || k / w == h - 1
            });
            if !border && comp.len() < 300 {
                continue;
            }
            let (ax, ay) = pole(w, comp, &dist);
            let extent = (comp.len() as f64).sqrt() * inp.cell_ft;
            let (kind, nk) = if border { ("ocean", NameKind::Ocean) } else { ("sea", NameKind::Sea) };
            self.push(kind, nk, ax, ay, 0.0, extent, None, None);
        }
    }

    /// Bays: stretches of sea mostly enclosed by land (ray casting on a coarse grid).
    fn bays(&mut self) {
        let inp = self.inp;
        let (w, h) = (inp.w, inp.h);
        const STEP: usize = 4;
        let (cw, ch) = (w / STEP, h / STEP);
        let rays: Vec<(f64, f64)> =
            (0..16).map(|r| (libm::cos(r as f64 * std::f64::consts::TAU / 16.0), libm::sin(r as f64 * std::f64::consts::TAU / 16.0))).collect();
        let mut enclosed = vec![false; cw * ch];
        for cj in 0..ch {
            for ci in 0..cw {
                let (i, j) = (ci * STEP, cj * STEP);
                if inp.land[j * w + i] {
                    continue;
                }
                let mut hits = 0;
                for &(dx, dy) in &rays {
                    for s in 1..50 {
                        let (x, y) = (i as f64 + dx * s as f64, j as f64 + dy * s as f64);
                        if x < 0.0 || y < 0.0 || x >= w as f64 || y >= h as f64 {
                            break;
                        }
                        if inp.land[y as usize * w + x as usize] {
                            hits += 1;
                            break;
                        }
                    }
                }
                enclosed[cj * cw + ci] = hits >= 11;
            }
        }
        for comp in components(cw, ch, |k| enclosed[k], false) {
            if comp.len() < 4 {
                continue;
            }
            let (sx, sy) = comp.iter().fold((0.0, 0.0), |a, &k| (a.0 + (k as usize % cw) as f64, a.1 + (k as usize / cw) as f64));
            let (cx, cy) = (sx / comp.len() as f64 * STEP as f64, sy / comp.len() as f64 * STEP as f64);
            let extent = (comp.len() as f64).sqrt() * STEP as f64 * inp.cell_ft;
            self.push("bay", NameKind::Bay, cx, cy, 0.0, extent, None, None);
        }
    }

    fn ranges_peaks_passes(&mut self) {
        let inp = self.inp;
        let (w, h) = (inp.w, inp.h);
        let p = inp.world.params();
        let mountain = |k: usize| inp.land[k] && self.above_sea(k) >= (0.28 * p.max_elev_ft).max(2500.0);
        let mask: Vec<bool> = (0..w * h).map(mountain).collect();
        let inside = distance_to(w, h, &mask.iter().map(|m| !m).collect::<Vec<_>>());
        for comp in components(w, h, |k| mask[k], true) {
            if comp.len() < 25 {
                continue;
            }
            let (ax, ay) = pole(w, &comp, &inside);
            let (angle, len) = principal_axis(w, &comp, 1.2);
            self.push("range", NameKind::Range, ax, ay, angle, len * inp.cell_ft, None, None);
        }

        // Topographic prominence by union-find over land cells, highest first.
        let mut order: Vec<usize> = (0..w * h).filter(|&k| inp.land[k]).collect();
        order.sort_by(|&a, &b| inp.height[b].total_cmp(&inp.height[a]).then(a.cmp(&b)));
        let mut parent: Vec<u32> = vec![u32::MAX; w * h];
        let mut summit: Vec<u32> = vec![u32::MAX; w * h];
        fn find(parent: &mut [u32], mut x: usize) -> usize {
            while parent[x] as usize != x {
                let g = parent[parent[x] as usize];
                parent[x] = g;
                x = g as usize;
            }
            x
        }
        let mut peaks: Vec<(f64, usize, usize)> = Vec::new(); // (prominence, summit, col)
        for &c in &order {
            let mut roots: Vec<usize> = Vec::new();
            for (nb, _) in neighbors(w, h, c) {
                if parent[nb] != u32::MAX {
                    let r = find(&mut parent, nb);
                    if !roots.contains(&r) {
                        roots.push(r);
                    }
                }
            }
            parent[c] = c as u32;
            if roots.is_empty() {
                summit[c] = c as u32;
                continue;
            }
            roots.sort_by(|&a, &b| {
                inp.height[summit[b] as usize].total_cmp(&inp.height[summit[a] as usize]).then(a.cmp(&b))
            });
            let main = roots[0];
            for &r in &roots[1..] {
                let s = summit[r] as usize;
                peaks.push((inp.height[s] - inp.height[c], s, c));
                parent[r] = main as u32;
            }
            parent[c] = main as u32;
        }
        // Each landmass's highest point: prominence is its height above the sea.
        for &c in &order {
            if find(&mut parent, c) == c {
                let s = summit[c] as usize;
                peaks.push((self.above_sea(s), s, s));
            }
        }
        let min_prom = (0.1 * p.max_elev_ft).max(1500.0);
        peaks.retain(|&(prom, s, _)| prom >= min_prom && self.above_sea(s) >= 3000.0);
        peaks.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));
        let near_volcano = |k: usize| {
            inp.volcanoes.iter().any(|v| {
                let (dx, dy) = ((k % w) as f64 - v.cx, (k / w) as f64 - v.cy);
                dx * dx + dy * dy < 9.0
            })
        };
        let mut passes: Vec<(f64, f64)> = Vec::new();
        let pass_sep = 25.0 * 5280.0 / inp.cell_ft;
        for &(prom, s, col) in peaks.iter().take(60) {
            if near_volcano(s) {
                continue;
            }
            let elev = self.above_sea(s);
            let (sx, sy) = ((s % w) as f64, (s / w) as f64);
            let extent = (prom * 12.0).max(8.0 * inp.cell_ft);
            self.push("peak", NameKind::Peak, sx, sy, 0.0, extent, Some(elev.round()), Some(format!("{} ft", fmt_thousands(elev))));

            let col_elev = self.above_sea(col);
            let (cx, cy) = ((col % w) as f64, (col / w) as f64);
            if col != s
                && passes.len() < 25
                && col_elev >= 1500.0
                && passes.iter().all(|&(px, py)| (px - cx) * (px - cx) + (py - cy) * (py - cy) >= pass_sep * pass_sep)
            {
                passes.push((cx, cy));
                self.push("pass", NameKind::Pass, cx, cy, 0.0, extent * 0.6, Some(col_elev.round()), Some(format!("{} ft", fmt_thousands(col_elev))));
            }
        }
    }

    /// Settlements (by tier) and points of interest. Settlement ids are stable per cell so
    /// later layout jobs and agents can refer to them.
    fn sites(&mut self) {
        let inp = self.inp;
        let c = inp.cell_ft;
        for s in inp.settlements {
            let (cx, cy) = (s.x / c, s.y / c);
            let k = s.cell;
            let kind = match s.tier {
                Tier::Metropolis => "metropolis",
                Tier::City => "city",
                Tier::Town => "town",
                Tier::Village => "village",
            };
            // Label extent: roughly the settlement footprint scaled up so it shows at the
            // zoom where it matters on the map.
            let extent = match s.tier {
                Tier::Metropolis => 60.0,
                Tier::City => 30.0,
                Tier::Town => 12.0,
                Tier::Village => 5.0,
            } * 5280.0;
            let kingdom = inp.politics.kingdoms.get(s.kingdom_id as usize);
            let rank = if s.capital { "capital" } else { match s.tier { Tier::Metropolis | Tier::City => "city", Tier::Town => "town", Tier::Village => "village" } };
            let mut detail = format!("{} {}, pop. {}", s.kind.name(), s.tier.name(), fmt_thousands(s.population as f64));
            if let Some(k) = kingdom { detail.push_str(&format!(", {} of {}", rank, k.name)); }
            if s.capital {
                detail.push_str(", capital");
            }
            let elev = self.above_sea(k);
            let culture = self.culture_at(cx, cy) as u8;
            self.out.settlement_cultures.push(culture);
            self.push(kind, NameKind::Settlement, cx, cy, 0.0, extent, Some(elev.round()), Some(detail));
            // A pinned settlement keeps the name drawn with it.
            let pinned = s.pin.and_then(|i| inp.world.file.sketch.strokes.get(i as usize)).and_then(|st| st.name.as_deref()).map(str::trim).filter(|n| !n.is_empty());
            if let Some(f) = self.out.features.last_mut() { f.kingdom_id=Some(s.kingdom_id); f.kingdom_name=kingdom.map(|k|k.name.clone()); f.political_rank=Some(rank); }
            if let Some(name) = pinned {
                self.out.features.last_mut().expect("just pushed").name = name.to_string();
            }
        }
        for p in inp.pois {
            let (cx, cy) = (p.x / c, p.y / c);
            let (kind, nk, extent) = match p.kind {
                PoiKind::Ruin => ("ruin", NameKind::Ruin, 3.0),
                PoiKind::Tower => ("tower", NameKind::Tower, 3.0),
                PoiKind::Camp => ("camp", NameKind::Camp, 2.0),
                PoiKind::Waystation => ("waystation", NameKind::Settlement, 2.0),
                PoiKind::Cave => ("cave", NameKind::Cave, 2.0),
                PoiKind::Mine => ("mine", NameKind::Mine, 2.0),
                PoiKind::LavaTube => ("lava_tube", NameKind::LavaTube, 2.0),
                PoiKind::Entrance => ("entrance", NameKind::Ruin, 2.0),
                PoiKind::Building => ("building", NameKind::Settlement, 0.5),
            };
            let k = (cy.round() as usize).min(inp.h - 1) * inp.w + (cx.round() as usize).min(inp.w - 1);
            let elev = self.above_sea(k);
            let id = self.push(kind, nk, cx, cy, 0.0, extent * 5280.0, Some(elev.round()), None);
            if p.kind == PoiKind::Waystation {
                let f = self.out.features.iter_mut().rev().find(|f| f.id == id).unwrap();
                f.name = format!("{} Inn", f.name);
                f.detail = Some("roadside inn".into());
            }
        }
    }

    fn volcanoes(&mut self) {
        let vs: Vec<Volcano> = self.inp.volcanoes.to_vec();
        for v in vs {
            let k = v.cy as usize * self.inp.w + v.cx as usize;
            let elev = self.above_sea(k);
            let kind = match v.kind {
                VolcanoKind::Stratovolcano => "stratovolcano",
                VolcanoKind::Shield => "shield volcano",
                VolcanoKind::CinderCone => "cinder cone",
            };
            let act = match v.activity {
                Activity::Active => "active",
                Activity::Dormant => "dormant",
                Activity::Extinct => "extinct",
            };
            let detail = format!("{act} {kind}, {} ft", fmt_thousands(elev));
            self.push("volcano", NameKind::Volcano, v.cx, v.cy, 0.0, v.radius_ft * 4.0, Some(elev.round()), Some(detail));
        }
    }

    fn lakes(&mut self) {
        let inp = self.inp;
        let lakes = inp.hydro.lakes.clone();
        let water: Vec<bool> = (0..inp.w * inp.h).map(|k| inp.hydro.lake_of[k] == super::hydro::NO_LAKE).collect();
        let inside = distance_to(inp.w, inp.h, &water);
        for lake in &lakes {
            if lake.cells.len() < 4 {
                continue;
            }
            let (ax, ay) = pole(inp.w, &lake.cells, &inside);
            let (angle, _) = principal_axis(inp.w, &lake.cells, 0.5);
            let extent = (lake.cells.len() as f64).sqrt() * inp.cell_ft;
            let (kind, nk) = match lake.kind {
                LakeKind::Fresh => ("lake", NameKind::Lake),
                LakeKind::Salt => ("salt_lake", NameKind::SaltLake),
                LakeKind::SaltFlat => ("salt_flat", NameKind::SaltFlat),
            };
            let elev = lake.level_ft - inp.world.params().sea_level_ft;
            let area_mi2 = lake.cells.len() as f64 * inp.cell_ft * inp.cell_ft / (5280.0 * 5280.0);
            let connectivity = if let Some(outlet) = lake.outlet {
                format!("flow-through lake, {} inlet{} | outlet cell {} | area {:.1} sq mi | max depth {:.0} ft", lake.inlet_count, if lake.inlet_count == 1 { "" } else { "s" }, outlet, area_mi2, lake.max_depth_ft)
            } else {
                format!("terminal lake, {} inlet{} | area {:.1} sq mi | max depth {:.0} ft", lake.inlet_count, if lake.inlet_count == 1 { "" } else { "s" }, area_mi2, lake.max_depth_ft)
            };
            let id = self.push(kind, nk, ax, ay, angle, extent, Some(elev.round()), Some(connectivity));
            if let Some(f) = self.out.features.iter_mut().find(|f| f.id == id) {
                f.area_mi2 = Some(area_mi2);
                f.max_depth_ft = Some(lake.max_depth_ft);
                f.inlet_count = Some(lake.inlet_count);
                f.has_outlet = Some(lake.outlet.is_some());
            }
        }
    }

    fn rivers(&mut self) {
        let inp = self.inp;
        let w = inp.w;
        let chains = inp.hydro.rivers.clone();
                let mut falls: Vec<(f64, f64, f64, usize)> = Vec::new();
        let mut rapids: Vec<(f64, f64, f64, usize)> = Vec::new();
        for r in &chains {
            if r.cells.len() < 3 {
                continue;
            }
            let mut pts_cells: Vec<[f64; 2]> = r.cells.iter().map(|&c| [(c as usize % w) as f64, (c as usize / w) as f64]).collect();
            let mut river_q = r.q.clone();
            if r.mouth == Mouth::Confluence {
                if let Some(parent) = r.into {
                    if let Some(nb) = r.terminal_receiver.map(|c| c as usize) {
                        if nb < inp.w * inp.h
                            && chains[parent].cells.iter().any(|&cell_id| cell_id as usize == nb)
                        {
                            pts_cells.push([(nb % w) as f64, (nb / w) as f64]);
                            river_q.push(r.q.last().copied().unwrap_or(0.0));
                        }
                    }
                }
            } else if r.mouth == Mouth::Ocean || r.mouth == Mouth::Lake {
                if let Some(nb) = r.terminal_receiver.map(|c| c as usize) {
                    if nb < inp.w * inp.h && (!inp.land[nb] || inp.hydro.lake_of[nb] != super::hydro::NO_LAKE) {
                        pts_cells.push([
                            0.5 * (pts_cells.last().unwrap()[0] + (nb % w) as f64),
                            0.5 * (pts_cells.last().unwrap()[1] + (nb / w) as f64),
                        ]);
                        river_q.push(r.q.last().copied().unwrap_or(0.0));
                    }
                }
            }
            let (pts, q) = chaikin(&pts_cells, &river_q, 2);
            let named = r.cells.len() >= if r.mouth == Mouth::Confluence { 14 } else { 20 };
            if named {
                let m = pts.len() * 11 / 20;
                let a = pts[m.saturating_sub(3)];
                let b = pts[(m + 3).min(pts.len() - 1)];
                let angle = upright(libm::atan2(b[1] - a[1], b[0] - a[0]));
                let len = r.length_ft;
                let basin_mi2 = r.drainage_area_cells as f64 * inp.cell_ft * inp.cell_ft / (5280.0 * 5280.0);
                let source = if r.source_lake.is_some() { "lake-fed" } else { "headwater" };
                let mouth = match r.mouth {
                    Mouth::Ocean => "ocean",
                    Mouth::Lake => "inland lake",
                    Mouth::Dry => "dry",
                    Mouth::Confluence => "confluence",
                };
                let detail = Some(format!("{} | mouth {} | order {} | basin {} | length {:.1} mi | drainage area {:.1} sq mi | discharge index {:.0} | {} direct tributaries", source, mouth, r.order, r.basin_id, len / 5280.0, basin_mi2, r.peak_discharge, r.tributary_count));
                let id = self.push("river", NameKind::River, pts[m][0], pts[m][1], angle, len, None, detail);
                let path = pts.iter().zip(&q).map(|(&p, &q)| {
                    [p[0] * inp.cell_ft, p[1] * inp.cell_ft, crate::lod::rivers::width_ft(q as f64)]
                }).collect();
                if let Some(f) = self.out.features.iter_mut().find(|f| f.id == id) {
                    f.river_path = Some(path);
                    f.stream_order = Some(r.order);
                    f.drainage_area_mi2 = Some(basin_mi2);
                    f.discharge_index = Some(r.peak_discharge as f64);
                    f.tributary_count = Some(r.tributary_count);
                    f.length_mi = Some(r.length_ft / 5280.0);
                    f.basin_id = Some(r.basin_id);
                    f.river_mouth = Some(match r.mouth { Mouth::Ocean => "ocean", Mouth::Lake => "lake", Mouth::Dry => "dry", Mouth::Confluence => "confluence" });
                    f.source_lake_id = r.source_lake;
                    f.river_source_x = Some((r.source_cell as usize % w) as f64 * inp.cell_ft);
                    f.river_source_y = Some((r.source_cell as usize / w) as f64 * inp.cell_ft);
                    f.mouth_lake_id = r.mouth_lake;
                }
            }
            let _ = q;

            // Deltas are terminal distributary networks, not decorative lines.
            // The authoritative river remains the parent channel; the delta branches only after
            // it reaches the coast. Each branch must cross the real land mask and terminate at a
            // distinct ocean shoreline cell. This keeps the visual feature attached to hydrology
            // while still allowing the many-to-one receiver graph to display a real bifurcation.
            if r.mouth == Mouth::Ocean && r.cells.len() >= super::hydro::DELTA_MIN_CELLS {
                let mouth_q = r.q.last().copied().unwrap_or(r.peak_discharge) as f64;
                let delta_strength = (mouth_q / super::hydro::RIVER_Q).max(0.0);
                let eligible = delta_strength >= 1.25
                    && (r.order >= 2 || r.drainage_area_cells >= 48)
                    && r.length_ft >= (super::hydro::DELTA_MIN_CELLS as f64 * inp.cell_ft);
                if eligible {
                    let mouth_cell = *r.cells.last().unwrap() as usize;
                    if let Some(receiver) = r.terminal_receiver.map(|c| c as usize) {
                        if receiver < inp.w * inp.h && !inp.land[receiver] {
                            let desired = if delta_strength >= 6.0 { 3 } else { 2 };
                            if let Some(delta_paths) = build_delta_paths(inp, r, mouth_cell, receiver, desired) {
                                if delta_paths.len() >= 2 {
                                    let spread_ft = delta_paths.iter()
                                        .flat_map(|p| p.iter())
                                        .map(|p| {
                                            let dx = p[0] - mouth_cell as f64 % w as f64 * inp.cell_ft;
                                            let dy = p[1] - (mouth_cell / w) as f64 * inp.cell_ft;
                                            crate::core::sqrt(dx * dx + dy * dy)
                                        })
                                        .fold(0.0, f64::max);
                                    let area_mi2 = (spread_ft * spread_ft).max(inp.cell_ft * inp.cell_ft)
                                        / (5280.0 * 5280.0);
                                    let mx = (receiver % w) as f64;
                                    let my = (receiver / w) as f64;
                                    let dx = mx - (mouth_cell % w) as f64;
                                    let dy = my - (mouth_cell / w) as f64;
                                    let forward = libm::atan2(dy, dx);
                                    let length_ft = delta_paths.iter()
                                        .map(|p| {
                                            p.windows(2).map(|q| {
                                                let dx = q[1][0] - q[0][0];
                                                let dy = q[1][1] - q[0][1];
                                                crate::core::sqrt(dx * dx + dy * dy)
                                            }).sum::<f64>()
                                        })
                                        .fold(0.0, f64::max);
                                    let delta_id = self.push(
                                        "delta",
                                        NameKind::Delta,
                                        mx,
                                        my,
                                        forward,
                                        spread_ft.max(length_ft),
                                        None,
                                        Some(format!(
                                            "delta of River {} | {} connected distributaries | area {:.1} sq mi | mouth discharge {:.0}",
                                            r.source_cell, delta_paths.len(), area_mi2, mouth_q
                                        )),
                                    );
                                    if let Some(feature) = self.out.features.iter_mut().find(|f| f.id == delta_id) {
                                        feature.area_mi2 = Some(area_mi2);
                                        feature.discharge_index = Some(mouth_q);
                                        feature.length_mi = Some(length_ft / 5280.0);
                                        feature.basin_id = Some(r.basin_id);
                                        feature.delta_paths = Some(delta_paths);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // Waterfalls and rapids: classify sharp downhill channel steps from terrain.
            let mut best: Option<(f64, usize)> = None;
            for k in 0..r.cells.len().saturating_sub(1) {
                let (a, b) = (r.cells[k] as usize, r.cells[k + 1] as usize);
                if !inp.land[b] || inp.hydro.lake_of[b] != super::hydro::NO_LAKE { continue; }
                let drop = inp.height[a] - inp.height[b];
                let grade = drop / inp.cell_ft;
                if drop >= 400.0 && grade >= 0.06 && best.is_none_or(|(d, _)| drop > d) {
                    best = Some((drop, k));
                } else if drop >= 120.0 && grade >= 0.02 {
                    let (cx, cy) = ((a % w) as f64 + 0.5 * ((b % w) as f64 - (a % w) as f64), (a / w) as f64 + 0.5 * ((b / w) as f64 - (a / w) as f64));
                    rapids.push((drop, cx, cy, a));
                }
            }
            if let Some((drop, k)) = best {
                let a = r.cells[k] as usize;
                let b = r.cells[k + 1] as usize;
                let (cx, cy) = ((a % w) as f64 + 0.5 * ((b % w) as f64 - (a % w) as f64), (a / w) as f64 + 0.5 * ((b / w) as f64 - (a / w) as f64));
                falls.push((drop, cx, cy, a));
            }
        }
        // Only the most dramatic drops become named landmarks.
        falls.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.3.cmp(&b.3)));
        for &(drop, cx, cy, c) in falls.iter().take(20) {
            let elev = self.above_sea(c);
            self.push("waterfall", NameKind::Waterfall, cx, cy, 0.0, 30.0 * inp.cell_ft, Some(elev.round()), Some(format!("drop ~{} ft", fmt_thousands(drop))));
        }
        rapids.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.3.cmp(&b.3)));
        let mut placed = Vec::<(f64, f64)>::new();
        let min_sep = 6.0 * inp.cell_ft;
        for &(drop, cx, cy, c) in &rapids {
            if placed.iter().any(|&(x, y)| (x - cx).hypot(y - cy) < min_sep) { continue; }
            let elev = self.above_sea(c);
            self.push("rapids", NameKind::Waterfall, cx, cy, 0.0, 20.0 * inp.cell_ft, Some(elev.round()), Some(format!("river drop ~{} ft", fmt_thousands(drop))));
            placed.push((cx, cy));
            if placed.len() >= 20 { break; }
        }
    }

    fn regions(&mut self) {
        let inp = self.inp;
        let groups: [(&'static str, NameKind, &[Biome], usize); 8] = [
            ("forest", NameKind::Forest, &[Biome::TemperateForest, Biome::TemperateRainforest], 120),
            ("jungle", NameKind::Jungle, &[Biome::Jungle], 120),
            ("taiga", NameKind::Taiga, &[Biome::Taiga], 150),
            ("desert", NameKind::Desert, &[Biome::HotDesert, Biome::ColdDesert], 150),
            ("swamp", NameKind::Swamp, &[Biome::Swamp], 15),
            ("plains", NameKind::Plains, &[Biome::Grassland, Biome::Steppe, Biome::Savanna], 250),
            ("tundra", NameKind::Tundra, &[Biome::Tundra], 150),
            ("glacier", NameKind::Glacier, &[Biome::Ice], 40),
        ];
        for (kind, nk, members, min_cells) in groups {
            let mask: Vec<bool> = inp.biome.iter().map(|&b| members.contains(&Biome::from_u8((b & 0xff) as u8))).collect();
            let inside = distance_to(inp.w, inp.h, &mask.iter().map(|m| !m).collect::<Vec<_>>());
            for comp in components(inp.w, inp.h, |k| mask[k], false) {
                if comp.len() < min_cells {
                    continue;
                }
                let (ax, ay) = pole(inp.w, &comp, &inside);
                let (angle, _) = principal_axis(inp.w, &comp, 0.35);
                let extent = (comp.len() as f64).sqrt() * inp.cell_ft;
                self.push(kind, nk, ax, ay, angle, extent, None, None);
            }
        }

        let p = inp.world.params();
        let plateau_mask: Vec<bool> = (0..inp.w * inp.h).map(|k| {
            if !inp.land[k] || inp.hydro.lake_of[k] != super::hydro::NO_LAKE { return false; }
            if inp.height[k] - p.sea_level_ft < 0.22 * p.max_elev_ft { return false; }
            let mut lo = inp.height[k];
            let mut hi = inp.height[k];
            for (nb, _) in neighbors(inp.w, inp.h, k) { lo = lo.min(inp.height[nb]); hi = hi.max(inp.height[nb]); }
            hi - lo < 0.055 * p.max_elev_ft
        }).collect();
        let plateau_inside = distance_to(inp.w, inp.h, &plateau_mask.iter().map(|m| !m).collect::<Vec<_>>());
        for comp in components(inp.w, inp.h, |k| plateau_mask[k], false) {
            if comp.len() < 45 { continue; }
            let (ax, ay) = pole(inp.w, &comp, &plateau_inside);
            let (angle, _) = principal_axis(inp.w, &comp, 0.25);
            let extent = (comp.len() as f64).sqrt() * inp.cell_ft;
            self.push("plateau", NameKind::Plateau, ax, ay, angle, extent, None, None);
        }

        let valley_mask: Vec<bool> = (0..inp.w * inp.h).map(|k| {
            if !inp.land[k] || inp.hydro.lake_of[k] != super::hydro::NO_LAKE { return false; }
            if inp.height[k] - p.sea_level_ft < 20.0 { return false; }
            let mut higher = 0;
            let mut min_neighbour = inp.height[k];
            for (nb, _) in neighbors(inp.w, inp.h, k) {
                min_neighbour = min_neighbour.min(inp.height[nb]);
                if inp.height[nb] > inp.height[k] + 250.0 { higher += 1; }
            }
            higher >= 2 && inp.height[k] - min_neighbour < 900.0
        }).collect();
        let valley_inside = distance_to(inp.w, inp.h, &valley_mask.iter().map(|m| !m).collect::<Vec<_>>());
        for comp in components(inp.w, inp.h, |k| valley_mask[k], false) {
            if comp.len() < 30 { continue; }
            let (ax, ay) = pole(inp.w, &comp, &valley_inside);
            let (angle, _) = principal_axis(inp.w, &comp, 0.35);
            let extent = (comp.len() as f64).sqrt() * inp.cell_ft;
            self.push("valley", NameKind::Valley, ax, ay, angle, extent, None, None);
        }
    }
}

/// Connected components of `mask` (8- or 4-connected), each a list of cell indices.
pub fn components(w: usize, h: usize, mask: impl Fn(usize) -> bool, eight: bool) -> Vec<Vec<u32>> {
    let mut seen = vec![false; w * h];
    let mut out = Vec::new();
    for s in 0..w * h {
        if seen[s] || !mask(s) {
            continue;
        }
        seen[s] = true;
        let mut comp = vec![s as u32];
        let mut k = 0;
        while k < comp.len() {
            let c = comp[k] as usize;
            k += 1;
            for (nb, d) in neighbors(w, h, c) {
                if (eight || d == 1.0) && !seen[nb] && mask(nb) {
                    seen[nb] = true;
                    comp.push(nb as u32);
                }
            }
        }
        out.push(comp);
    }
    out
}

/// The component cell farthest from its edge (pole of inaccessibility), cell coordinates.
fn pole(w: usize, comp: &[u32], inside: &[f64]) -> (f64, f64) {
    let best = comp.iter().copied().max_by(|&a, &b| inside[a as usize].total_cmp(&inside[b as usize]).then(b.cmp(&a))).unwrap();
    ((best as usize % w) as f64, (best as usize / w) as f64)
}

/// Principal-axis angle (clamped to ±max_angle, kept upright) and axis length in cells.
fn principal_axis(w: usize, comp: &[u32], max_angle: f64) -> (f64, f64) {
    let n = comp.len() as f64;
    let (mut sx, mut sy) = (0.0, 0.0);
    for &k in comp {
        sx += (k as usize % w) as f64;
        sy += (k as usize / w) as f64;
    }
    let (mx, my) = (sx / n, sy / n);
    let (mut cxx, mut cyy, mut cxy) = (0.0, 0.0, 0.0);
    for &k in comp {
        let (dx, dy) = ((k as usize % w) as f64 - mx, (k as usize / w) as f64 - my);
        cxx += dx * dx;
        cyy += dy * dy;
        cxy += dx * dy;
    }
    let (cxx, cyy, cxy) = (cxx / n, cyy / n, cxy / n);
    let angle = 0.5 * libm::atan2(2.0 * cxy, cxx - cyy);
    let tr = cxx + cyy;
    let det = cxx * cyy - cxy * cxy;
    let l1 = tr / 2.0 + crate::core::sqrt((tr * tr / 4.0 - det).max(0.0));
    (upright(angle).clamp(-max_angle, max_angle), 4.0 * crate::core::sqrt(l1))
}

/// Keep text readable: map an angle into (-π/2, π/2].
fn upright(a: f64) -> f64 {
    let pi = std::f64::consts::PI;
    let mut a = a;
    while a > pi / 2.0 {
        a -= pi;
    }
    while a <= -pi / 2.0 {
        a += pi;
    }
    a
}

/// Chaikin corner cutting, keeping both endpoints; per-point values are interpolated.

/// Build a small, deterministic distributary fan from a real ocean mouth.
///
/// The main receiver graph is single-flow by design, so distributaries are represented as a
/// terminal overlay derived from that graph. They are nevertheless terrain-aware: every land
/// segment is found by bounded A* and every branch ends on an actual ocean shoreline cell.
/// Reused cells are penalized so branches share only the natural proximal trunk.
fn build_delta_paths(
    inp: &Inputs,
    river: &super::hydro::River,
    mouth: usize,
    ocean: usize,
    _desired: usize,
) -> Option<Vec<Vec<[f64; 2]>>> {
    let (w, h) = (inp.w, inp.h);
    let cell_ft = inp.cell_ft;
    let mx = (mouth % w) as f64;
    let my = (mouth / w) as f64;
    let ox = (ocean % w) as f64;
    let oy = (ocean / w) as f64;

    // A delta is a terminal network, not a collection of arbitrary paths from the same
    // point. Establish one stable apex-to-sea axis first. The parent river supplies the
    // tangent; the actual ocean receiver prevents a noisy D8 tangent from pointing inland.
    let anchor = river.cells[river.cells.len().saturating_sub(7)] as usize;
    let mut tx = mx - (anchor % w) as f64;
    let mut ty = my - (anchor / w) as f64;
    let tl = crate::core::sqrt(tx * tx + ty * ty).max(1e-6);
    tx /= tl;
    ty /= tl;

    let mut oxv = ox - mx;
    let mut oyv = oy - my;
    let ol = crate::core::sqrt(oxv * oxv + oyv * oyv).max(1e-6);
    oxv /= ol;
    oyv /= ol;

    let mut fx = 0.60 * tx + 0.40 * oxv;
    let mut fy = 0.60 * ty + 0.40 * oyv;
    let fl = crate::core::sqrt(fx * fx + fy * fy).max(1e-6);
    fx /= fl;
    fy /= fl;

    let mouth_z = inp.height[mouth];
    let strength = (river.q.last().copied().unwrap_or(river.peak_discharge) as f64
        / super::hydro::RIVER_Q).max(1.0);

    // Keep the delta compact relative to the parent channel. A broad fan with long fingers
    // reads as a cracked river rather than a delta, especially on generated coastlines.
    let radius = (11.0 + 2.0 * crate::core::sqrt(strength)).round() as usize;
    let radius = radius.clamp(11, 23);

    // Existing rivers are hard obstacles. The parent is blocked too, except for the apex.
    // This preserves the hydrology/feature separation without allowing a branch to crawl
    // backwards along its parent.
    let mut blocked = vec![false; w * h];
    for other in &inp.hydro.rivers {
        if other.source_cell == river.source_cell {
            continue;
        }
        for &c in &other.cells {
            let c = c as usize;
            blocked[c] = true;
            for (nb, _) in neighbors(w, h, c) {
                blocked[nb] = true;
            }
        }
    }
    for &c in &river.cells {
        blocked[c as usize] = true;
    }
    blocked[mouth] = false;

    // A river-dominated delta is a small distributary network: several channels share
    // the proximal trunk, then separate around mouth bars into a fan of unequal outlets.
    // Keep this deliberately small so generated maps get morphology, not a bundle of scratches.
    let branch_count = if strength >= 5.0 { 5 } else { 4 };
    let mut targets = Vec::<(f64, f64, usize)>::new();
    let target_angles: &[f64] = if branch_count == 5 {
        &[-0.95_f64, -0.48, 0.0, 0.48, 0.95]
    } else {
        &[-0.82_f64, -0.30, 0.30, 0.82]
    };
    let target_radius = radius as f64 * 0.82;

    for y in my as isize - radius as isize..=my as isize + radius as isize {
        if y < 0 || y >= h as isize {
            continue;
        }
        for x in mx as isize - radius as isize..=mx as isize + radius as isize {
            if x < 0 || x >= w as isize {
                continue;
            }
            let k = y as usize * w + x as usize;
            if inp.land[k] || !neighbors(w, h, k).any(|(nb, _)| inp.land[nb]) {
                continue;
            }

            let vx = x as f64 - mx;
            let vy = y as f64 - my;
            let d = crate::core::sqrt(vx * vx + vy * vy);
            if d < 5.0 || d > radius as f64 {
                continue;
            }

            let along = (vx * fx + vy * fy) / d;
            if along < 0.35 {
                continue;
            }

            let cross = fx * vy - fy * vx;
            let angle = libm::atan2(cross, vx * fx + vy * fy);
            if angle.abs() > 1.15 {
                continue;
            }

            let land_goal = neighbors(w, h, k)
                .filter(|(nb, _)| inp.land[*nb] && inp.hydro.lake_of[*nb] == super::hydro::NO_LAKE)
                .map(|(nb, _)| nb)
                .find(|&nb| !blocked[nb]);
            let Some(goal) = land_goal else { continue; };

            let coast_z = inp.height[goal];
            let grade = ((mouth_z - coast_z).max(0.0)) / (d * cell_ft).max(cell_ft);
            if !grade.is_finite() || grade > super::hydro::DELTA_MAX_GRADE {
                continue;
            }

            let target_angle_error = target_angles
                .iter()
                .map(|&a| (angle - a).abs())
                .fold(f64::INFINITY, f64::min);
            let distance_error = (d - target_radius).abs() / radius as f64;
            let target_score = target_angle_error * 9.0
                + distance_error * 4.0
                - along * 0.5;
            targets.push((target_score, angle, k));
        }
    }

    // Select one terminal mouth for each intended finger. The separation constraint prevents
    // adjacent shoreline pixels from becoming fake duplicate distributaries.
    let mut chosen = Vec::<usize>::with_capacity(branch_count);
    for &wanted_angle in target_angles {
        let mut best: Option<(f64, usize)> = None;
        for &(score, angle, target) in &targets {
            if (angle - wanted_angle).abs() > 0.32 {
                continue;
            }
            if chosen.iter().any(|&other| {
                let dx = (target % w) as f64 - (other % w) as f64;
                let dy = (target / w) as f64 - (other / w) as f64;
                dx * dx + dy * dy < (0.34 * radius as f64).powi(2)
            }) {
                continue;
            }
            if best.is_none_or(|b| score < b.0) {
                best = Some((score, target));
            }
        }
        if let Some((_, target)) = best {
            chosen.push(target);
        }
    }
    if chosen.len() < 3 {
        return None;
    }

    // Route all distributaries from the real mouth. Only the distal portion is reserved after
    // each route, so later branches naturally share the proximal trunk before bifurcating.
    let branch_start = mouth;
    let mut used = vec![false; w * h];
    let mut paths = Vec::with_capacity(chosen.len());

    for (branch_index, &target) in chosen.iter().enumerate() {
        let mut goals = Vec::<usize>::new();
        for (nb, _) in neighbors(w, h, target) {
            if inp.land[nb]
                && inp.hydro.lake_of[nb] == super::hydro::NO_LAKE
                && !blocked[nb]
            {
                goals.push(nb);
            }
        }
        goals.sort_unstable();

        let mut best: Option<Vec<usize>> = None;
        for goal in goals {
            if let Some(path) = delta_route(
                inp,
                branch_start,
                goal,
                &used,
                &blocked,
                radius + 7,
                fx,
                fy,
            ) {
                if path.len() < 5 {
                    continue;
                }
                let simplified = simplify_delta_path(inp, &path, &blocked);
                if simplified.len() < 4 {
                    continue;
                }
                if best.as_ref().is_none_or(|b| simplified.len() < b.len()) {
                    best = Some(simplified);
                }
            }
        }

        let Some(path) = best else { continue; };

        // Reserve only the downstream part. Natural deltas share a feeder trunk, then split
        // progressively farther from the apex around inter-distributary islands/bars.
        let reserve_from = path.len() * 38 / 100;
        for (i, &cell) in path.iter().enumerate().skip(reserve_from) {
            used[cell] = true;
            if i > reserve_from && i + 1 < path.len() {
                let prev = path[i - 1];
                let next = path[i + 1];
                let dx = (next % w) as isize - (prev % w) as isize;
                let dy = (next / w) as isize - (prev / w) as isize;
                let side = if branch_index % 2 == 0 { (-dy, dx) } else { (dy, -dx) };
                let sx = (cell % w) as isize + side.0.signum();
                let sy = (cell / w) as isize + side.1.signum();
                if sx >= 0 && sy >= 0 && sx < w as isize && sy < h as isize {
                    used[sy as usize * w + sx as usize] = true;
                }
            }
        }

        let mut points = path.iter()
            .map(|&k| [(k % w) as f64 * cell_ft, (k / w) as f64 * cell_ft])
            .collect::<Vec<_>>();
        points.push([(target % w) as f64 * cell_ft, (target / w) as f64 * cell_ft]);

        // Preserve a smooth, slightly sinuous distributary planform without applying the
        // ordinary river meander field a second time in mod.rs.
        let weights = vec![1.0f32; points.len()];
        let (smooth, _) = chaikin(&points, &weights, 2);
        if smooth.len() >= 4 {
            paths.push(smooth);
        }
    }

    if paths.len() >= 3 { Some(paths) } else { None }
}

fn simplify_delta_path(inp: &Inputs, path: &[usize], blocked: &[bool]) -> Vec<usize> {
    if path.len() <= 2 {
        return path.to_vec();
    }

    let mut out = Vec::with_capacity(path.len());
    let mut anchor = 0usize;
    out.push(path[0]);

    while anchor + 1 < path.len() {
        let mut farthest = anchor + 1;
        for candidate in (anchor + 2..path.len()).rev() {
            if delta_segment_clear(inp, path[anchor], path[candidate], blocked) {
                farthest = candidate;
                break;
            }
        }
        out.push(path[farthest]);
        anchor = farthest;
    }

    out
}

fn delta_segment_clear(inp: &Inputs, a: usize, b: usize, blocked: &[bool]) -> bool {
    let (w, h) = (inp.w, inp.h);
    let mut x0 = (a % w) as isize;
    let mut y0 = (a / w) as isize;
    let x1 = (b % w) as isize;
    let y1 = (b / w) as isize;

    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        if x0 < 0 || y0 < 0 || x0 >= w as isize || y0 >= h as isize {
            return false;
        }
        let k = y0 as usize * w + x0 as usize;
        if !inp.land[k]
            || inp.hydro.lake_of[k] != super::hydro::NO_LAKE
            || (blocked[k] && k != a)
        {
            return false;
        }

        if x0 == x1 && y0 == y1 {
            break;
        }

        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x0 += sx;
        }
        if e2 <= dx {
            err += dx;
            y0 += sy;
        }
    }

    true
}

fn delta_route(
    inp: &Inputs,
    start: usize,
    goal: usize,
    used: &[bool],
    blocked: &[bool],
    radius: usize,
    flow_x: f64,
    flow_y: f64,
) -> Option<Vec<usize>> {
    let (w, h) = (inp.w, inp.h);
    let sx = start % w;
    let sy = start / w;
    let gx = goal % w;
    let gy = goal / w;

    let min_x = sx.min(gx).saturating_sub(2);
    let max_x = (sx.max(gx) + 2).min(w.saturating_sub(1));
    let min_y = sy.min(gy).saturating_sub(2);
    let max_y = (sy.max(gy) + 2).min(h.saturating_sub(1));

    let mut heap = BinaryHeap::<Reverse<(u64, u32)>>::new();
    let mut dist = vec![u64::MAX; w * h];
    let mut prev = vec![u32::MAX; w * h];
    dist[start] = 0;
    heap.push(Reverse((0, start as u32)));

    while let Some(Reverse((_, cur_u))) = heap.pop() {
        let cur = cur_u as usize;
        if cur == goal {
            break;
        }

        let cx = cur % w;
        let cy = cur / w;
        if cx < min_x || cx > max_x || cy < min_y || cy > max_y {
            continue;
        }
        if ((cx as isize - sx as isize).abs().max((cy as isize - sy as isize) as isize)) as usize > radius {
            continue;
        }

        let base = dist[cur];
        if base == u64::MAX {
            continue;
        }

        for (nb, diagonal) in neighbors(w, h, cur) {
            if !inp.land[nb] || inp.hydro.lake_of[nb] != super::hydro::NO_LAKE {
                continue;
            }
            if blocked[nb] && nb != start {
                continue;
            }

            let nx = nb % w;
            let ny = nb / w;
            if nx < min_x || nx > max_x || ny < min_y || ny > max_y {
                continue;
            }

            let step = if diagonal > 1.0 { 141u64 } else { 100u64 };
            let rise = (inp.height[nb] - inp.height[cur]).max(0.0);
            if rise > inp.cell_ft * 0.04 {
                continue;
            }

            let cur_dx = cx as f64 - sx as f64;
            let cur_dy = cy as f64 - sy as f64;
            let next_dx = nx as f64 - sx as f64;
            let next_dy = ny as f64 - sy as f64;
            let cur_projection = cur_dx * flow_x + cur_dy * flow_y;
            let next_projection = next_dx * flow_x + next_dy * flow_y;
            if next_projection + 0.15 < cur_projection {
                continue;
            }

            let forward_step = (nx as f64 - cx as f64) * flow_x
                + (ny as f64 - cy as f64) * flow_y;
            let direction_cost = if forward_step < -0.15 {
                2_500u64
            } else if forward_step < 0.05 {
                800u64
            } else {
                0u64
            };

            let slope_cost = (rise / inp.cell_ft * 2400.0) as u64;
            let reuse_cost = if used[nb] && nb != start { 50_000 } else { 0 };
            let g = base.saturating_add(step + slope_cost + reuse_cost + direction_cost);
            if g >= dist[nb] {
                continue;
            }

            dist[nb] = g;
            prev[nb] = cur as u32;

            let dx = gx as f64 - nx as f64;
            let dy = gy as f64 - ny as f64;
            let heuristic = (crate::core::sqrt(dx * dx + dy * dy) * 140.0) as u64;
            heap.push(Reverse((g.saturating_add(heuristic), nb as u32)));
        }
    }

    if dist[goal] == u64::MAX {
        return None;
    }

    let mut path = Vec::new();
    let mut cur = goal;
    for _ in 0..(radius * radius + 8) {
        path.push(cur);
        if cur == start {
            path.reverse();
            return Some(path);
        }
        let p = prev[cur];
        if p == u32::MAX {
            return None;
        }
        cur = p as usize;
    }
    None
}

fn chaikin(pts: &[[f64; 2]], q: &[f32], iterations: usize) -> (Vec<[f64; 2]>, Vec<f32>) {
    let (mut p, mut v) = (pts.to_vec(), q.to_vec());
    for _ in 0..iterations {
        if p.len() < 3 {
            break;
        }
        let mut np = vec![p[0]];
        let mut nv = vec![v[0]];
        for k in 0..p.len() - 1 {
            let (a, b) = (p[k], p[k + 1]);
            np.push([0.75 * a[0] + 0.25 * b[0], 0.75 * a[1] + 0.25 * b[1]]);
            np.push([0.25 * a[0] + 0.75 * b[0], 0.25 * a[1] + 0.75 * b[1]]);
            nv.push(0.75 * v[k] + 0.25 * v[k + 1]);
            nv.push(0.25 * v[k] + 0.75 * v[k + 1]);
        }
        np.push(*p.last().unwrap());
        nv.push(*v.last().unwrap());
        p = np;
        v = nv;
    }
    (p, v)
}

pub fn fmt_thousands(v: f64) -> String {
    let n = v.round() as i64;
    let s = n.abs().to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if n < 0 { format!("-{out}") } else { out }
}
//! Sketch constraints (`world::Sketch`) on a T0-style grid (the full grid, or the coarse one a
//! sketch preview uses):
//!
//! - land and sea strokes decide the land mask (outlines are coastlines; once any land is
//!   drawn, undrawn map is sea), with a natural, noisy coast unless drawn hard;
//! - ranges add rock uplift along their lines, before erosion carves them;
//! - rivers are carved strictly downhill along their lines and fed at their sources, so the
//!   hydrology maps them;
//! - biome paint overrides the climate's biomes, blending at its edges;
//! - pins place settlements (snapped onto land, kept apart).
//!
//! What can't be honoured exactly (a river forced through a gorge, a pin at sea) is reported
//! as a conflict, with where it happened.

use serde::Serialize;

use super::biome::{ALL as BIOMES, Biome};
use super::hydro::NO_LAKE;
use super::settle::Tier;
use crate::World;
use crate::core::noise::{fbm, ridged, smoothstep};
use crate::world::{SketchTool, Stroke};

const MI: f64 = 5280.0;

/// Something drawn that the world could not follow exactly.
#[derive(Clone, Debug, Serialize)]
pub struct Conflict {
    /// Index of the stroke in the sketch.
    pub stroke: usize,
    pub message: String,
    /// Where (world ft).
    pub x: f64,
    pub y: f64,
}

/// A settlement the sketch places.
#[derive(Clone, Debug)]
pub struct Pin {
    pub stroke: usize,
    pub tier: Tier,
    pub x: f64,
    pub y: f64,
    pub cell: usize,
}

/// The grid the sketch is applied to: `w` × `h` points `cell` ft apart, point (i, j) at
/// (i * cell, j * cell).
#[derive(Clone, Copy)]
pub struct Raster {
    pub w: usize,
    pub h: usize,
    pub cell: f64,
}

impl Raster {
    fn cell_of(&self, p: [f64; 2]) -> usize {
        let i = (p[0] / self.cell).round().clamp(0.0, (self.w - 1) as f64) as usize;
        let j = (p[1] / self.cell).round().clamp(0.0, (self.h - 1) as f64) as usize;
        j * self.w + i
    }

    fn at(&self, k: usize) -> [f64; 2] {
        [(k % self.w) as f64 * self.cell, (k / self.w) as f64 * self.cell]
    }
}

fn seg_dist(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let l2 = dx * dx + dy * dy;
    let t = if l2 > 0.0 { (((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / l2).clamp(0.0, 1.0) } else { 0.0 };
    let (qx, qy) = (a[0] + t * dx - p[0], a[1] + t * dy - p[1]);
    crate::core::sqrt(qx * qx + qy * qy)
}

/// Distance (ft) from every grid point to a polyline (closed: back to its start), up to
/// `reach`; farther points get `reach`.
fn distance(g: Raster, pts: &[[f64; 2]], closed: bool, reach: f64) -> Vec<f32> {
    let mut d = vec![reach as f32; g.w * g.h];
    let segs: Vec<([f64; 2], [f64; 2])> = if pts.len() == 1 {
        vec![(pts[0], pts[0])]
    } else {
        let mut s: Vec<_> = pts.windows(2).map(|w| (w[0], w[1])).collect();
        if closed && pts.len() > 2 {
            s.push((pts[pts.len() - 1], pts[0]));
        }
        s
    };
    for (a, b) in segs {
        let i0 = ((a[0].min(b[0]) - reach) / g.cell).floor().max(0.0) as usize;
        let i1 = (((a[0].max(b[0]) + reach) / g.cell).ceil().max(0.0) as usize).min(g.w - 1);
        let j0 = ((a[1].min(b[1]) - reach) / g.cell).floor().max(0.0) as usize;
        let j1 = (((a[1].max(b[1]) + reach) / g.cell).ceil().max(0.0) as usize).min(g.h - 1);
        for j in j0..=j1 {
            for i in i0..=i1 {
                let k = j * g.w + i;
                let v = seg_dist([i as f64 * g.cell, j as f64 * g.cell], a, b) as f32;
                if v < d[k] {
                    d[k] = v;
                }
            }
        }
    }
    d
}

/// Grid points inside a closed outline (even-odd, by scanline).
fn inside(g: Raster, pts: &[[f64; 2]]) -> Vec<bool> {
    let mut out = vec![false; g.w * g.h];
    if pts.len() < 3 {
        return out;
    }
    let mut xs: Vec<f64> = Vec::new();
    for j in 0..g.h {
        let y = j as f64 * g.cell;
        xs.clear();
        for e in 0..pts.len() {
            let (a, b) = (pts[e], pts[(e + 1) % pts.len()]);
            if (a[1] <= y) != (b[1] <= y) {
                xs.push(a[0] + (y - a[1]) / (b[1] - a[1]) * (b[0] - a[0]));
            }
        }
        xs.sort_by(|a, b| a.total_cmp(b));
        for pair in xs.chunks(2) {
            if pair.len() < 2 {
                break;
            }
            let i0 = (pair[0] / g.cell).ceil().max(0.0) as usize;
            let i1 = (pair[1] / g.cell).floor().min((g.w - 1) as f64);
            if i1 < 0.0 {
                continue;
            }
            for i in i0..=(i1 as usize) {
                out[j * g.w + i] = true;
            }
        }
    }
    out
}

/// How far into a stroke's area each point is (ft): positive inside a closed outline or
/// within the brush radius of the line, negative outside; clamped to ±`reach`.
fn signed(g: Raster, s: &Stroke, reach: f64) -> Vec<f64> {
    if s.closed && s.pts.len() >= 3 {
        let d = distance(g, &s.pts, true, reach);
        let ins = inside(g, &s.pts);
        d.iter().zip(&ins).map(|(&d, &i)| if i { d as f64 } else { -(d as f64) }).collect()
    } else {
        let d = distance(g, &s.pts, false, reach + s.radius_ft);
        d.iter().map(|&d| (s.radius_ft - d as f64).clamp(-reach, reach)).collect()
    }
}

/// Unit coordinates (as `plates` uses) of grid point `k`.
fn unit(world: &World, g: Raster, k: usize) -> (f64, f64) {
    let x = (k % g.w) as f64 * g.cell / world.geom.map_w_ft;
    let y = (k / g.w) as f64 * g.cell / world.geom.map_w_ft;
    (x, y)
}

fn strokes(world: &World, tool: SketchTool) -> impl Iterator<Item = (usize, &Stroke)> {
    world.file.sketch.strokes.iter().enumerate().filter(move |(_, s)| s.tool == tool)
}

/// The land signal from drawn land and sea, as a replacement for the plates' crust field
/// (land where it is above 0), or `None` if the sketch draws neither. `crust` and `thr` are the
/// plates' own field and threshold, kept where only sea is drawn.
pub fn land_crust(world: &World, g: Raster, crust: &[f64], thr: f64) -> Option<Vec<f64>> {
    let lands: Vec<&Stroke> = strokes(world, SketchTool::Land).map(|(_, s)| s).collect();
    let seas: Vec<&Stroke> = strokes(world, SketchTool::Sea).map(|(_, s)| s).collect();
    if lands.is_empty() && seas.is_empty() {
        return None;
    }
    let n = g.w * g.h;
    let reach = 60.0 * MI;
    // A natural coast wanders off the drawn line: fractal, from bays and headlands tens of
    // miles across down to coves; a hard one keeps within a mile or so.
    let s_coast = world.stream("t0.sketch.coast");
    let wander: Vec<f64> = (0..n)
        .map(|k| {
            let (x, y) = unit(world, g, k);
            // Wavelengths ~200, 60, 20 and 6 mi; amplitudes falling more slowly than them
            // (a coast's fractal roughness).
            let mut v = 0.0;
            for (o, (freq, amp)) in [(6.0, 18.0), (20.0, 10.0), (60.0, 5.0), (200.0, 2.2)].into_iter().enumerate() {
                v += amp * fbm(s_coast ^ (o as u64 * 0x9e37), x * freq, y * freq, 2, 2.0, 0.5);
            }
            MI * v
        })
        .collect();
    let amount = |s: &Stroke| if s.hard { 0.06 } else { 1.0 };
    // Signals in miles: positive on drawn land (or sea), the coast where they cross zero.
    let mut land = vec![-reach / MI; n];
    for s in &lands {
        let a = amount(s);
        for (k, v) in signed(g, s, reach).into_iter().enumerate() {
            land[k] = land[k].max((v + a * wander[k]) / MI);
        }
    }
    let mut sea = vec![f64::NEG_INFINITY; n];
    for s in &seas {
        let a = amount(s);
        for (k, v) in signed(g, s, reach).into_iter().enumerate() {
            sea[k] = sea[k].max((v - a * wander[k]) / MI);
        }
    }
    // Settlements pinned near drawn land keep land under them (a coastal city stays coastal,
    // wherever the natural coast wanders); one far out at sea is reported by `pins`.
    if !lands.is_empty() {
        let near: Vec<&Stroke> = strokes(world, SketchTool::Pin).map(|(_, s)| s).filter(|s| land[g.cell_of(s.pts[0])] >= -25.0).collect();
        for s in near {
            let reach = 4.0 * MI;
            for (k, d) in distance(g, &s.pts[..1], false, reach).into_iter().enumerate() {
                if (d as f64) < reach {
                    land[k] = land[k].max((3.0 * MI - d as f64) / MI);
                }
            }
        }
    }
    let edge = 4.0;
    Some(
        (0..n)
            .map(|k| {
                let own = if lands.is_empty() { (crust[k] - thr) * 10.0 } else { land[k] };
                let mut c = own.min(-sea[k]);
                // The map's border stays sea.
                let (i, j) = ((k % g.w) as f64, (k / g.w) as f64);
                let to_edge = i.min(j).min((g.w - 1) as f64 - i).min((g.h - 1) as f64 - j);
                c -= 100.0 * (1.0 - smoothstep(0.0, edge, to_edge));
                c
            })
            .collect(),
    )
}

/// Rock uplift along drawn ranges (added to the plates' field before erosion, which halves
/// when any range is drawn).
pub fn add_ranges(world: &World, g: Raster, uplift: &mut [f64]) {
    let s_rid = world.stream("t0.sketch.range");
    let rugged = world.params().ruggedness.max(0.3);
    // Drawn ranges are the main ones: the plates' own ranges are kept, but lower.
    if strokes(world, SketchTool::Range).next().is_some() {
        uplift.iter_mut().for_each(|u| *u *= 0.5);
    }
    for (_, s) in strokes(world, SketchTool::Range) {
        let r = s.radius_ft.max(2.0 * g.cell);
        let amp = (0.2 + 1.3 * s.strength) * rugged;
        let d = distance(g, &s.pts, false, 2.5 * r);
        for (k, &d) in d.iter().enumerate() {
            let d = d as f64;
            if d >= 2.5 * r {
                continue;
            }
            let (x, y) = unit(world, g, k);
            let crest = 0.6 + 0.5 * ridged(s_rid, x * 10.0, y * 10.0, 4);
            uplift[k] += amp * crest * libm::exp(-(d / r) * (d / r));
        }
    }
}

/// Ranges drawn mostly over the sea (they raise nothing there).
pub fn check_ranges(world: &World, g: Raster, land: &[bool], conflicts: &mut Vec<Conflict>) {
    for (i, s) in strokes(world, SketchTool::Range) {
        let wet = s.pts.iter().filter(|p| !land[g.cell_of(**p)]).count();
        if wet * 2 > s.pts.len() {
            let p = s.pts[s.pts.len() / 2];
            conflicts.push(Conflict { stroke: i, message: "This range is mostly at sea: draw land under it".into(), x: p[0], y: p[1] });
        }
    }
}

/// Grid cells along a polyline, in order, 8-connected, without repeats (a loop is cut out).
fn cells_along(g: Raster, pts: &[[f64; 2]]) -> Vec<usize> {
    let mut out: Vec<usize> = Vec::new();
    let mut at = std::collections::HashMap::new();
    let mut add = |k: usize, out: &mut Vec<usize>| {
        if out.last() == Some(&k) {
            return;
        }
        if let Some(&p) = at.get(&k) {
            for c in out.drain(p + 1..) {
                at.remove(&c);
            }
            return;
        }
        at.insert(k, out.len());
        out.push(k);
    };
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = crate::core::sqrt((b[0] - a[0]) * (b[0] - a[0]) + (b[1] - a[1]) * (b[1] - a[1]));
        let steps = (len / (0.4 * g.cell)).ceil().max(1.0) as usize;
        for t in 0..=steps {
            let f = t as f64 / steps as f64;
            add(g.cell_of([a[0] + (b[0] - a[0]) * f, a[1] + (b[1] - a[1]) * f]), &mut out);
        }
    }
    if pts.len() == 1 {
        add(g.cell_of(pts[0]), &mut out);
    }
    out
}

/// Carve the drawn rivers into `height` (strictly downhill from source to mouth, in a valley
/// with banks), and return the discharge (mm·cells) to add at each source so the hydrology
/// maps them as rivers.
pub fn carve_rivers(world: &World, g: Raster, height: &mut [f64], land: &[bool], conflicts: &mut Vec<Conflict>) -> Vec<(usize, f64)> {
    let threshold = super::hydro::RIVER_Q / world.params().river_density;
    let mut feed = Vec::new();
    for (si, s) in strokes(world, SketchTool::River) {
        let mut path = cells_along(g, &s.pts);
        if path.len() < 2 {
            continue;
        }
        // Rivers run from their first point; drawn from the mouth (or uphill), turn them round.
        let (first, last) = (path[0], *path.last().unwrap());
        if (!land[first] && land[last]) || (land[first] && land[last] && height[first] < height[last]) {
            path.reverse();
        }
        let start = path.iter().position(|&k| land[k]);
        let Some(start) = start else {
            let p = s.pts[0];
            conflicts.push(Conflict { stroke: si, message: "This river is entirely at sea".into(), x: p[0], y: p[1] });
            continue;
        };
        let mut course: Vec<usize> = Vec::new();
        for (idx, &k) in path.iter().enumerate().skip(start) {
            if !land[k] {
                if path[idx..].iter().filter(|&&c| land[c]).count() >= 6 {
                    let p = g.at(k);
                    conflicts.push(Conflict { stroke: si, message: "This river reaches the sea before its end; the rest is ignored".into(), x: p[0], y: p[1] });
                }
                break;
            }
            course.push(k);
        }
        if course.len() < 3 {
            let p = g.at(course[0]);
            conflicts.push(Conflict { stroke: si, message: "This river is too short to map".into(), x: p[0], y: p[1] });
            continue;
        }
        // Profile: never rising, at least a foot down per cell; deep cuts are reported.
        let mut z = Vec::with_capacity(course.len());
        let (mut worst, mut worst_at) = (0.0f64, course[0]);
        for (idx, &k) in course.iter().enumerate() {
            let v = if idx == 0 { height[k] } else { height[k].min(z[idx - 1] - 1.0) };
            if height[k] - v > worst {
                (worst, worst_at) = (height[k] - v, k);
            }
            z.push(v);
        }
        if worst > 800.0 {
            let p = g.at(worst_at);
            conflicts.push(Conflict {
                stroke: si,
                message: format!("To keep flowing downhill this river cuts a {:.0}-ft gorge", (worst / 100.0).round() * 100.0),
                x: p[0],
                y: p[1],
            });
        }
        // The valley: banks beside the channel (raised where the ground falls away, so the
        // water stays in it), sides sloping up to the valley's edge. A deep cut widens the
        // valley until its sides meet the ground at a walkable slope (no canal-like walls).
        let base = (s.radius_ft / g.cell).clamp(1.0, 5.0);
        const SIDE: f64 = 0.05;
        let mut floor: std::collections::HashMap<usize, (f64, f64)> = std::collections::HashMap::new();
        for (idx, &k) in course.iter().enumerate() {
            let (ci, cj) = ((k % g.w) as i64, (k / g.w) as i64);
            let reach = ((height[k] - z[idx]) / (g.cell * SIDE)).clamp(base, 14.0);
            let r = reach.ceil() as i64;
            for dj in -r..=r {
                for di in -r..=r {
                    let (i, j) = (ci + di, cj + dj);
                    if i < 0 || j < 0 || i >= g.w as i64 || j >= g.h as i64 || (di == 0 && dj == 0) {
                        continue;
                    }
                    let d = crate::core::sqrt((di * di + dj * dj) as f64);
                    if d > reach {
                        continue;
                    }
                    let nb = j as usize * g.w + i as usize;
                    let target = z[idx] + 2.0 + d * g.cell * SIDE;
                    let e = floor.entry(nb).or_insert((target, d));
                    if target < e.0 {
                        *e = (target, d);
                    }
                }
            }
        }
        let mut cells: Vec<(&usize, &(f64, f64))> = floor.iter().collect();
        cells.sort_by_key(|(k, _)| **k);
        for (&k, &(target, d)) in cells {
            if !land[k] {
                continue;
            }
            height[k] = if d < 1.5 { target } else { height[k].min(target) };
        }
        for (idx, &k) in course.iter().enumerate() {
            height[k] = z[idx];
        }
        feed.push((course[0], threshold * (1.2 + 10.0 * s.strength * s.strength)));
    }
    feed
}

/// Paint drawn biomes over the classified ones (on land, not lakes): the painted biome inside,
/// an ecotone with the old one at the edge.
pub fn paint_biomes(world: &World, g: Raster, land: &[bool], lake_of: &[u32], biome: &mut [u32]) {
    let s_edge = world.stream("t0.sketch.biome");
    for (_, s) in strokes(world, SketchTool::Biome) {
        let Some(target) = s.biome.as_deref().and_then(|b| BIOMES.iter().copied().find(|x| x.name() == b)) else { continue };
        if matches!(target, Biome::Ocean | Biome::Lake) {
            continue;
        }
        let width = if s.hard { 0.6 * g.cell } else { (0.35 * s.radius_ft).max(2.0 * g.cell) };
        let sd = signed(g, s, 3.0 * width);
        for (k, &v) in sd.iter().enumerate() {
            if !land[k] || lake_of[k] != NO_LAKE {
                continue;
            }
            let (x, y) = unit(world, g, k);
            let wobble = if s.hard { 0.0 } else { 0.8 * width * fbm(s_edge, x * 30.0, y * 30.0, 3, 2.0, 0.5) };
            let m = smoothstep(-width, width, v + wobble);
            if m <= 0.0 {
                continue;
            }
            let b1 = biome[k] & 0xff;
            if m > 0.5 {
                let blend = ((1.0 - m) * 2.0 * 255.0) as u32;
                biome[k] = target as u32 | b1 << 8 | blend.min(255) << 16;
            } else {
                let blend = (m * 2.0 * 255.0) as u32;
                biome[k] = b1 | (target as u32) << 8 | blend.min(255) << 16;
            }
        }
    }
}

/// The settlements the sketch pins: snapped onto land (within 25 mi) and at least 3 mi apart.
pub fn pins(world: &World, g: Raster, land: &[bool], conflicts: &mut Vec<Conflict>) -> Vec<Pin> {
    let mut out: Vec<Pin> = Vec::new();
    for (si, s) in strokes(world, SketchTool::Pin) {
        let tier = match s.tier.as_deref() {
            Some("metropolis") => Tier::Metropolis,
            Some("city") => Tier::City,
            Some("town") => Tier::Town,
            _ => Tier::Village,
        };
        let p = s.pts[0];
        let mut k = g.cell_of(p);
        let (mut x, mut y) = (p[0].clamp(0.0, (g.w - 1) as f64 * g.cell), p[1].clamp(0.0, (g.h - 1) as f64 * g.cell));
        if !land[k] {
            let r = (25.0 * MI / g.cell).ceil() as i64;
            let (ci, cj) = ((k % g.w) as i64, (k / g.w) as i64);
            let mut best: Option<(i64, usize)> = None;
            for dj in -r..=r {
                for di in -r..=r {
                    let (i, j) = (ci + di, cj + dj);
                    if i < 0 || j < 0 || i >= g.w as i64 || j >= g.h as i64 || di * di + dj * dj > r * r {
                        continue;
                    }
                    let c = j as usize * g.w + i as usize;
                    if land[c] && best.is_none_or(|b| di * di + dj * dj < b.0) {
                        best = Some((di * di + dj * dj, c));
                    }
                }
            }
            match best {
                Some((_, c)) => {
                    k = c;
                    [x, y] = g.at(c);
                    conflicts.push(Conflict { stroke: si, message: "This pin was in the water: moved onto the nearest land".into(), x, y });
                }
                None => {
                    conflicts.push(Conflict { stroke: si, message: "This pin is far out at sea: no settlement placed".into(), x: p[0], y: p[1] });
                    continue;
                }
            }
        }
        if out.iter().any(|q| crate::core::sqrt((q.x - x) * (q.x - x) + (q.y - y) * (q.y - y)) < 3.0 * MI) {
            conflicts.push(Conflict { stroke: si, message: "This pin is within 3 miles of another: no settlement placed".into(), x, y });
            continue;
        }
        out.push(Pin { stroke: si, tier, x, y, cell: k });
    }
    out
}

//! Volcanoes: sited on subduction arcs and hotspots (the tectonic `arc` field), spaced apart,
//! then stamped onto the eroded terrain so cones and craters stay sharp. Active ones get a
//! volcanic-waste halo (biomes) and, later, lava flows and lava tubes.

use serde::Serialize;

use crate::World;
use crate::core::rng::Pcg32;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum VolcanoKind {
    Stratovolcano,
    Shield,
    CinderCone,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Activity {
    Active,
    Dormant,
    Extinct,
}

#[derive(Clone, Debug)]
pub struct Volcano {
    /// Cell coordinates on the T0 grid.
    pub cx: f64,
    pub cy: f64,
    pub kind: VolcanoKind,
    pub activity: Activity,
    /// Cone height above the surrounding terrain and base radius, ft.
    pub height_ft: f64,
    pub radius_ft: f64,
}

pub fn place(world: &World, w: usize, h: usize, cell_ft: f64, arc: &[f64], height: &[f64]) -> Vec<Volcano> {
    let count = world.params().volcanoes as usize;
    if count == 0 {
        return Vec::new();
    }
    let mut rng = Pcg32::new(world.stream("t0.volcano"), 3);
    let sea = world.params().sea_level_ft;
    let min_sep = sq(70.0 * 5280.0 / cell_ft);

    // Candidates: strongest arc cells on land or shallow sea, with jitter to vary picks.
    let mut cands: Vec<(f64, usize)> = (0..w * h)
        .filter(|&k| arc[k] > 0.25 && height[k] > sea - 3000.0)
        .map(|k| (arc[k] + 0.4 * rng.next_f64(), k))
        .collect();
    cands.sort_by(|a, b| b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)));

    let mut out: Vec<Volcano> = Vec::new();
    for (_, k) in cands {
        if out.len() >= count {
            break;
        }
        let (cx, cy) = ((k % w) as f64, (k / w) as f64);
        if out.iter().any(|v| sq(v.cx - cx) + sq(v.cy - cy) < min_sep) {
            continue;
        }
        let kind = match rng.below(10) {
            0..=5 => VolcanoKind::Stratovolcano,
            6..=7 => VolcanoKind::Shield,
            _ => VolcanoKind::CinderCone,
        };
        let activity = match rng.below(20) {
            0..=6 => Activity::Active,
            7..=14 => Activity::Dormant,
            _ => Activity::Extinct,
        };
        let (height_ft, radius_mi) = match kind {
            VolcanoKind::Stratovolcano => (rng.range(6_000.0, 10_000.0), rng.range(9.0, 14.0)),
            VolcanoKind::Shield => (rng.range(4_000.0, 7_000.0), rng.range(20.0, 30.0)),
            VolcanoKind::CinderCone => (rng.range(1_000.0, 2_000.0), rng.range(3.0, 5.0)),
        };
        out.push(Volcano { cx, cy, kind, activity, height_ft, radius_ft: radius_mi * 5280.0 });
    }
    out
}

/// Stamp cones and summit craters onto the height grid (ft).
pub fn apply(volcanoes: &[Volcano], w: usize, h: usize, cell_ft: f64, sea: f64, max_elev: f64, height: &mut [f64]) {
    for v in volcanoes {
        let r_cells = v.radius_ft / cell_ft;
        let crater = (0.09 * r_cells).max(0.6);
        let (x0, x1) = ((v.cx - r_cells).floor().max(0.0) as usize, ((v.cx + r_cells).ceil() as usize).min(w - 1));
        let (y0, y1) = ((v.cy - r_cells).floor().max(0.0) as usize, ((v.cy + r_cells).ceil() as usize).min(h - 1));
        // Cone sits on the local base level; offshore ones rise from a shallow seamount so
        // they break the surface as volcanic islands.
        let base = height[v.cy as usize * w + v.cx as usize].max(sea - 1_500.0);
        for y in y0..=y1 {
            for x in x0..=x1 {
                let r = crate::core::sqrt(sq(x as f64 - v.cx) + sq(y as f64 - v.cy));
                if r >= r_cells {
                    continue;
                }
                let t = 1.0 - r / r_cells;
                let exp = if v.kind == VolcanoKind::Shield { 1.1 } else { 1.8 };
                let mut add = v.height_ft * libm::pow(t, exp);
                if r < crater {
                    add -= 0.12 * v.height_ft * (1.0 - r / crater);
                }
                let k = y * w + x;
                height[k] = height[k].max((base + add).min(sea + max_elev));
            }
        }
    }
}

#[inline]
fn sq(x: f64) -> f64 {
    x * x
}

//! Scratch dev tool: shoreline consistency across levels. Samples a grid of fixed world
//! points around a map point and reports, per level, the wet fraction and how many points
//! flip wet/dry (and the mean height change) relative to the previous level.
//! `cargo run --release -p worldgen --example seacheck -- <seed> <fx> <fy> [half-width ft]`
use worldgen::core::tile::{HALO, PADDED, TileKey};
use worldgen::pipeline::Executor;
use worldgen::{World, WorldFile};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let seed: u32 = a[1].parse().unwrap();
    let (fx, fy): (f64, f64) = (a[2].parse().unwrap(), a[3].parse().unwrap());
    let half: f64 = a.get(4).and_then(|v| v.parse().ok()).unwrap_or(15_000.0);
    let world = World::new(WorldFile { seed, ..Default::default() }).unwrap();
    let g = world.geom.clone();
    let mut ex = Executor::new(world);
    let (cx, cy) = (fx * g.map_w_ft, fy * g.map_h_ft);
    const N: usize = 60;
    let pts: Vec<(f64, f64)> = (0..N * N).map(|k| (cx - half + 2.0 * half * (k % N) as f64 / (N - 1) as f64, cy - half + 2.0 * half * (k / N) as f64 / (N - 1) as f64)).collect();
    let mut prev: Option<Vec<(f32, bool)>> = None;
    for level in 6..=14 {
        let s = g.spacing_ft(level);
        let size = g.tile_size_ft(level);
        let mut cur = Vec::with_capacity(pts.len());
        for &(x, y) in &pts {
            let key = TileKey::surface(level, (x / size) as u32, (y / size) as u32);
            let (ox, oy) = g.tile_origin_ft(&key);
            let (i, j) = (((x - ox) / s).round() as usize, ((y - oy) / s).round() as usize);
            let k = (j + HALO) * PADDED + i + HALO;
            let t = ex.terrain(key);
            let (h, rw) = (t.padded[k], t.river_water[k]);
            let w = ex.t0.sample_water(ox + i as f64 * s, oy + j as f64 * s).max(rw);
            cur.push((h, h < w));
        }
        let wet = cur.iter().filter(|c| c.1).count();
        let line = match &prev {
            Some(p) => {
                let flips = p.iter().zip(&cur).filter(|(a, b)| a.1 != b.1).count();
                let dh = p.iter().zip(&cur).map(|(a, b)| (a.0 - b.0).abs() as f64).sum::<f64>() / cur.len() as f64;
                format!("flips {:5.1}%  mean |dh| {dh:6.2} ft", 100.0 * flips as f64 / cur.len() as f64)
            }
            None => String::new(),
        };
        println!("L{level:<2} wet {:5.1}%  {line}", 100.0 * wet as f64 / cur.len() as f64);
        prev = Some(cur);
    }
}

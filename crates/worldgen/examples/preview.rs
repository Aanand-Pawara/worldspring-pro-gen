//! Dev tool: generate T0 for a seed and write a PNG preview plus stats.
//! `cargo run --release -p worldgen --example preview -- <seed> <out.png> [params-json] [sketch.json]`
//! With a sketch, also writes the quick sketch preview (`<out>-quick.png`) and lists conflicts.

use std::time::Instant;

use worldgen::t0::T0;
use worldgen::t0::biome::{ALL, Biome};
use worldgen::{World, WorldFile};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let seed: u32 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let out = args.get(2).cloned().unwrap_or_else(|| "preview.png".into());
    let mut file = WorldFile { seed, ..Default::default() };
    if let Some(p) = args.get(3) {
        if !p.is_empty() && p != "{}" {
            file.params = serde_json::from_str(p).expect("params json");
        }
    }
    if let Some(path) = args.get(4) {
        file.sketch = serde_json::from_str(&std::fs::read_to_string(path).expect("sketch file")).expect("sketch json");
    }
    let world = World::new(file).expect("valid world");
    if !world.file.sketch.is_empty() {
        let t = Instant::now();
        let q = T0::preview(&world, 256);
        eprintln!("quick preview {}x{} in {:.0} ms", q.w, q.h, t.elapsed().as_secs_f64() * 1000.0);
        let quick = out.replace(".png", "-quick.png");
        let mut enc = png::Encoder::new(std::io::BufWriter::new(std::fs::File::create(&quick).unwrap()), q.w as u32, q.h as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        enc.write_header().unwrap().write_image_data(&q.rgba).unwrap();
        for c in &q.conflicts {
            eprintln!("  preview conflict (stroke {}): {} at ({:.0}, {:.0})", c.stroke, c.message, c.x, c.y);
        }
    }

    let start = Instant::now();
    let mut last = Instant::now();
    let mut stage = String::new();
    let t0 = T0::generate_with_progress(&world, &mut |s, _| {
        if s != stage {
            if !stage.is_empty() {
                eprintln!("  {stage:<10} {:>6.0} ms", last.elapsed().as_secs_f64() * 1000.0);
            }
            stage = s.to_string();
            last = Instant::now();
        }
    });
    eprintln!("T0 total {:.2} s", start.elapsed().as_secs_f64());

    let (w, h) = (t0.height.w, t0.height.h);
    let extra = t0.extra.as_ref().unwrap();
    for c in &extra.overlay.conflicts {
        eprintln!("conflict (stroke {}): {} at ({:.0}, {:.0})", c.stroke, c.message, c.x, c.y);
    }
    let towns: Vec<_> = extra.overlay.features.iter().filter(|f| ["metropolis", "city", "town", "village"].contains(&f.kind)).collect();
    for (s, f) in t0.settlements.iter().zip(&towns) {
        if let Some(pin) = s.pin {
            eprintln!("pinned (stroke {pin}): {} {} at ({:.0}, {:.0}), {}", f.kind, f.name, f.x, f.y, f.detail.as_deref().unwrap_or(""));
        }
    }
    let sea = world.params().sea_level_ft;
    let mut counts = [0usize; ALL.len()];
    for &b in &t0.biome.data {
        counts[(b & 0xff) as usize] += 1;
    }
    let land = t0.height.data.iter().filter(|&&v| v as f64 > sea).count();
    let low_land = (0..w * h).filter(|&k| t0.water.data[k] <= -29_000.0 && (t0.height.data[k] as f64) < sea).count();
    eprintln!("dry cells below sea level: {low_land}");
    eprintln!("land {:.1}%  max elev {:.0} ft", 100.0 * land as f64 / (w * h) as f64, t0.height.data.iter().fold(f32::MIN, |a, &b| a.max(b)));
    let named = |k: &str| extra.overlay.features.iter().filter(|f| f.kind == k).count();
    eprintln!(
        "rivers {} (named {}, falls {})  lakes {}  ranges {}  peaks {}  passes {}  volcanoes {}  islands {}  bays {}",
        t0.rivers.rivers.len(),
        named("river"),
        named("waterfall"),
        extra.hydro.lakes.len(),
        named("range"),
        named("peak"),
        named("pass"),
        named("volcano"),
        named("island"),
        named("bay")
    );
    let mut bs: Vec<String> = ALL
        .iter()
        .filter(|b| counts[**b as usize] > 0 && !matches!(b, Biome::Ocean))
        .map(|b| format!("{} {:.1}%", b.name(), 100.0 * counts[*b as usize] as f64 / land.max(1) as f64))
        .collect();
    bs.sort();
    eprintln!("biomes: {}", bs.join(", "));
    for f in extra.overlay.features.iter().filter(|f| matches!(f.kind, "continent" | "range" | "volcano" | "ocean")).take(12) {
        eprintln!("  {:<10} {}{}", f.kind, f.name, f.detail.as_ref().map(|d| format!(" ({d})")).unwrap_or_default());
    }

    // Settlements and roads.
    let tiers = ["village", "town", "city", "metropolis"].map(named);
    eprintln!("settlements: {} villages, {} towns, {} cities, {} metropolises; ruins {}, towers {}, waystations {}", tiers[0], tiers[1], tiers[2], tiers[3], named("ruin"), named("tower"), named("waystation"));
    let mut len = [0.0f64; 3];
    let mut worst = [0.0f64; 3];
    let mut pts = 0;
    for r in &t0.roads.roads {
        pts += r.pts.len();
        for k in 1..r.pts.len() {
            let d = ((r.pts[k][0] - r.pts[k - 1][0]).powi(2) + (r.pts[k][1] - r.pts[k - 1][1]).powi(2)).sqrt();
            len[r.class as usize] += d / 5280.0;
            if d > 1.0 {
                worst[r.class as usize] = worst[r.class as usize].max(((r.z[k] - r.z[k - 1]) as f64).abs() / d);
            }
        }
    }
    let mut cross = [0usize; 3];
    for c in &extra.crossings {
        cross[c.kind as usize] += 1;
    }
    eprintln!(
        "roads {} ({} pts): king's {:.0} mi, road {:.0} mi, track {:.0} mi; max grade {:.3}/{:.3}/{:.3}; bridges {} fords {} ferries {}",
        t0.roads.roads.len(), pts, len[0], len[1], len[2], worst[0], worst[1], worst[2], cross[0], cross[1], cross[2]
    );
    // Where a king's road actually crosses the widest river (road curve over a channel).
    {
        let cell = t0.cell_ft;
        let mut best: Option<(f64, [f64; 2])> = None;
        for r in t0.roads.roads.iter().filter(|r| r.class as u8 == 0) {
            for k in 0..r.pts.len() - 1 {
                for s in 0..8 {
                    let p = r.eval(k, s as f64 / 8.0, 2.5, cell).p;
                    for (vi, vk) in t0.rivers.segments_near(p[0] - 300.0, p[1] - 300.0, p[0] + 300.0, p[1] + 300.0, 0.0) {
                        for u in 0..=16 {
                            let cp = t0.rivers.rivers[vi as usize].eval(vk as usize, u as f64 / 16.0, 2.5, cell);
                            let d = ((p[0] - cp.p[0]).powi(2) + (p[1] - cp.p[1]).powi(2)).sqrt();
                            if d < 0.4 * cp.w && best.is_none_or(|b| cp.w > b.0) {
                                best = Some((cp.w, p));
                            }
                        }
                    }
                }
            }
        }
        if let Some((w, p)) = best {
            eprintln!("  king's road bridge over {w:.0} ft river at fx {:.6} fy {:.6}", p[0] / world.geom.map_w_ft, p[1] / world.geom.map_h_ft);
        }
    }
    // A switchback stretch (points with no wander), the longest one.
    if let Some((r, k0, len)) = t0
        .roads
        .roads
        .iter()
        .flat_map(|r| {
            let mut runs = Vec::new();
            let mut k = 0;
            while k < r.pts.len() {
                if r.wander[k] == 0.0 {
                    let s = k;
                    while k < r.pts.len() && r.wander[k] == 0.0 {
                        k += 1;
                    }
                    runs.push((r, s, k - s));
                }
                k += 1;
            }
            runs
        })
        .max_by_key(|x| x.2)
    {
        let p = r.pts[k0 + len / 2];
        eprintln!("  switchbacks: {len} legs on a {:?} at fx {:.6} fy {:.6}", r.class, p[0] / world.geom.map_w_ft, p[1] / world.geom.map_h_ft);
    }
    for f in extra.overlay.features.iter().filter(|f| matches!(f.kind, "metropolis" | "city")).take(8) {
        eprintln!("  {:<10} {} ({}) fx {:.4} fy {:.4}", f.kind, f.name, f.detail.as_deref().unwrap_or(""), f.x / world.geom.map_w_ft, f.y / world.geom.map_h_ft);
    }

    // One sample location (map fractions) per biome, for screenshots.
    for b in ALL {
        let mut hits: Vec<usize> = (0..w * h).filter(|&k| (t0.biome.data[k] & 0xff) as u8 == b as u8).collect();
        if hits.len() > 50 && !matches!(b, Biome::Ocean | Biome::Lake) {
            hits.sort_by_key(|&k| (k * 2654435761) % 1_000_003);
            let k = hits[0];
            eprintln!("  at {:<20} fx {:.4} fy {:.4}", b.name(), (k % w) as f64 / (w - 1) as f64, (k / w) as f64 / (h - 1) as f64);
        }
    }
    if let Some(r) = t0.rivers.rivers.iter().max_by(|a, b| a.q.last().unwrap().total_cmp(b.q.last().unwrap())) {
        let p = r.pts[r.pts.len() / 2];
        eprintln!("largest river mid: fx {:.4} fy {:.4}", p[0] / world.geom.map_w_ft, p[1] / world.geom.map_h_ft);
    }

    // Render: biome color × hillshade, water, rivers, feature dots.
    let mut img = vec![0u8; w * h * 3];
    let cell = t0.cell_ft;
    for j in 0..h {
        for i in 0..w {
            let k = j * w + i;
            let hgt = t0.height.data[k] as f64;
            let water = t0.water.data[k] as f64;
            let at = |x: usize, y: usize| t0.height.data[y.min(h - 1) * w + x.min(w - 1)] as f64;
            let gx = (at(i + 1, j) - at(i.saturating_sub(1), j)) / (2.0 * cell);
            let gy = (at(i, j + 1) - at(i, j.saturating_sub(1))) / (2.0 * cell);
            let (nx, ny, nz) = (-gx * 12.0, -gy * 12.0, 1.0);
            let nl = (nx * nx + ny * ny + nz * nz).sqrt();
            let shade = ((nx * -0.6 + ny * -0.6 + nz * 0.53) / nl / 0.53).clamp(0.3, 1.4);
            let c = if hgt < water {
                let d = ((water - hgt) / 8000.0).clamp(0.0, 1.0);
                [(150.0 - 60.0 * d) as u8, (185.0 - 60.0 * d) as u8, (200.0 - 40.0 * d) as u8]
            } else {
                let b = Biome::from_u8((t0.biome.data[k] & 0xff) as u8);
                let base = color(b);
                [
                    (base[0] as f64 * shade).min(255.0) as u8,
                    (base[1] as f64 * shade).min(255.0) as u8,
                    (base[2] as f64 * shade).min(255.0) as u8,
                ]
            };
            img[k * 3..k * 3 + 3].copy_from_slice(&c);
        }
    }
    for r in &t0.rivers.rivers {
        let maxq = r.q.iter().fold(0f32, |a, &b| a.max(b));
        let thick = maxq > 2_000_000.0;
        for p in &r.pts {
            let (i, j) = ((p[0] as f64 / cell) as usize, (p[1] as f64 / cell) as usize);
            for (di, dj) in if thick { vec![(0, 0), (1, 0), (0, 1)] } else { vec![(0, 0)] } {
                let (x, y) = ((i + di).min(w - 1), (j + dj).min(h - 1));
                img[(y * w + x) * 3..(y * w + x) * 3 + 3].copy_from_slice(&[40, 80, 170]);
            }
        }
    }
    for r in &t0.roads.roads {
        let col = [[120, 30, 20], [150, 80, 40], [170, 130, 90]][r.class as usize];
        for k in 1..r.pts.len() {
            let (a, b) = (r.pts[k - 1], r.pts[k]);
            let n = ((((b[0] - a[0]).abs()).max((b[1] - a[1]).abs()) / cell * 2.0).ceil() as usize).max(1);
            for s in 0..=n {
                let t = s as f64 / n as f64;
                let (x, y) = (((a[0] + (b[0] - a[0]) * t) / cell) as usize, ((a[1] + (b[1] - a[1]) * t) / cell) as usize);
                let (x, y) = (x.min(w - 1), y.min(h - 1));
                img[(y * w + x) * 3..(y * w + x) * 3 + 3].copy_from_slice(&col);
            }
        }
    }
    for f in &extra.overlay.features {
        let col = match f.kind {
            "metropolis" | "city" => [0, 0, 0],
            "town" => [60, 20, 60],
            "village" => [200, 200, 200],
            "ruin" | "tower" => [255, 0, 255],
            "peak" => [120, 20, 20],
            "volcano" => [255, 60, 0],
            "pass" => [255, 255, 255],
            _ => continue,
        };
        let (i, j) = ((f.x / cell) as usize, (f.y / cell) as usize);
        for d in [(0i64, 0i64), (1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (x, y) = ((i as i64 + d.0).clamp(0, w as i64 - 1) as usize, (j as i64 + d.1).clamp(0, h as i64 - 1) as usize);
            img[(y * w + x) * 3..(y * w + x) * 3 + 3].copy_from_slice(&col);
        }
    }
    let file = std::fs::File::create(&out).unwrap();
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), w as u32, h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.write_header().unwrap().write_image_data(&img).unwrap();
    eprintln!("wrote {out}");
}

fn color(b: Biome) -> [u8; 3] {
    match b {
        Biome::Ocean => [90, 120, 150],
        Biome::Lake => [110, 150, 190],
        Biome::Ice => [240, 245, 250],
        Biome::Tundra => [170, 175, 150],
        Biome::Alpine => [150, 140, 125],
        Biome::Taiga => [70, 105, 80],
        Biome::TemperateForest => [80, 130, 60],
        Biome::TemperateRainforest => [45, 110, 70],
        Biome::Grassland => [165, 185, 100],
        Biome::Steppe => [190, 185, 120],
        Biome::ColdDesert => [185, 170, 140],
        Biome::HotDesert => [225, 200, 140],
        Biome::Savanna => [195, 185, 95],
        Biome::Jungle => [30, 110, 40],
        Biome::Swamp => [90, 115, 80],
        Biome::Volcanic => [80, 60, 55],
        Biome::SaltFlat => [235, 230, 215],
    }
}

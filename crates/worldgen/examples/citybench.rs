//! Dev tool: time the heaviest content natively: T0, the metropolis layout, and the battlemap
//! chunks (terrain refine + chunk + pack) around its centre.
//! `cargo run --release -p worldgen --example citybench -- [seed] [radius]`
use std::time::Instant;

use worldgen::core::tile::TileKey;
use worldgen::lod::terrain_refine::terrain_tile;
use worldgen::t0::T0;
use worldgen::t0::settle::Tier;
use worldgen::{World, WorldFile};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let seed: u32 = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(1);
    let r: i64 = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(2);
    let world = World::new(WorldFile { seed, ..Default::default() }).unwrap();
    let t = Instant::now();
    let t0 = T0::generate(&world);
    println!("T0 {:.0} ms", t.elapsed().as_secs_f64() * 1e3);
    let g = &world.geom;
    let si = (0..t0.settlements.len()).filter(|&i| t0.settlements[i].tier == Tier::Metropolis).max_by_key(|&i| t0.settlements[i].population).unwrap_or(0);
    let s = &t0.settlements[si];
    let t = Instant::now();
    let l = worldgen::town::layout(&world, &t0, si);
    println!("layout #{si}: {:.0} ms, {} buildings", t.elapsed().as_secs_f64() * 1e3, l.buildings.len());
    // Output fingerprint (to confirm a speed-up changes nothing).
    let mut lh = String::new();
    for b in &l.buildings {
        lh.push_str(&format!("{:?}{}{:?}{}", b.poly, b.floors, b.func, b.pad_ft));
    }
    for p in l.streets.iter().chain(&l.walls).chain(&l.districts) {
        lh.push_str(&format!("{p:?}"));
    }
    println!("layout hash {:016x}", worldgen::core::hash::fnv64(lh.as_bytes()));
    let mut chunk_bytes: Vec<u8> = Vec::new();
    let lv = g.max_level;
    let size = g.tile_size_ft(lv);
    let (cx, cy) = ((s.x / size) as i64, (s.y / size) as i64);
    let (mut refine, mut chunk_ms, mut pack_ms, mut worst, mut n, mut objs, mut kb) = (0.0, 0.0, 0.0, 0.0f64, 0, 0, 0);
    for dy in -r..=r {
        for dx in -r..=r {
            let key = TileKey::surface(lv, (cx + dx) as u32, (cy + dy) as u32);
            // The parent chain, as the app would have it.
            let mut parent: Option<Vec<f32>> = None;
            let mut tile = None;
            for l in 0..=lv {
                let pk = TileKey::surface(l, key.x >> (lv - l), key.y >> (lv - l));
                let t = Instant::now();
                let tt = terrain_tile(&world, &t0, &pk, parent.as_deref());
                if l == lv {
                    refine += t.elapsed().as_secs_f64() * 1e3;
                }
                parent = Some(tt.padded.clone());
                tile = Some(tt);
            }
            let tile = tile.unwrap();
            let t = Instant::now();
            let c = worldgen::battlemap::generate(&world, &t0, &key, &tile);
            let ms = t.elapsed().as_secs_f64() * 1e3;
            let t = Instant::now();
            let bytes = worldgen::battlemap::pack(&world, &c);
            chunk_bytes.extend_from_slice(&bytes);
            pack_ms += t.elapsed().as_secs_f64() * 1e3;
            chunk_ms += ms;
            worst = worst.max(ms);
            n += 1;
            objs += c.objects.len();
            kb += bytes.len() / 1024;
        }
    }
    println!("chunks hash {:016x}", worldgen::core::hash::fnv64(&chunk_bytes));
    let n = n as f64;
    println!(
        "{n} chunks: refine {:.1} ms, battlemap {:.1} ms (worst {worst:.1}), pack {:.2} ms, {:.0} objects, {:.0} KB avg",
        refine / n,
        chunk_ms / n,
        pack_ms / n,
        objs as f64 / n,
        kb as f64 / n
    );
}

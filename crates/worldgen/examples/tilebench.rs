//! Dev tool: time terrain tile generation per level (refine vs pack) for the default world.
//! `cargo run --release -p worldgen --example tilebench -- [seed] [fx fy]` (map fractions; a
//! second pass shows warm-cache timings when the point is in a settlement).

use std::time::Instant;

use worldgen::core::tile::TileKey;
use worldgen::lod::terrain_refine::terrain_tile;
use worldgen::payload::pack_terrain;
use worldgen::t0::T0;
use worldgen::{World, WorldFile};

fn main() {
    let seed: u32 = std::env::args().nth(1).and_then(|s| s.parse().ok()).unwrap_or(99);
    let world = World::new(WorldFile { seed, ..Default::default() }).unwrap();
    let t0 = T0::generate(&world);
    let g = &world.geom;
    let arg = |i: usize, d: f64| std::env::args().nth(i).and_then(|s| s.parse().ok()).unwrap_or(d);
    let (fx, fy) = (arg(2, 0.5), arg(3, 0.45));
    for pass in 0..2 {
    if pass == 1 {
        println!("-- warm --");
    }
    let mut parent: Option<Vec<f32>> = None;
    for level in 0..=g.max_level {
        let size = g.tile_size_ft(level);
        let key = TileKey::surface(level, (fx * g.map_w_ft / size) as u32, (fy * g.map_h_ft / size) as u32);
        let t = Instant::now();
        let tile = terrain_tile(&world, &t0, &key, parent.as_deref());
        let refine_ms = t.elapsed().as_secs_f64() * 1000.0;
        let t = Instant::now();
        let bytes = pack_terrain(&world, &t0, &key, &tile);
        let pack_ms = t.elapsed().as_secs_f64() * 1000.0;
        println!("L{level:<2} refine {refine_ms:6.2} ms  pack {pack_ms:6.2} ms  ({} KB)", bytes.len() / 1024);
        if level == g.max_level {
            let t = Instant::now();
            let chunk = worldgen::battlemap::generate(&world, &t0, &key, &tile);
            let ms = t.elapsed().as_secs_f64() * 1000.0;
            let bytes = worldgen::battlemap::pack(&world, &chunk);
            let tiers = chunk.tier.iter().copied().collect::<std::collections::BTreeSet<_>>().len();
            println!("battlemap {ms:6.2} ms  {} objects  {tiers} tiers  {:?}  ({} KB)", chunk.objects.len(), chunk.atmosphere, bytes.len() / 1024);
        }
        parent = Some(tile.padded);
    }
    }
}

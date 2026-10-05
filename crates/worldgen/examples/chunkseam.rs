//! Dev tool: compare battlemap heights across a chunk boundary (seam diagnosis).
//! `cargo run --release -p worldgen --example chunkseam -- <seed> <fx> <fy>`

use worldgen::battlemap::SQ;
use worldgen::core::tile::TileKey;
use worldgen::pipeline::Executor;
use worldgen::{World, WorldFile};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let seed: u32 = a.get(1).and_then(|s| s.parse().ok()).unwrap_or(99);
    let fx: f64 = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(0.3666);
    let fy: f64 = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(0.8357);
    let world = World::new(WorldFile { seed, ..Default::default() }).unwrap();
    let g = world.geom.clone();
    let mut ex = Executor::new(world);
    let size = g.tile_size_ft(g.max_level);
    let (x, y) = ((fx * g.map_w_ft / size) as u32, (fy * g.map_h_ft / size) as u32);
    let top = ex.battlemap(TileKey::surface(g.max_level, x, y));
    let bot = ex.battlemap(TileKey::surface(g.max_level, x, y + 1));
    let (mut worst, mut worst_i, mut inner) = (0f32, 0, 0f32);
    for i in 0..SQ {
        let d = (top.height[(SQ - 1) * SQ + i] - bot.height[i]).abs();
        if d > worst {
            worst = d;
            worst_i = i;
        }
        inner = inner.max((top.height[(SQ - 2) * SQ + i] - top.height[(SQ - 1) * SQ + i]).abs());
    }
    println!("max jump across boundary {worst:.2} ft at column {worst_i}; max jump between the two rows inside {inner:.2} ft");
    let i = worst_i;
    for r in (SQ - 4)..SQ {
        print!("{:.1} ", top.height[r * SQ + i]);
    }
    print!("| ");
    for r in 0..4 {
        print!("{:.1} ", bot.height[r * SQ + i]);
    }
    println!();
}

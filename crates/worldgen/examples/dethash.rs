//! Prints the determinism report for a world file (default world if no argument).
//! `scripts/det-wasm.mjs` compares this against the WASM build.

use std::io::Read;

fn main() {
    let mut json = String::new();
    if std::env::args().nth(1).as_deref() == Some("-") {
        std::io::stdin().read_to_string(&mut json).unwrap();
    } else {
        json = serde_json::to_string(&worldgen::WorldFile::default()).unwrap();
    }
    match worldgen::pipeline::det_report(&json) {
        Ok(report) => print!("{report}"),
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}

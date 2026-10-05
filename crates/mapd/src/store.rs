//! Worlds on disk: `<dir>/<world hash>/world.json` (the world file with its edits) and
//! `edits.jsonl` (every change: when, who, what), `<dir>/current` (the open world's hash), and
//! `<dir>/assets/<id>`: uploaded pictures (sprites, portraits) by content hash, shared by all
//! worlds.

use std::io::Write;
use std::path::PathBuf;

use serde_json::Value;
use worldgen::WorldFile;

pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn new(dir: PathBuf) -> Store {
        let _ = std::fs::create_dir_all(&dir);
        Store { dir }
    }

    fn world_dir(&self, hash: u64) -> PathBuf {
        self.dir.join(format!("{hash:016x}"))
    }

    pub fn load(&self, hash: u64) -> Option<WorldFile> {
        let text = std::fs::read_to_string(self.world_dir(hash).join("world.json")).ok()?;
        serde_json::from_str(&text).ok()
    }

    pub fn save(&self, hash: u64, file: &WorldFile) {
        let d = self.world_dir(hash);
        let _ = std::fs::create_dir_all(&d);
        if let Ok(text) = serde_json::to_string_pretty(file) {
            // Write then rename, so a crash never leaves half a file.
            let tmp = d.join("world.json.tmp");
            if std::fs::write(&tmp, text).is_ok() {
                let _ = std::fs::rename(&tmp, d.join("world.json"));
            }
        }
        let _ = std::fs::write(self.dir.join("current"), format!("{hash:016x}"));
    }

    /// Append a change to the world's edit log.
    pub fn log(&self, hash: u64, entry: Value) {
        let d = self.world_dir(hash);
        let _ = std::fs::create_dir_all(&d);
        if let Ok(mut f) = std::fs::OpenOptions::new().create(true).append(true).open(d.join("edits.jsonl")) {
            let _ = writeln!(f, "{entry}");
        }
    }

    /// An asset's id is the hex content hash the app gives it.
    pub fn asset_id_ok(id: &str) -> bool {
        (16..=64).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
    }

    pub fn asset(&self, id: &str) -> Option<Vec<u8>> {
        Self::asset_id_ok(id).then(|| std::fs::read(self.dir.join("assets").join(id)).ok()).flatten()
    }

    pub fn has_asset(&self, id: &str) -> bool {
        Self::asset_id_ok(id) && self.dir.join("assets").join(id).exists()
    }

    pub fn put_asset(&self, id: &str, bytes: &[u8]) -> Result<(), String> {
        if !Self::asset_id_ok(id) {
            return Err("an asset id is 16–64 lowercase hex digits".into());
        }
        let d = self.dir.join("assets");
        let _ = std::fs::create_dir_all(&d);
        let tmp = d.join(format!("{id}.tmp"));
        std::fs::write(&tmp, bytes).and_then(|_| std::fs::rename(&tmp, d.join(id))).map_err(|e| e.to_string())
    }

    /// The world open last time.
    pub fn current(&self) -> Option<WorldFile> {
        let hash = u64::from_str_radix(std::fs::read_to_string(self.dir.join("current")).ok()?.trim(), 16).ok()?;
        self.load(hash)
    }
}

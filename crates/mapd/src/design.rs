//! MCP tools for underground sites designed by hand (`Edits::designs`, `under::design`): read a
//! site as a text plan, write one back (checked by the same rules every generated site keeps),
//! or go back to the generated site.

use serde_json::{Value, json};
use worldgen::core::hash::fnv64;
use worldgen::under::design::{self, SiteDesign};

use crate::Shared;
use crate::tools::{arg_str, schema, text};

const FORMAT: &str = "The plan, levels numbered from 1 at the top: 'level <n>: <name>', then 'rooms: a=<kind>; b=<kind>+5 \"<name>\"' (symbol = room; +5 a floor raised 5 ft; a name in quotes), 'grid:' and one row per line (one character per 5-ft square, '.' rock, x across from 0, y down from 0), 'doors: x,y e; x,y s secret' (between square x,y and the one east or south of it; 'auto' adds doors wherever a room is shut off), 'items: <kind> x,y [wxh]; …' (props, and the ways: exit on the top level at the entry, down on every level but the deepest, up below each down on the same square).";

pub fn list() -> Vec<Value> {
    vec![
        json!({ "name": "get_site_design", "title": "Site plan", "description": format!("An underground site (u:<layout>:<k>; not a city's sewers) as a text plan to read or change with set_site_design, with what is wrong with it if anything. {FORMAT}"), "inputSchema": schema(json!({ "id": { "type": "string", "description": "The site's id (u:<layout>:<k>)" } }), &["id"]), "annotations": { "readOnlyHint": true } }),
        json!({ "name": "set_site_design", "title": "Design a site", "description": format!("Change an underground site by hand from a text plan (get_site_design gives the current one). Only what the text gives changes: a level block with just 'items:' changes only that level's items. A header 'site · <n> levels' adds levels below (new ones start as rock: give their grids) or fills in the deepest. Kinds of room the generator doesn't know become chambers named as written. Refused, with the reasons, when the site would break a rule play mode needs: one way in at the entry, each way down above a way up, every square reachable (around props that block movement), props on floor and apart. {FORMAT}"), "inputSchema": schema(json!({
            "id": { "type": "string" },
            "text": { "type": "string", "description": "The plan, or the parts of it to change" },
            "furnish": { "type": "boolean", "default": false, "description": "Fill rooms that have no props from their kind's kit" },
            "dry_run": { "type": "boolean", "default": false, "description": "Check it without saving" },
        }), &["id", "text"]) }),
        json!({ "name": "reset_site_design", "title": "Site as generated", "description": "Drop a site's design: it is generated again as it was.", "inputSchema": schema(json!({ "id": { "type": "string" } }), &["id"]) }),
    ]
}

pub async fn call(app: &Shared, name: &str, a: &Value) -> Option<Result<Vec<Value>, String>> {
    Some(match name {
        "get_site_design" => get(app, a).await,
        "set_site_design" => set(app, a).await,
        "reset_site_design" => reset(app, a).await,
        _ => return None,
    })
}

/// A summary and the plan, as two texts (the plan reads as it is written).
fn reply(summary: Value, plan: String) -> Vec<Value> {
    let mut out = text(summary);
    out.push(json!({ "type": "text", "text": plan }));
    out
}

fn summary(id: &str, d: &SiteDesign, designed: bool, problems: &[design::Problem]) -> Value {
    json!({
        "id": id,
        "designed": designed,
        "kind": d.kind,
        "theme": d.theme,
        "grid": [d.nx, d.ny],
        "entry": d.entry,
        "levels": d.levels.len(),
        "problems": problems.iter().map(|p| json!({ "text": p.text, "at": p.at, "refused": p.blocking })).collect::<Vec<_>>(),
    })
}

async fn get(app: &Shared, a: &Value) -> Result<Vec<Value>, String> {
    let id = arg_str(a, "id")?;
    app.worker
        .with(move |ex| {
            let d = design::design_of(&ex.world, &ex.t0, &id, false)?;
            let names = &ex.world.file.edits.renames;
            let plan = design::to_text(&d, &|li, ri| names.get(&format!("r:{id}:{li}:{ri}")).cloned())?;
            let (_, problems) = d.problems(&id);
            Ok(reply(summary(&id, &d, ex.world.file.edits.designs.contains_key(&id), &problems), plan))
        })
        .await
}

async fn set(app: &Shared, a: &Value) -> Result<Vec<Value>, String> {
    let id = arg_str(a, "id")?;
    let src = arg_str(a, "text")?;
    let (furnish, dry) = (a["furnish"].as_bool().unwrap_or(false), a["dry_run"].as_bool().unwrap_or(false));
    let id2 = id.clone();
    let (d, names, problems) = app
        .worker
        .with(move |ex| {
            let base = design::design_of(&ex.world, &ex.t0, &id2, false)?;
            let (mut d, names) = design::from_text(&src, &base)?;
            if furnish {
                let (it, _) = d.problems(&id2);
                for (li, lv) in it.levels.iter().enumerate() {
                    for (ri, r) in lv.rooms.iter().enumerate() {
                        let bare = !lv
                            .furniture
                            .iter()
                            .any(|f| !design::WAYS.contains(&f.kind) && lv.cells[f.y as usize * it.nx + f.x as usize] == ri as i16);
                        if r.squares > 0 && bare {
                            d.furnish(li, ri, fnv64(id2.as_bytes()));
                        }
                    }
                }
            }
            let (_, problems) = d.problems(&id2);
            Ok((d, names, problems))
        })
        .await?;
    let refused: Vec<&str> = problems.iter().filter(|p| p.blocking).map(|p| p.text.as_str()).collect();
    if !refused.is_empty() {
        return Err(format!("not saved: {}", refused.join("; ")));
    }
    let plan = design::to_text(&d, &|li, ri| names.iter().find(|n| n.0 == li && n.1 == ri).map(|n| n.2.clone()))?;
    let out = summary(&id, &d, true, &problems);
    if dry {
        return Ok(reply(json!({ "dry_run": true, "would_save": out }), plan));
    }
    let id3 = id.clone();
    app.edit("agent", 0, move |e| {
        for (li, ri, name) in names {
            e.renames.insert(format!("r:{id3}:{li}:{ri}"), name);
        }
        e.designs.insert(id3.clone(), d);
        Ok(json!({ "tool": "set_site_design", "id": id3 }))
    })
    .await?;
    Ok(reply(out, plan))
}

async fn reset(app: &Shared, a: &Value) -> Result<Vec<Value>, String> {
    let id = arg_str(a, "id")?;
    app.edit("agent", 0, move |e| {
        e.designs.remove(&id).ok_or_else(|| format!("{id} has no design: it is as generated"))?;
        Ok(json!({ "tool": "reset_site_design", "id": id }))
    })
    .await
    .map(text)
}

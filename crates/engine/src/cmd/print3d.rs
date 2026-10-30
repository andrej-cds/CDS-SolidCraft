//! 3D-printing tools (CDS fork): Lego brick, Supershape, airfoil and infill generators, vent
//! slots, dogbone corners, whole-body and whole-sketch offsets, Export for Printing and a quick
//! Send to Slicer. Written for SolidCraft from the geometry itself; no add-in code is used.
//!
//! The generators build through the ordinary commands (sketches, extrusions, cylinders,
//! patterns), so every result is plain, editable history; each tool is one undo step.

use serde_json::{Value, json};
use solidcraft_geom::{Vec2, Vec3};

use super::print3d_shapes as shapes;
use super::{CommandSpec, in_sketch, not_in_sketch};
use crate::params::{bad, bool_, num, str_, vec3};
use crate::{EngineError, Result, Session};

pub static COMMANDS: &[CommandSpec] = &[
    CommandSpec::new("print.lego", "Lego Brick", lego).at("3D PRINT", "CREATE").icon("box").enabled(not_in_sketch).params(
        "studs_x?, studs_y?: whole numbers 1…32 (default 4 × 2); kind?: brick|plate|tile (default brick: 9.6 mm high; plate and tile 3.2 mm, a tile without studs); \
         clearance?: mm taken off each side for printing (default 0.1); position?: [x,y,z] corner (default the origin); name? → body, features",
    ),
    CommandSpec::new("print.supershape", "Supershape", supershape).at("3D PRINT", "CREATE").icon("equation_curve").enabled(not_in_sketch).params(
        "m?, n1?, n2?, n3?, a?, b?: the superformula's numbers (default 6, 1, 1.7, 1.7, 1, 1); size?: mm across (default 50); \
         height?: mm extruded (default 10; 0: the sketch alone); solid?: bool (true: a 3D Supershape as a mesh body, size across); plane?: XY|XZ|YZ → sketch, body",
    ),
    CommandSpec::new("print.airfoil", "Airfoil Profile", airfoil).at("3D PRINT", "CREATE").icon("spline").enabled(not_in_sketch).params(
        "naca?: four NACA digits (default \"2412\"), or path: an airfoil .dat file (Selig or Lednicer); chord?: mm (default 100); points?: per side 10…200 (default 60); \
         span?: mm extruded (default 0: the sketch alone); plane?: XY|XZ|YZ → sketch, body?",
    ),
    CommandSpec::new("print.infill", "Infill Block", infill).at("3D PRINT", "CREATE").icon("pattern_rect").enabled(not_in_sketch).params(
        "pattern?: honeycomb|grid|gyroid (default honeycomb; gyroid is a mesh body); size?: [x,y,z] mm (default the body's size, else 40 × 40 × 10); body?: a body whose box it fills; \
         position?: [x,y,z] corner (default the body's corner, else the origin); cell?: mm (default 8); wall?: mm (default 0.8); frame?: mm border (honeycomb and grid, default 2) → body",
    ),
    CommandSpec::new("print.vent", "Vent Slots", vent).at("3D PRINT", "MODIFY").icon("slot").enabled(not_in_sketch).params(
        "face: [x,y,z] on a flat face (the middle of the vents); body?; shape?: round|rectangle|slot (default slot); columns?, rows?: 1…40 (default 6 × 1); \
         width?, height?: mm of one opening (default 4 × 20; round: width is the diameter); pitch_x?, pitch_y?: mm centre to centre (default width + 3, height + 3); \
         depth?: mm (default through all) → sketch, feature",
    ),
    CommandSpec::new("print.dogbone", "Dogbone Corners", dogbone).at("3D PRINT", "MODIFY").icon("hole").enabled(not_in_sketch).params(
        "edges: [[x,y,z] points on straight inside corner edges]; diameter?: mm of the tool (default 3); body? → features: one cut cylinder per corner",
    ),
    CommandSpec::new("print.offset_body", "Offset Body", offset_body).at("3D PRINT", "MODIFY").icon("offset_face").enabled(not_in_sketch).params(
        "body: name; distance: mm or expression (positive grows the body, negative shrinks it, e.g. -0.2 for a printing clearance) → feature",
    ),
    CommandSpec::new("print.offset_sketch", "Offset Whole Sketch", offset_sketch).at("SKETCH", "MODIFY").icon("offset").enabled(in_sketch).params(
        "distance: mm or expression; inward?: bool (default false: every loop and chain grows outward) → the offset of every connected chain of the open sketch",
    ),
    CommandSpec::new("print.export", "Export for Printing", export).at("3D PRINT", "OUTPUT").icon("export").noundo().params(
        "folder?: where the files go; config?: a .json settings file (default `<design>.print.json` beside the saved design, written with the defaults when missing); \
         format?: 3mf|stl; quality?: standard|fine|finest; per?: body|component; name?: file name pattern with {design}, {body}, {n}; folder_suffix?: the default folder's last word (`<design> - print`) → files, folder, config",
    ),
    CommandSpec::new("print.send", "Quick Send to Slicer", quick_send).at("3D PRINT", "OUTPUT").icon("mesh_export").noundo().params(
        "slicer: as file.send_to_slicer takes it (the app passes the one chosen last); bodies?: [names] (default the selected bodies, else every visible body) → as file.send_to_slicer",
    ),
];

/// Run `steps` as one tool: the commands' own undo steps are folded into the caller's one.
fn run_steps(s: &mut Session, steps: Vec<Value>) -> Result<Vec<Value>> {
    let undo = s.undo.clone();
    let redo = s.redo.clone();
    let r = s.run_script(&Value::Array(steps));
    s.undo = undo;
    if r.is_err() {
        s.redo = redo;
    }
    r
}

fn step(command: &str, params: Value) -> Value {
    json!({"command": command, "params": params})
}

/// Run one command as part of a tool, keeping the caller's single undo step.
fn run_one(s: &mut Session, command: &str, params: Value) -> Result<Value> {
    Ok(run_steps(s, vec![step(command, params)])?.into_iter().next().unwrap_or(Value::Null))
}

/// A number parameter within `lo..=hi`, or its default.
fn number(cmd: &str, p: &Value, k: &str, default: f64, lo: f64, hi: f64) -> Result<f64> {
    match p.get(k) {
        None | Some(Value::Null) => Ok(default),
        Some(Value::String(t)) => t.trim().parse::<f64>().ok().filter(|v| (lo..=hi).contains(v)).ok_or_else(|| bad(cmd, format!("`{k}` must be a number from {lo} to {hi}"))),
        Some(_) => num(p, k).filter(|v| (lo..=hi).contains(v)).ok_or_else(|| bad(cmd, format!("`{k}` must be a number from {lo} to {hi}"))),
    }
}

/// A length parameter in the design's expressions (a number is mm), its value in mm.
fn length(s: &Session, cmd: &str, p: &Value, k: &str, default: f64, lo: f64, hi: f64) -> Result<f64> {
    let v = match p.get(k) {
        None | Some(Value::Null) => return Ok(default),
        Some(Value::String(t)) if t.trim().is_empty() => return Ok(default),
        Some(Value::String(t)) => s.doc.eval(t, solidcraft_doc::expr::Kind::Length).map_err(|e| bad(cmd, format!("`{k}`: {e}")))?,
        Some(_) => num(p, k).ok_or_else(|| bad(cmd, format!("`{k}` must be a length")))?,
    };
    if !(v >= lo && v <= hi) {
        return Err(bad(cmd, format!("`{k}` must be from {lo} to {hi} mm")));
    }
    Ok(v)
}

fn count(cmd: &str, p: &Value, k: &str, default: usize, hi: usize) -> Result<usize> {
    let v = number(cmd, p, k, default as f64, 1.0, hi as f64)?;
    if v.fract() != 0.0 {
        return Err(bad(cmd, format!("`{k}` must be a whole number")));
    }
    Ok(v as usize)
}

fn plane(cmd: &str, p: &Value) -> Result<&'static str> {
    match str_(p, "plane").unwrap_or("XY") {
        "XY" | "xy" => Ok("XY"),
        "XZ" | "xz" => Ok("XZ"),
        "YZ" | "yz" => Ok("YZ"),
        _ => Err(bad(cmd, "`plane` must be XY, XZ or YZ")),
    }
}

/// A body name not used yet: `base`, else `base (2)`, `base (3)`…
fn fresh_body(s: &Session, base: &str) -> String {
    let st = s.model.state();
    let taken = |n: &str| st.bodies.iter().any(|b| b.name == n);
    if !taken(base) {
        return base.to_string();
    }
    (2..10_000).map(|i| format!("{base} ({i})")).find(|n| !taken(n)).unwrap_or_else(|| base.to_string())
}

/// The feature a command answered with.
fn feature_of(v: &Value) -> Option<u64> {
    v.get("feature").and_then(Value::as_u64).or_else(|| v.get("id").and_then(Value::as_u64))
}

fn sketch_of(v: &Value) -> Option<u64> {
    v.get("sketch").and_then(Value::as_u64).or_else(|| feature_of(v))
}

/// The point ids a sketch command created, in order.
fn created_points(v: &Value) -> Vec<String> {
    v.pointer("/created/points").and_then(Value::as_array).map(|a| a.iter().filter_map(|x| x.as_str().map(str::to_string)).collect()).unwrap_or_default()
}

fn xy(p: Vec2) -> Value {
    json!([round(p.x), round(p.y)])
}

fn xyz(p: Vec3) -> Value {
    json!([round(p.x), round(p.y), round(p.z)])
}

fn round(v: f64) -> f64 {
    (v * 1e6).round() / 1e6
}

// ---------------------------------------------------------------- Lego brick

fn lego(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "print.lego";
    const PITCH: f64 = 8.0;
    let nx = count(cmd, p, "studs_x", 4, 32)?;
    let ny = count(cmd, p, "studs_y", 2, 32)?;
    let kind = str_(p, "kind").unwrap_or("brick");
    let height = match kind {
        "brick" => 9.6,
        "plate" | "tile" => 3.2,
        _ => return Err(bad(cmd, "`kind` must be brick, plate or tile")),
    };
    let gap = length(s, cmd, p, "clearance", 0.1, 0.0, 1.0)?;
    let at = match p.get("position") {
        None | Some(Value::Null) => Vec3::ZERO,
        Some(v) => vec3(v).ok_or_else(|| bad(cmd, "`position` must be [x, y, z]"))?,
    };
    let (long, short) = (nx.max(ny), nx.min(ny));
    let base = str_(p, "name").filter(|n| !n.trim().is_empty()).map(str::to_string).unwrap_or_else(|| format!("Lego {nx}x{ny}"));
    let body = fresh_body(s, &base);
    let (len, wid) = (nx as f64 * PITCH - 2.0 * gap, ny as f64 * PITCH - 2.0 * gap);
    const WALL: f64 = 1.2;
    let inner = height - WALL;
    let first = |i: f64, j: f64| Vec3::new(at.x + PITCH / 2.0 - gap + i * PITCH, at.y + PITCH / 2.0 - gap + j * PITCH, 0.0);
    let mut steps = vec![
        step("solid.box", json!({"length": round(len), "width": round(wid), "height": height, "corner": xyz(at), "body_name": body, "name": body})),
        step("solid.shell", json!({"faces": [xyz(Vec3::new(at.x + len / 2.0, at.y + wid / 2.0, at.z))], "thickness": WALL, "direction": "inside", "body": body})),
    ];
    let mut patterned: Vec<(usize, usize, usize, f64, f64)> = Vec::new();
    if kind != "tile" {
        let c = first(0.0, 0.0);
        steps.push(step(
            "solid.cylinder",
            json!({"diameter": 4.8, "height": 1.8, "base": xyz(Vec3::new(c.x, c.y, at.z + height)), "axis": "Z", "operation": "join", "targets": [body], "name": format!("{body} Stud")}),
        ));
        patterned.push((steps.len() - 1, nx, ny, PITCH, PITCH));
    }
    if short >= 2 {
        let c = first(0.5, 0.5);
        steps.push(step(
            "solid.cylinder",
            json!({"diameter": 6.51, "height": round(inner), "base": xyz(Vec3::new(c.x, c.y, at.z)), "axis": "Z", "operation": "join", "targets": [body], "name": format!("{body} Tube")}),
        ));
        patterned.push((steps.len() - 1, nx - 1, ny - 1, PITCH, PITCH));
        steps.push(step(
            "solid.cylinder",
            json!({"diameter": 4.8, "height": round(inner), "base": xyz(Vec3::new(c.x, c.y, at.z)), "axis": "Z", "operation": "cut", "targets": [body], "name": format!("{body} Tube Bore")}),
        ));
        patterned.push((steps.len() - 1, nx - 1, ny - 1, PITCH, PITCH));
    } else if long >= 2 {
        let c = if nx >= ny { first(0.5, 0.0) } else { first(0.0, 0.5) };
        steps.push(step(
            "solid.cylinder",
            json!({"diameter": 3.0, "height": round(inner), "base": xyz(Vec3::new(c.x, c.y, at.z)), "axis": "Z", "operation": "join", "targets": [body], "name": format!("{body} Post")}),
        ));
        patterned.push((steps.len() - 1, if nx >= ny { nx - 1 } else { 1 }, if nx >= ny { 1 } else { ny - 1 }, PITCH, PITCH));
    }
    // Build the box, shell and seeds first, then pattern each seed by the feature it made.
    let results = run_steps(s, steps)?;
    let mut features: Vec<u64> = results.iter().filter_map(feature_of).collect();
    let mut pattern_steps = Vec::new();
    for (index, cx, cy, sx, sy) in patterned {
        let Some(seed) = results.get(index).and_then(feature_of).and_then(|id| s.doc.feature(id).map(|f| f.name.clone())) else { continue };
        let q = match (cx > 1, cy > 1) {
            (true, true) => json!({"features": [seed], "dir1": [1, 0, 0], "count1": cx, "spacing1": sx, "dir2": [0, 1, 0], "count2": cy, "spacing2": sy}),
            (true, false) => json!({"features": [seed], "dir1": [1, 0, 0], "count1": cx, "spacing1": sx}),
            (false, true) => json!({"features": [seed], "dir1": [0, 1, 0], "count1": cy, "spacing1": sy}),
            (false, false) => continue,
        };
        pattern_steps.push(step("solid.pattern.rectangular", q));
    }
    if !pattern_steps.is_empty() {
        features.extend(run_steps(s, pattern_steps)?.iter().filter_map(feature_of));
    }
    Ok(json!({"body": body, "features": features, "size": [round(len), round(wid), height]}))
}

// ---------------------------------------------------------------- Supershape

fn supershape(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "print.supershape";
    let m = number(cmd, p, "m", 6.0, 0.0, 100.0)?;
    let n1 = number(cmd, p, "n1", 1.0, 0.01, 100.0)?;
    let n2 = number(cmd, p, "n2", 1.7, 0.0, 100.0)?;
    let n3 = number(cmd, p, "n3", 1.7, 0.0, 100.0)?;
    let a = number(cmd, p, "a", 1.0, 0.01, 100.0)?;
    let b = number(cmd, p, "b", 1.0, 0.01, 100.0)?;
    let size = length(s, cmd, p, "size", 50.0, 0.1, 10_000.0)?;
    let body = fresh_body(s, "Supershape");
    if bool_(p, "solid").unwrap_or(false) {
        let tris = shapes::supershape_solid(m, n1, n2, n3, a, b, size).ok_or_else(|| bad(cmd, "these numbers give no closed shape"))?;
        return mesh_body(s, cmd, &tris, &body);
    }
    let height = length(s, cmd, p, "height", 10.0, 0.0, 10_000.0)?;
    let pts = shapes::supershape_outline(m, n1, n2, n3, a, b, size, 240).ok_or_else(|| bad(cmd, "these numbers give no closed outline"))?;
    let plane = plane(cmd, p)?;
    let created = run_steps(
        s,
        vec![
            step("sketch.create", json!({"plane": plane, "name": "Supershape"})),
            step("sketch.spline.fit_point", json!({"points": pts.iter().map(|q| xy(*q)).collect::<Vec<_>>(), "periodic": true})),
            step("sketch.finish", json!({})),
        ],
    )?;
    let sketch = created.first().and_then(sketch_of);
    let mut out = json!({"sketch": sketch});
    if height > 0.0 {
        let r = run_one(s, "solid.extrude", json!({"sketch": sketch, "distance": height, "body_name": body, "name": "Supershape"}))?;
        out["body"] = json!(body);
        out["feature"] = json!(feature_of(&r));
    }
    Ok(out)
}

/// Triangles as a mesh body, in a component of its own.
fn mesh_body(s: &mut Session, cmd: &str, tris: &[[Vec3; 3]], name: &str) -> Result<Value> {
    if tris.is_empty() {
        return Err(bad(cmd, "nothing to build"));
    }
    let file = format!("{name}.stl");
    let bytes = shapes::stl_bytes(tris, name);
    let mf = solidcraft_io::mesh_import_feature(&bytes, &file).map_err(|e| EngineError::Other(format!("{cmd}: {e}")))?;
    let (id, placed) = super::file::import::add_in_component(s, mf.kind, name, mf.body_names, name, super::file::import::Tree::Flat)?;
    let mut out = json!({"feature": id, "component": name, "triangles": tris.len()});
    placed.report(&mut out);
    Ok(out)
}

// ---------------------------------------------------------------- Airfoil

fn airfoil(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "print.airfoil";
    let per_side = count(cmd, p, "points", 60, 200)?.max(10);
    let (pts, name) = match str_(p, "path").filter(|x| !x.trim().is_empty()) {
        Some(path) => {
            let bytes = solidcraft_io::vfs::read(path, 4 << 20).map_err(|e| EngineError::Other(format!("{path}: {e}")))?;
            let text = String::from_utf8_lossy(&bytes);
            let pts = shapes::airfoil_dat(&text).ok_or_else(|| bad(cmd, "the file holds no airfoil coordinates"))?;
            let stem = std::path::Path::new(path).file_stem().map(|x| x.to_string_lossy().to_string()).unwrap_or_else(|| "Airfoil".into());
            (pts, stem)
        }
        None => {
            let code = str_(p, "naca").unwrap_or("2412");
            let pts = shapes::naca4(code, per_side).ok_or_else(|| bad(cmd, "`naca` must be four digits with a thickness above 0, such as 2412 or 0012"))?;
            (pts, format!("NACA {}", code.trim()))
        }
    };
    if pts.len() > 499 {
        return Err(bad(cmd, "the airfoil has more than 499 points"));
    }
    let chord = length(s, cmd, p, "chord", 100.0, 1.0, 100_000.0)?;
    let span = length(s, cmd, p, "span", 0.0, 0.0, 100_000.0)?;
    let plane = plane(cmd, p)?;
    let scaled: Vec<Value> = pts.iter().map(|q| xy(Vec2::new(q.x * chord, q.y * chord))).collect();
    let made = run_steps(s, vec![step("sketch.create", json!({"plane": plane, "name": name})), step("sketch.spline.fit_point", json!({"points": scaled}))])?;
    let sketch = made.first().and_then(sketch_of);
    let ids = made.get(1).map(created_points).unwrap_or_default();
    let mut close = vec![];
    if let (Some(first), Some(last), Some(a), Some(b)) = (ids.first(), ids.last(), pts.first(), pts.last())
        && a.dist(*b) * chord > 1e-6
    {
        close.push(step("sketch.line", json!({"points": [last, first]})));
    }
    close.push(step("sketch.finish", json!({})));
    run_steps(s, close)?;
    let mut out = json!({"sketch": sketch, "points": pts.len()});
    if span > 0.0 {
        let body = fresh_body(s, &name);
        let r = run_one(s, "solid.extrude", json!({"sketch": sketch, "distance": span, "body_name": body, "name": name}))?;
        out["body"] = json!(body);
        out["feature"] = json!(feature_of(&r));
    }
    Ok(out)
}

// ---------------------------------------------------------------- Infill

fn infill(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "print.infill";
    let pattern = str_(p, "pattern").unwrap_or("honeycomb");
    let body_box = match str_(p, "body").filter(|b| !b.trim().is_empty()) {
        Some(b) => {
            let st = s.world_state();
            let bb = st.body(b).ok_or_else(|| bad(cmd, format!("no body `{b}`")))?.mesh().bounds();
            Some((bb.min, bb.max - bb.min))
        }
        None => None,
    };
    let size = match p.get("size") {
        None | Some(Value::Null) => body_box.map_or(Vec3::new(40.0, 40.0, 10.0), |b| b.1),
        Some(v) => vec3(v).ok_or_else(|| bad(cmd, "`size` must be [x, y, z] in mm"))?,
    };
    let at = match p.get("position") {
        None | Some(Value::Null) => body_box.map_or(Vec3::ZERO, |b| b.0),
        Some(v) => vec3(v).ok_or_else(|| bad(cmd, "`position` must be [x, y, z]"))?,
    };
    if !(size.x >= 1.0 && size.y >= 1.0 && size.z >= 0.2 && size.x <= 2000.0 && size.y <= 2000.0 && size.z <= 2000.0) {
        return Err(bad(cmd, "`size` must be 1…2000 mm each way (height from 0.2)"));
    }
    let cell = length(s, cmd, p, "cell", 8.0, 1.0, 500.0)?;
    let wall = length(s, cmd, p, "wall", 0.8, 0.2, cell * 0.8)?;
    let name = match pattern {
        "honeycomb" => "Honeycomb",
        "grid" => "Grid Infill",
        "gyroid" => "Gyroid",
        _ => return Err(bad(cmd, "`pattern` must be honeycomb, grid or gyroid")),
    };
    let body = fresh_body(s, name);
    if pattern == "gyroid" {
        let tris = shapes::gyroid_block(at, size, cell, wall, 900_000);
        return mesh_body(s, cmd, &tris, &body);
    }
    let frame = length(s, cmd, p, "frame", 2.0, 0.5, size.x.min(size.y) / 3.0)?;
    let lo = Vec2::new(at.x + frame, at.y + frame);
    let hi = Vec2::new(at.x + size.x - frame, at.y + size.y - frame);
    let mut steps = vec![
        step("sketch.create", json!({"plane": "XY", "offset": round(at.z), "name": name})),
        step("sketch.rectangle.two_point", json!({"p0": xy(Vec2::new(at.x, at.y)), "p1": xy(Vec2::new(at.x + size.x, at.y + size.y))})),
    ];
    const MAX_CELLS: usize = 400;
    let mut cells = 0;
    if pattern == "honeycomb" {
        let r = (cell - wall) / 3f64.sqrt();
        for c in shapes::hex_centres(lo, hi, cell, r, MAX_CELLS) {
            let pts: Vec<Value> = shapes::hexagon(c, r).into_iter().map(xy).collect();
            steps.push(step("sketch.line", json!({"points": pts, "closed": true, "infer": false})));
            cells += 1;
        }
    } else {
        let hole = cell - wall;
        let nx = ((hi.x - lo.x + wall) / cell).floor().max(0.0) as usize;
        let ny = ((hi.y - lo.y + wall) / cell).floor().max(0.0) as usize;
        let ox = lo.x + ((hi.x - lo.x) - (nx as f64 * cell - wall)) / 2.0;
        let oy = lo.y + ((hi.y - lo.y) - (ny as f64 * cell - wall)) / 2.0;
        'grid: for j in 0..ny {
            for i in 0..nx {
                if cells >= MAX_CELLS {
                    break 'grid;
                }
                let a = Vec2::new(ox + i as f64 * cell, oy + j as f64 * cell);
                steps.push(step("sketch.rectangle.two_point", json!({"p0": xy(a), "p1": xy(Vec2::new(a.x + hole, a.y + hole))})));
                cells += 1;
            }
        }
    }
    if cells == 0 {
        return Err(bad(cmd, "no cell fits: make the block bigger or the cells smaller"));
    }
    steps.push(step("sketch.finish", json!({})));
    let made = run_steps(s, steps)?;
    let sketch = made.first().and_then(sketch_of);
    let pick = xy(Vec2::new(at.x + frame / 2.0, at.y + frame / 2.0));
    let r = run_one(s, "solid.extrude", json!({"sketch": sketch, "profiles": [{"point": pick}], "distance": round(size.z), "body_name": body, "name": name}))?;
    Ok(json!({"body": body, "sketch": sketch, "feature": feature_of(&r), "cells": cells}))
}

// ---------------------------------------------------------------- Vent slots

fn vent(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "print.vent";
    let at = p.get("face").and_then(vec3).ok_or_else(|| bad(cmd, "`face` must be a point [x, y, z] on a flat face"))?;
    let shape = str_(p, "shape").unwrap_or("slot");
    if !matches!(shape, "round" | "rectangle" | "slot") {
        return Err(bad(cmd, "`shape` must be round, rectangle or slot"));
    }
    let cols = count(cmd, p, "columns", 6, 40)?;
    let rows = count(cmd, p, "rows", 1, 40)?;
    let w = length(s, cmd, p, "width", 4.0, 0.2, 1000.0)?;
    let h = if shape == "round" { w } else { length(s, cmd, p, "height", 20.0, 0.2, 1000.0)? };
    let px = length(s, cmd, p, "pitch_x", w + 3.0, 0.2, 2000.0)?;
    let py = length(s, cmd, p, "pitch_y", h + 3.0, 0.2, 2000.0)?;
    if (cols > 1 && px <= w) || (rows > 1 && py <= h) {
        return Err(bad(cmd, "the pitch must be larger than the opening, or the openings run into each other"));
    }
    let depth = match p.get("depth") {
        None | Some(Value::Null) => None,
        Some(Value::String(t)) if t.trim().is_empty() => None,
        _ => Some(length(s, cmd, p, "depth", 1.0, 0.01, 10_000.0)?),
    };
    let mut plane = json!({"face": xyz(at)});
    if let Some(b) = str_(p, "body") {
        plane["body"] = json!(b);
    }
    let created = run_one(s, "sketch.create", json!({"plane": plane, "name": "Vents"}))?;
    let sketch = sketch_of(&created).ok_or_else(|| EngineError::Other("the sketch was not made".into()))?;
    let frame = s.model.state().sketch(sketch).map(|k| k.plane).ok_or_else(|| EngineError::Other("the sketch has no plane".into()))?;
    let d = at - frame.origin;
    let centre = Vec2::new(d.dot(frame.x), d.dot(frame.y));
    let mut steps = Vec::new();
    let mut picks = Vec::new();
    for j in 0..rows {
        for i in 0..cols {
            let c = Vec2::new(centre.x + (i as f64 - (cols - 1) as f64 / 2.0) * px, centre.y + (j as f64 - (rows - 1) as f64 / 2.0) * py);
            picks.push(json!({"point": xy(c)}));
            steps.push(match shape {
                "round" => step("sketch.circle.center", json!({"center": xy(c), "diameter": round(w)})),
                "rectangle" => step("sketch.rectangle.two_point", json!({"p0": xy(Vec2::new(c.x - w / 2.0, c.y - h / 2.0)), "p1": xy(Vec2::new(c.x + w / 2.0, c.y + h / 2.0))})),
                _ if (w - h).abs() < 1e-9 => step("sketch.circle.center", json!({"center": xy(c), "diameter": round(w)})),
                _ if w > h => step("sketch.slot.overall", json!({"p0": xy(Vec2::new(c.x - w / 2.0, c.y)), "p1": xy(Vec2::new(c.x + w / 2.0, c.y)), "width": round(h)})),
                _ => step("sketch.slot.overall", json!({"p0": xy(Vec2::new(c.x, c.y - h / 2.0)), "p1": xy(Vec2::new(c.x, c.y + h / 2.0)), "width": round(w)})),
            });
        }
    }
    steps.push(step("sketch.finish", json!({})));
    run_steps(s, steps)?;
    let mut q = json!({"sketch": sketch, "profiles": picks, "operation": "cut", "direction": "negative", "name": "Vents"});
    match depth {
        Some(d) => q["distance"] = json!(round(d)),
        None => q["through_all"] = json!(true),
    }
    if let Some(b) = str_(p, "body") {
        q["targets"] = json!([b]);
    }
    let r = run_one(s, "solid.extrude", q)?;
    Ok(json!({"sketch": sketch, "feature": feature_of(&r), "openings": rows * cols}))
}

// ---------------------------------------------------------------- Dogbone corners

fn dogbone(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "print.dogbone";
    let picks: Vec<Vec3> = p
        .get("edges")
        .and_then(Value::as_array)
        .ok_or_else(|| bad(cmd, "`edges` must list points on inside corner edges"))?
        .iter()
        .map(|v| vec3(v).ok_or_else(|| bad(cmd, "`edges` must contain [x, y, z] points")))
        .collect::<Result<_>>()?;
    if picks.is_empty() || picks.len() > 200 {
        return Err(bad(cmd, "pick 1…200 inside corner edges"));
    }
    let diameter = length(s, cmd, p, "diameter", 3.0, 0.1, 100.0)?;
    let r = diameter / 2.0;
    let st = s.world_state();
    let mut steps = Vec::new();
    for pick in &picks {
        let mut found = None;
        for b in st.bodies.iter().filter(|b| str_(p, "body").is_none_or(|n| n == b.name)) {
            let tol = (b.body.size() * 2e-3).max(1e-3);
            let Ok(edges) = b.body.edges(tol) else { continue };
            let best = edges
                .iter()
                .filter_map(|e| {
                    let (a, z) = (*e.points.first()?, *e.points.last()?);
                    Some((pick.dist_to_segment(a, z), a, z, e.length))
                })
                .min_by(|x, y| x.0.total_cmp(&y.0));
            if let Some((dist, a, z, len)) = best
                && dist < tol * 5.0 + 0.05
                && found.as_ref().is_none_or(|f: &(f64, String, Vec3, Vec3, f64)| dist < f.0)
            {
                found = Some((dist, b.name.clone(), a, z, len));
            }
        }
        let (_, body, a, z, len) = found.ok_or_else(|| bad(cmd, "a pick is not on an edge"))?;
        if (a.dist(z) - len).abs() > 1e-3 * len.max(1.0) {
            return Err(bad(cmd, "dogbones go on straight edges"));
        }
        let Some(b) = st.body(&body) else { continue };
        let tol = (b.body.size() * 2e-3).max(1e-3);
        let dir = (z - a).normalized().ok_or_else(|| bad(cmd, "the edge has no length"))?;
        // The two flat faces that meet at the edge.
        let mut normals: Vec<Vec3> = Vec::new();
        for f in b.body.faces(tol)? {
            let Some(n) = f.plane_normal else { continue };
            let on = |q: Vec3| (q - f.centroid).dot(n).abs() < tol * 2.0;
            if n.dot(dir).abs() < 1e-3 && on(a) && on(z) && !normals.iter().any(|m| m.cross(n).len() < 1e-3) {
                normals.push(n);
            }
        }
        let [n1, n2] = normals.as_slice() else {
            return Err(bad(cmd, "a dogbone goes where two flat faces meet"));
        };
        // The open side of the corner: the one quadrant around the edge that is not material.
        let mid = a.lerp(z, 0.5);
        let mesh = b.mesh();
        let e = (r * 0.2).min(tol * 20.0).max(tol * 2.0);
        let quadrants: Vec<Vec3> = [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)]
            .iter()
            .filter_map(|(s1, s2)| (*n1 * *s1 + *n2 * *s2).normalized())
            .collect();
        let open: Vec<Vec3> = quadrants.iter().copied().filter(|q| !mesh.contains(mid + *q * e)).collect();
        let [void] = open.as_slice() else {
            return Err(bad(cmd, "dogbones go on inside corners (where a pocket's walls meet)"));
        };
        let centre = a + *void * r;
        steps.push(step(
            "solid.cylinder",
            json!({"diameter": round(diameter), "height": round(len), "base": xyz(centre), "axis": xyz(dir), "operation": "cut", "targets": [body], "name": "Dogbone"}),
        ));
    }
    let results = run_steps(s, steps)?;
    Ok(json!({"features": results.iter().filter_map(feature_of).collect::<Vec<_>>()}))
}

// ---------------------------------------------------------------- Offsets

fn offset_body(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "print.offset_body";
    let body = str_(p, "body").filter(|b| !b.trim().is_empty()).ok_or_else(|| bad(cmd, "`body` is required"))?.to_string();
    let distance = match p.get("distance") {
        Some(Value::String(t)) => s.doc.eval(t, solidcraft_doc::expr::Kind::Length).map_err(|e| bad(cmd, format!("`distance`: {e}")))?,
        _ => num(p, "distance").ok_or_else(|| bad(cmd, "`distance` must be a length"))?,
    };
    if !(distance.abs() > 1e-9 && distance.abs() <= 1000.0) {
        return Err(bad(cmd, "`distance` must be from -1000 to 1000 mm and not 0"));
    }
    let faces = super::inspect::face_refs(s, &body).map_err(|e| bad(cmd, e.to_string()))?;
    if faces.is_empty() {
        return Err(bad(cmd, format!("`{body}` has no faces")));
    }
    let r = run_one(s, "solid.offset_face", json!({"faces": faces, "distance": round(distance), "body": body, "name": "Offset Body"}))?;
    Ok(json!({"body": body, "feature": feature_of(&r), "faces": faces.len()}))
}

fn offset_sketch(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "print.offset_sketch";
    let id = s.active_sketch.ok_or_else(|| bad(cmd, "open a sketch first"))?;
    let distance = match p.get("distance") {
        Some(Value::String(t)) => s.doc.eval(t, solidcraft_doc::expr::Kind::Length).map_err(|e| bad(cmd, format!("`distance`: {e}")))?,
        _ => num(p, "distance").ok_or_else(|| bad(cmd, "`distance` must be a length"))?,
    };
    if !(distance > 1e-9 && distance <= 1000.0) {
        return Err(bad(cmd, "`distance` must be above 0 and at most 1000 mm"));
    }
    let inward = bool_(p, "inward").unwrap_or(false);
    let sk = s.doc.sketch(id)?.clone();
    // Connected chains of drawn (not construction or projected) curves.
    let drawn: Vec<usize> = (0..sk.curves.len()).filter(|i| sk.curves.get(*i).is_some_and(|c| !c.construction && !c.centerline && c.link.is_none())).collect();
    let mut group: Vec<usize> = (0..sk.curves.len()).collect();
    fn root(g: &mut [usize], mut i: usize) -> usize {
        while g[i] != i {
            g[i] = g[g[i]];
            i = g[i];
        }
        i
    }
    for (x, &i) in drawn.iter().enumerate() {
        for &j in drawn.iter().skip(x + 1) {
            let (Some(ci), Some(cj)) = (sk.curves.get(i), sk.curves.get(j)) else { continue };
            let ends = |c: &solidcraft_sketch::Curve| c.kind.ends().map(|(a, b)| vec![a, b]).unwrap_or_default();
            let touch = ends(ci).iter().any(|a| {
                ends(cj).iter().any(|b| a == b || matches!((sk.points.get(*a), sk.points.get(*b)), (Some(pa), Some(pb)) if pa.pos.dist(pb.pos) < 1e-6))
            });
            if touch {
                let (ri, rj) = (root(&mut group, i), root(&mut group, j));
                group[ri] = rj;
            }
        }
    }
    let mut chains: Vec<(usize, Vec<usize>)> = Vec::new();
    for &i in &drawn {
        let r = root(&mut group, i);
        match chains.iter_mut().find(|(k, _)| *k == r) {
            Some((_, v)) => v.push(i),
            None => chains.push((r, vec![i])),
        }
    }
    if chains.is_empty() {
        return Err(bad(cmd, "the sketch has no curves to offset"));
    }
    let mut steps = Vec::new();
    for (_, members) in &chains {
        // A point clear of the chain on its outer side.
        let mut lo = Vec2::new(f64::INFINITY, f64::INFINITY);
        let mut hi = Vec2::new(f64::NEG_INFINITY, f64::NEG_INFINITY);
        for c in members.iter().filter_map(|i| sk.curves.get(*i)) {
            let extra = match c.kind {
                solidcraft_sketch::CurveKind::Circle { r, .. } | solidcraft_sketch::CurveKind::Ellipse { r, .. } => r,
                _ => 0.0,
            };
            for q in c.kind.point_ids().iter().filter_map(|i| sk.points.get(*i)) {
                lo = Vec2::new(lo.x.min(q.pos.x - extra), lo.y.min(q.pos.y - extra));
                hi = Vec2::new(hi.x.max(q.pos.x + extra), hi.y.max(q.pos.y + extra));
            }
        }
        let span = (hi.x - lo.x).max(hi.y - lo.y).max(1.0);
        let outside = Vec2::new(hi.x + span + distance * 4.0, (lo.y + hi.y) / 2.0);
        let Some(first) = members.first().and_then(|i| sk.curves.get(*i)) else { continue };
        let d = if inward { -distance } else { distance };
        steps.push(step("sketch.offset", json!({"curves": [first.id], "distance": round(d), "side": xy(outside), "chain": true})));
    }
    let results = run_steps(s, steps)?;
    Ok(json!({"sketch": id, "chains": results.len()}))
}

// ---------------------------------------------------------------- Export for Printing

/// The settings Export for Printing reads from its .json file.
fn print_settings(path: &str) -> Value {
    let defaults = json!({
        "folder": "",
        "format": "3mf",
        "quality": "standard",
        "per": "body",
        "name": "{design} - {body}",
        "visible_only": true,
    });
    match solidcraft_io::vfs::read(path, 1 << 20).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()) {
        Some(Value::Object(o)) => {
            let mut v = defaults;
            for (k, x) in o {
                v[k] = x;
            }
            v
        }
        _ => {
            let _ = solidcraft_io::vfs::write(path, serde_json::to_string_pretty(&defaults).unwrap_or_default().as_bytes());
            defaults
        }
    }
}

fn export(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "print.export";
    let design = s.path.clone();
    let stem = design
        .as_deref()
        .and_then(|d| std::path::Path::new(d).file_stem().map(|x| x.to_string_lossy().to_string()))
        .unwrap_or_else(|| s.doc.name.clone());
    let dir = design.as_deref().and_then(|d| std::path::Path::new(d).parent().map(std::path::Path::to_path_buf));
    let config = match str_(p, "config").filter(|c| !c.trim().is_empty()) {
        Some(c) => c.to_string(),
        None => {
            let dir = dir.clone().ok_or_else(|| bad(cmd, "save the design first: the print settings and files go beside it"))?;
            dir.join(format!("{stem}.print.json")).to_string_lossy().to_string()
        }
    };
    let mut cfg = print_settings(&config);
    for k in ["folder", "format", "quality", "per", "name"] {
        if let Some(v) = p.get(k).filter(|v| !v.is_null()) {
            cfg[k] = v.clone();
        }
    }
    let format = cfg["format"].as_str().unwrap_or("3mf").to_ascii_lowercase();
    if !matches!(format.as_str(), "3mf" | "stl") {
        return Err(bad(cmd, "`format` must be 3mf or stl"));
    }
    let quality = cfg["quality"].as_str().unwrap_or("standard").to_string();
    let per_component = cfg["per"].as_str() == Some("component");
    let folder = match cfg["folder"].as_str().filter(|f| !f.trim().is_empty()) {
        Some(f) => std::path::PathBuf::from(f),
        None => {
            let base = std::path::Path::new(&config).parent().map(std::path::Path::to_path_buf).or(dir).ok_or_else(|| bad(cmd, "give `folder`"))?;
            let suffix = str_(p, "folder_suffix").filter(|x| !x.trim().is_empty()).unwrap_or("print");
            base.join(format!("{stem} - {suffix}"))
        }
    };
    let folder_text = folder.to_string_lossy().to_string();
    solidcraft_io::vfs::create_dir_all(&folder_text).map_err(|e| EngineError::Other(format!("{folder_text}: {e}")))?;
    let visible_only = cfg["visible_only"].as_bool().unwrap_or(true);
    let st = s.world_state();
    let bodies: Vec<String> = st.bodies.iter().map(|b| b.name.clone()).filter(|n| !visible_only || s.body_visible(n)).collect();
    if bodies.is_empty() {
        return Err(bad(cmd, "there are no bodies to export"));
    }
    let pattern = cfg["name"].as_str().unwrap_or("{design} - {body}").to_string();
    let safe = |t: &str| -> String { t.chars().map(|c| if "\\/:*?\"<>|".contains(c) || c.is_control() { '_' } else { c }).collect() };
    let mut files = Vec::new();
    let groups: Vec<(String, Vec<String>)> = if per_component {
        vec![(stem.clone(), bodies.clone())]
    } else {
        bodies.iter().map(|b| (b.clone(), vec![b.clone()])).collect()
    };
    for (n, (label, members)) in groups.iter().enumerate() {
        let file = pattern.replace("{design}", &stem).replace("{body}", label).replace("{n}", &(n + 1).to_string());
        let path = folder.join(format!("{}.{format}", safe(&file))).to_string_lossy().to_string();
        let mut q = json!({"path": path, "format": format, "bodies": members, "quality": quality});
        if per_component && format == "3mf" {
            q["objects"] = json!("component");
        }
        s.execute("file.export", &q)?;
        files.push(path);
    }
    Ok(json!({"files": files, "folder": folder_text, "config": config}))
}

// ---------------------------------------------------------------- Quick Send to Slicer

fn quick_send(s: &mut Session, p: &Value) -> Result<Value> {
    let cmd = "print.send";
    let slicer = str_(p, "slicer").filter(|x| !x.trim().is_empty()).ok_or_else(|| bad(cmd, "choose a slicer once with Send to Slicer"))?;
    let bodies: Vec<String> = match p.get("bodies").and_then(Value::as_array) {
        Some(a) => a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect(),
        None => {
            let picked: Vec<String> = s.selection.iter().filter_map(|x| if let crate::Sel::Body { name } = x { Some(name.clone()) } else { None }).collect();
            if picked.is_empty() { s.world_state().bodies.iter().map(|b| b.name.clone()).filter(|n| s.body_visible(n)).collect() } else { picked }
        }
    };
    if bodies.is_empty() {
        return Err(bad(cmd, "there are no bodies to send"));
    }
    let mut q = json!({"slicer": slicer, "bodies": bodies});
    for k in ["wait", "single_instance", "format"] {
        if let Some(v) = p.get(k) {
            q[k] = v.clone();
        }
    }
    s.execute("file.send_to_slicer", &q)
}

#[cfg(test)]
#[path = "print3d_tests.rs"]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests;

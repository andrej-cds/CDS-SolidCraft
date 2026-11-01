//! The 3D PRINT tab's dialogs (CDS fork): Lego Brick, Supershape, Airfoil Profile, Infill Block,
//! Vent Slots, Dogbone Corners, Offset Body and Offset Whole Sketch. Each is a list of fields
//! that becomes the `print.*` command's parameters.

use serde_json::{Map, Value, json};
use solidcraft_engine::Sel;
use solidcraft_engine::doc::expr::Kind as ValueKind;

use crate::SolveApp;
use crate::dialogs::Kind;
use crate::selection::{BODIES, EDGES, PLANAR_FACES, SelInput};
use crate::ui_kit::{choice_row, expr_row, hint_row};

/// One field of a dialog.
#[derive(Clone, Debug)]
pub enum Field {
    /// A value in the design's expressions: `Unitless` ones go to the command as numbers.
    Value { key: &'static str, label: &'static str, value: String, kind: ValueKind, hint: &'static str },
    /// One of a few choices: (the command's value, the label shown).
    Choice { key: &'static str, label: &'static str, options: &'static [(&'static str, &'static str)], pick: usize },
    Flag { key: &'static str, label: &'static str, on: bool },
    /// Free text, sent as it is.
    Text { key: &'static str, label: &'static str, value: String },
}

#[derive(Clone, Debug)]
pub struct Pr {
    pub cmd: &'static str,
    pub title: &'static str,
    pub fields: Vec<Field>,
}

fn v(key: &'static str, label: &'static str, value: &str, kind: ValueKind) -> Field {
    Field::Value { key, label, value: value.into(), kind, hint: "" }
}

fn hinted(key: &'static str, label: &'static str, kind: ValueKind, hint: &'static str) -> Field {
    Field::Value { key, label, value: String::new(), kind, hint }
}

fn n(key: &'static str, label: &'static str, value: &str) -> Field {
    v(key, label, value, ValueKind::Unitless)
}

fn mm(key: &'static str, label: &'static str, value: &str) -> Field {
    v(key, label, value, ValueKind::Length)
}

const LEGO_KINDS: &[(&str, &str)] = &[("brick", "Brick"), ("plate", "Plate"), ("tile", "Tile")];
const INFILLS: &[(&str, &str)] = &[("honeycomb", "Honeycomb"), ("grid", "Grid"), ("gyroid", "Gyroid (mesh)")];
const VENTS: &[(&str, &str)] = &[("slot", "Slot"), ("rectangle", "Rectangle"), ("round", "Round")];
const AIRFOIL_PLANES: &[(&str, &str)] = &[("XY", "XY"), ("XZ", "XZ"), ("YZ", "YZ")];

/// The dialog for a `print.*` command.
pub fn start(_app: &SolveApp, id: &str) -> Option<(Kind, Vec<SelInput>)> {
    let (title, fields, inputs): (&'static str, Vec<Field>, Vec<SelInput>) = match id {
        "print.lego" => (
            "LEGO BRICK",
            vec![
                n("studs_x", "Studs along X", "4"),
                n("studs_y", "Studs along Y", "2"),
                Field::Choice { key: "kind", label: "Type", options: LEGO_KINDS, pick: 0 },
                mm("clearance", "Printing clearance", "0.1 mm"),
            ],
            vec![],
        ),
        "print.supershape" => (
            "SUPERSHAPE",
            vec![
                n("m", "Symmetry (m)", "6"),
                n("n1", "Shape n1", "1"),
                n("n2", "Shape n2", "1.7"),
                n("n3", "Shape n3", "1.7"),
                n("a", "Stretch a", "1"),
                n("b", "Stretch b", "1"),
                mm("size", "Size", "50 mm"),
                mm("height", "Height (0: sketch only)", "10 mm"),
                Field::Flag { key: "solid", label: "3D shape (mesh body)", on: false },
            ],
            vec![],
        ),
        "print.airfoil" => (
            "AIRFOIL PROFILE",
            vec![
                Field::Text { key: "naca", label: "NACA (4 digits)", value: "2412".into() },
                Field::Text { key: "path", label: "Or a .dat file", value: String::new() },
                mm("chord", "Chord", "100 mm"),
                mm("span", "Span (0: sketch only)", "0 mm"),
                n("points", "Points per side", "60"),
                Field::Choice { key: "plane", label: "Plane", options: AIRFOIL_PLANES, pick: 0 },
            ],
            vec![],
        ),
        "print.infill" => (
            "INFILL BLOCK",
            vec![
                Field::Choice { key: "pattern", label: "Pattern", options: INFILLS, pick: 0 },
                hinted("size_x", "Length", ValueKind::Length, "the body's size"),
                hinted("size_y", "Width", ValueKind::Length, "the body's size"),
                hinted("size_z", "Height", ValueKind::Length, "the body's size"),
                mm("cell", "Cell size", "8 mm"),
                mm("wall", "Wall thickness", "0.8 mm"),
                mm("frame", "Border", "2 mm"),
            ],
            vec![SelInput::new("Body", BODIES, false)],
        ),
        "print.vent" => (
            "VENT SLOTS",
            vec![
                Field::Choice { key: "shape", label: "Shape", options: VENTS, pick: 0 },
                n("columns", "Columns", "6"),
                n("rows", "Rows", "1"),
                mm("width", "Opening width", "4 mm"),
                mm("height", "Opening height", "20 mm"),
                mm("pitch_x", "Spacing across", "7 mm"),
                mm("pitch_y", "Spacing down", "23 mm"),
                hinted("depth", "Depth", ValueKind::Length, "through all"),
            ],
            vec![SelInput::new("Face", PLANAR_FACES, false)],
        ),
        "print.dogbone" => ("DOGBONE CORNERS", vec![mm("diameter", "Tool diameter", "3 mm")], vec![SelInput::new("Edges", EDGES, true)]),
        "print.offset_body" => (
            "OFFSET BODY",
            vec![mm("distance", "Distance (negative shrinks)", "-0.2 mm")],
            vec![SelInput::new("Body", BODIES, false)],
        ),
        "print.offset_sketch" => (
            "OFFSET WHOLE SKETCH",
            vec![mm("distance", "Distance", "0.2 mm"), Field::Flag { key: "inward", label: "Inward", on: false }],
            vec![],
        ),
        _ => return None,
    };
    let cmd = solidcraft_engine::find_command(id)?.id;
    Some((Kind::Print(Pr { cmd, title, fields }), inputs))
}

/// The dialog's rows. Returns whether Enter was pressed in a field.
pub fn rows(ui: &mut egui::Ui, k: &mut Pr) -> bool {
    let mut enter = false;
    for f in k.fields.iter_mut() {
        match f {
            Field::Value { label, value, hint, .. } if !hint.is_empty() => enter |= hint_row(ui, label, value, crate::i18n::tr_label(hint)),
            Field::Value { label, value, .. } | Field::Text { label, value, .. } => enter |= expr_row(ui, label, value),
            Field::Choice { key, label, options, pick } => {
                let shown: Vec<&str> = options.iter().map(|(_, l)| crate::i18n::tr_label(l)).collect();
                choice_row(ui, label, &format!("print_{key}"), pick, &shown);
            }
            Field::Flag { label, on, .. } => {
                crate::ui_kit::checkbox(ui, crate::i18n::tr_label(label), on);
            }
        }
    }
    enter
}

fn first(inputs: &[SelInput]) -> Option<&Sel> {
    inputs.first().and_then(|i| i.items.first())
}

/// The command and its parameters.
pub fn commands(app: &SolveApp, k: &Pr, inputs: &[SelInput]) -> Result<Vec<(String, Value)>, String> {
    let mut p = Map::new();
    let mut size: [Option<f64>; 3] = [None; 3];
    for f in &k.fields {
        match f {
            Field::Value { key, value, kind, .. } => {
                let text = value.trim();
                if text.is_empty() {
                    continue;
                }
                let at = match *key {
                    "size_x" => Some(0),
                    "size_y" => Some(1),
                    "size_z" => Some(2),
                    _ => None,
                };
                if let Some(i) = at {
                    size[i] = Some(app.session.doc.eval(text, ValueKind::Length).map_err(|e| e.to_string())?);
                } else if *kind == ValueKind::Unitless {
                    p.insert((*key).into(), json!(app.session.doc.eval(text, ValueKind::Unitless).map_err(|e| e.to_string())?));
                } else {
                    p.insert((*key).into(), json!(text));
                }
            }
            Field::Choice { key, options, pick, .. } => {
                p.insert((*key).into(), json!(options.get(*pick).map_or("", |o| o.0)));
            }
            Field::Flag { key, on, .. } => {
                p.insert((*key).into(), json!(on));
            }
            Field::Text { key, value, .. } => {
                if !value.trim().is_empty() {
                    p.insert((*key).into(), json!(value.trim()));
                }
            }
        }
    }
    match k.cmd {
        "print.infill" => {
            if let Some(Sel::Body { name }) = first(inputs) {
                p.insert("body".into(), json!(name));
            }
            match size {
                [Some(x), Some(y), Some(z)] => {
                    p.insert("size".into(), json!([x, y, z]));
                }
                [None, None, None] if p.contains_key("body") => {}
                [None, None, None] => {
                    p.insert("size".into(), json!([40.0, 40.0, 10.0]));
                }
                _ => return Err("give the length, width and height, or leave all three empty for the body's size".into()),
            }
        }
        "print.airfoil" => {
            if p.contains_key("path") {
                p.remove("naca");
            }
        }
        "print.vent" => match first(inputs) {
            Some(Sel::Face { body, point, .. }) => {
                p.insert("face".into(), json!([point.x, point.y, point.z]));
                p.insert("body".into(), json!(body));
            }
            _ => return Err("select a flat face first".into()),
        },
        "print.dogbone" => {
            let edges: Vec<Value> = inputs
                .first()
                .map(|i| i.items.iter().filter_map(|x| if let Sel::Edge { point, .. } = x { Some(json!([point.x, point.y, point.z])) } else { None }).collect())
                .unwrap_or_default();
            if edges.is_empty() {
                return Err("select the inside corner edges first".into());
            }
            p.insert("edges".into(), Value::Array(edges));
        }
        "print.offset_body" => match first(inputs) {
            Some(Sel::Body { name }) => {
                p.insert("body".into(), json!(name));
            }
            Some(Sel::Face { body, .. } | Sel::Edge { body, .. }) => {
                p.insert("body".into(), json!(body));
            }
            _ => return Err("select a body first".into()),
        },
        _ => {}
    }
    Ok(vec![(k.cmd.to_string(), Value::Object(p))])
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    #[test]
    fn every_print_dialog_builds_its_command() {
        let mut h = crate::scenario::Harness::new();
        h.app.run("solid.box", json!({"length": 20, "width": 20, "height": 4, "body_name": "Plate"})).unwrap_or_default();
        for id in ["print.lego", "print.supershape", "print.airfoil", "print.infill"] {
            let (kind, inputs) = super::start(&h.app, id).unwrap_or_else(|| panic!("{id}"));
            let crate::dialogs::Kind::Print(k) = kind else { panic!("{id}") };
            let cmds = super::commands(&h.app, &k, &inputs).unwrap_or_else(|e| panic!("{id}: {e}"));
            for (cmd, params) in cmds {
                h.app.run(&cmd, params.clone()).unwrap_or_else(|e| panic!("{cmd} {params}: {e}"));
            }
        }
        assert!(super::start(&h.app, "solid.extrude").is_none());
    }
}

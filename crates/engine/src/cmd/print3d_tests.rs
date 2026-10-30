use serde_json::{Value, json};
use solidcraft_geom::Vec3;

use crate::Session;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    match s.execute(id, &p) {
        Ok(v) => v,
        Err(e) => panic!("{id} {p}: {e}"),
    }
}

fn clean(s: &Session) {
    let failed: Vec<String> = s.model.results.iter().filter_map(|r| r.error.as_ref().map(|e| format!("{}: {e}", r.name))).collect();
    assert!(failed.is_empty(), "{failed:?}");
}

#[test]
fn a_lego_brick_has_studs_tubes_and_one_undo_step() {
    let mut s = Session::default();
    let undo = s.undo.len();
    let out = run(&mut s, "print.lego", json!({"studs_x": 4, "studs_y": 2}));
    clean(&s);
    assert_eq!(s.undo.len(), undo + 1, "one tool, one undo step");
    let body = out["body"].as_str().unwrap().to_string();
    assert_eq!(body, "Lego 4x2");
    let world = s.world_state();
    let m = world.body(&body).unwrap().mesh();
    let b = m.bounds();
    assert!((b.max.x - b.min.x - 31.8).abs() < 1e-6 && (b.max.y - b.min.y - 15.8).abs() < 1e-6);
    assert!((b.max.z - 11.4).abs() < 1e-3, "studs on top: {}", b.max.z);
    // A stud at each corner and the hollow underneath with tubes between the studs.
    assert!(m.contains(Vec3::new(3.9, 3.9, 10.5)) && m.contains(Vec3::new(27.9, 11.9, 10.5)));
    assert!(!m.contains(Vec3::new(3.9, 3.9, 4.0)), "hollow under a stud");
    assert!(m.contains(Vec3::new(7.9 + 2.9, 7.9, 4.0)), "a tube wall between the studs");
    assert!(!m.contains(Vec3::new(7.9, 7.9, 4.0)), "the tube is bored");
    run(&mut s, "edit.undo", json!({}));
    assert!(s.model.state().bodies.is_empty());
    for (x, y, kind) in [(1, 1, "plate"), (1, 6, "brick"), (2, 2, "tile")] {
        let mut s = Session::default();
        run(&mut s, "print.lego", json!({"studs_x": x, "studs_y": y, "kind": kind}));
        clean(&s);
    }
    assert!(Session::default().execute("print.lego", &json!({"studs_x": 0})).is_err());
}

#[test]
fn supershape_airfoil_and_infill_blocks_build() {
    let mut s = Session::default();
    let out = run(&mut s, "print.supershape", json!({"m": 5, "n1": 2, "n2": 7, "n3": 7, "size": 40, "height": 6}));
    clean(&s);
    let world = s.world_state();
    let b = world.body(out["body"].as_str().unwrap()).unwrap().mesh().bounds();
    assert!((b.max.z - 6.0).abs() < 1e-6 && b.max.x - b.min.x > 20.0);
    run(&mut s, "print.supershape", json!({"solid": true, "size": 30}));
    let out = run(&mut s, "print.airfoil", json!({"naca": "2412", "chord": 120, "span": 30}));
    clean(&s);
    let world = s.world_state();
    let b = world.body(out["body"].as_str().unwrap()).unwrap().mesh().bounds();
    assert!((b.max.x - b.min.x - 120.0).abs() < 0.5, "{:?}", b);
    assert!(s.execute("print.airfoil", &json!({"naca": "12"})).is_err());
    let mut s = Session::default();
    for pattern in ["honeycomb", "grid"] {
        let out = run(&mut s, "print.infill", json!({"pattern": pattern, "size": [40, 30, 5], "cell": 8, "wall": 1, "position": [0, 0, 0]}));
        clean(&s);
        let world = s.world_state();
        let m = world.body(out["body"].as_str().unwrap()).unwrap().mesh();
        assert!(m.contains(Vec3::new(0.5, 0.5, 2.5)), "the frame");
        assert!(out["cells"].as_u64().unwrap() > 4);
    }
    let out = run(&mut s, "print.infill", json!({"pattern": "gyroid", "size": [20, 20, 10], "cell": 10, "wall": 1}));
    assert!(out["triangles"].as_u64().unwrap() > 1000);
}

#[test]
fn vents_dogbones_and_offsets_change_the_body() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 60, "width": 40, "height": 4, "body_name": "Panel"}));
    let out = run(&mut s, "print.vent", json!({"face": [30, 20, 4], "shape": "slot", "columns": 5, "rows": 1, "width": 3, "height": 20, "pitch_x": 6}));
    clean(&s);
    assert_eq!(out["openings"], 5);
    let world = s.world_state();
    let m = world.body("Panel").unwrap().mesh();
    assert!(!m.contains(Vec3::new(30.0, 20.0, 2.0)), "a slot through the middle");
    assert!(m.contains(Vec3::new(30.0, 35.0, 2.0)));
    run(&mut s, "print.vent", json!({"face": [10, 10, 4], "shape": "round", "columns": 2, "rows": 2, "width": 2, "pitch_x": 4, "pitch_y": 4}));
    clean(&s);

    // A pocket's inside corner gets a dogbone.
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 40, "width": 40, "height": 10, "body_name": "Block"}));
    run(&mut s, "sketch.create", json!({"plane": "XY", "offset": 10}));
    run(&mut s, "sketch.rectangle.two_point", json!({"p0": [10, 10], "p1": [30, 30]}));
    run(&mut s, "sketch.finish", json!({}));
    run(&mut s, "solid.extrude", json!({"distance": 5, "direction": "negative", "operation": "cut"}));
    clean(&s);
    run(&mut s, "print.dogbone", json!({"edges": [[10, 10, 7.5]], "diameter": 3}));
    clean(&s);
    let world = s.world_state();
    let m = world.body("Block").unwrap().mesh();
    assert!(!m.contains(Vec3::new(9.7, 11.06, 7.5)) && !m.contains(Vec3::new(11.06, 9.7, 7.5)), "the relief runs into both walls");
    assert!(m.contains(Vec3::new(9.7, 14.0, 7.5)), "the wall stays further along");
    assert!(s.execute("print.dogbone", &json!({"edges": [[0, 0, 5]]})).is_err(), "an outside corner");

    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 20, "width": 20, "height": 10, "body_name": "Pin"}));
    run(&mut s, "print.offset_body", json!({"body": "Pin", "distance": -0.2}));
    clean(&s);
    let world = s.world_state();
    let b = world.body("Pin").unwrap().mesh().bounds();
    assert!((b.max.x - b.min.x - 19.6).abs() < 1e-6, "{:?}", b);

    run(&mut s, "sketch.create", json!({"plane": "XY", "offset": 10}));
    run(&mut s, "sketch.rectangle.two_point", json!({"p0": [0, 0], "p1": [10, 10]}));
    run(&mut s, "sketch.circle.center", json!({"center": [30, 30], "diameter": 6}));
    let out = run(&mut s, "print.offset_sketch", json!({"distance": 1}));
    assert_eq!(out["chains"], 2);
    let id = s.active_sketch.unwrap();
    assert!(s.doc.sketch(id).unwrap().curves.len() >= 10);
}

#[test]
fn printing_materials_have_printed_part_data_and_the_bom_writes_markdown() {
    let mut s = Session::default();
    run(&mut s, "solid.box", json!({"length": 10, "width": 10, "height": 10, "body_name": "Part"}));
    run(&mut s, "material.assign", json!({"bodies": ["Part"], "material": "PETG (3D print)"}));
    let v = run(&mut s, "material.inspect", json!({"material": "PETG (3D print)"}));
    assert_eq!(v["density_g_cm3"], 1.27);
    assert!(v["physical_card"]["properties"].as_array().unwrap().len() == 3);
    assert!(solidcraft_doc::physical_material::validate_card(&v["physical_card"]).is_ok());
    let dir = std::env::temp_dir().join(format!("sc-print3d-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("bom.md");
    run(&mut s, "assembly.bom", json!({"path": path.to_string_lossy(), "titles": {"made": "Izdelani deli", "purchased": "Kupljeni deli"}}));
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(text.contains("## Izdelani deli") && text.contains("## Kupljeni deli"), "{text}");
    // Export for Printing writes its settings beside the design and a file per body.
    run(&mut s, "solid.box", json!({"length": 5, "width": 5, "height": 5, "corner": [20, 0, 0], "body_name": "Clip"}));
    let design = dir.join("Holder.solidcraft");
    run(&mut s, "file.save", json!({"path": design.to_string_lossy()}));
    let out = run(&mut s, "print.export", json!({}));
    assert_eq!(out["files"].as_array().unwrap().len(), 2);
    assert!(dir.join("Holder.print.json").exists());
    assert!(dir.join("Holder - print").join("Holder - Clip.3mf").exists());
    let _ = std::fs::remove_dir_all(&dir);
}

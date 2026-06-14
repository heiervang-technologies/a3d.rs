//! Structural invariants for the OBJ loader against the committed sample models.
//! Complements the rendering snapshot tests (which only cover bunny/cow/teapot)
//! by also exercising dragon.obj and asserting mesh-level properties directly.
use std::path::Path;

use a3d::model::load_model;

fn assert_valid_mesh(path: &str) {
    let mesh = load_model(Path::new(path));

    assert!(!mesh.vertices.is_empty(), "{path}: loaded no vertices");
    assert!(!mesh.indices.is_empty(), "{path}: loaded no indices");
    assert_eq!(
        mesh.indices.len() % 3,
        0,
        "{path}: index count {} is not a multiple of 3 (not triangulated)",
        mesh.indices.len()
    );

    let n = mesh.vertices.len() as u32;
    assert!(
        mesh.indices.iter().all(|&i| i < n),
        "{path}: an index references a vertex out of range (n={n})"
    );

    // load_model normalizes to the unit sphere, so the farthest vertex from the
    // origin sits at distance ~1.
    let max_dist = mesh
        .vertices
        .iter()
        .map(|v| {
            let [x, y, z] = v.position;
            (x * x + y * y + z * z).sqrt()
        })
        .fold(0.0f32, f32::max);
    assert!(
        (max_dist - 1.0).abs() < 1e-3,
        "{path}: not unit-normalized (max distance from origin = {max_dist})"
    );
}

#[test]
fn loads_all_sample_models() {
    for model in ["bunny", "cow", "teapot", "dragon"] {
        assert_valid_mesh(&format!("models/{model}.obj"));
    }
}

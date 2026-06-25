//! Coverage for the STL loader path, which has no committed fixture model.
//! Writes a minimal one-triangle binary STL to a temp file and loads it.
use std::io::Write;
use std::path::PathBuf;

use a3d::model::load_model;

/// Build a valid binary STL containing a single triangle.
/// Layout: 80-byte header, u32 triangle count, then per triangle
/// (3 normal + 9 vertex floats, little-endian) + u16 attribute bytes.
fn one_triangle_binary_stl() -> Vec<u8> {
    let mut bytes = vec![0u8; 80];
    bytes.extend_from_slice(&1u32.to_le_bytes());
    let floats: [f32; 12] = [
        0.0, 0.0, 1.0, // face normal
        0.0, 0.0, 0.0, // v0
        1.0, 0.0, 0.0, // v1
        0.0, 1.0, 0.0, // v2
    ];
    for f in floats {
        bytes.extend_from_slice(&f.to_le_bytes());
    }
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes
}

fn write_temp(name: &str, bytes: &[u8]) -> PathBuf {
    let path = std::env::temp_dir().join(format!("a3d_{}_{}.stl", name, std::process::id()));
    let mut f = std::fs::File::create(&path).expect("create temp stl");
    f.write_all(bytes).expect("write temp stl");
    path
}

#[test]
fn loads_binary_stl() {
    let path = write_temp("one_tri", &one_triangle_binary_stl());
    let mesh = load_model(&path).expect("synthetic STL should load");
    let _ = std::fs::remove_file(&path);

    assert_eq!(mesh.vertices.len(), 3, "expected 3 vertices");
    assert_eq!(mesh.indices, vec![0, 1, 2], "expected one triangle");

    // load_model normalizes to the unit sphere.
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
        "not unit-normalized (max distance = {max_dist})"
    );
}

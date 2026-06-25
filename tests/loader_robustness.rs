//! The loader parses untrusted OBJ/STL files (see `SECURITY.md`), so it must
//! surface bad input as an error and never panic, hang, or allocate unboundedly.
//! This throws a spread of adversarial byte sequences at `load_model` through
//! both extensions and asserts only that it returns (Ok or Err — both fine).
use std::io::Write;
use std::path::PathBuf;

use a3d::model::load_model;

fn write_temp(name: &str, bytes: &[u8]) -> PathBuf {
    let path = std::env::temp_dir().join(name);
    let mut f = std::fs::File::create(&path).expect("create temp file");
    f.write_all(bytes).expect("write temp file");
    path
}

fn adversarial_cases() -> Vec<Vec<u8>> {
    let mut cases: Vec<Vec<u8>> = vec![
        vec![],                                             // empty
        b"solid header with no facets".to_vec(),            // truncated ASCII STL
        b"not a 3d model at all, just prose".to_vec(),      // junk text
        b"v 1 2 3\nf 1 2 3\n".to_vec(),                     // OBJ: face refs missing verts
        b"v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 99\n".to_vec(),  // OBJ: out-of-range index
        b"v nan nan nan\nf 1 1 1\n".to_vec(),               // OBJ: NaN coordinates
        b"v inf 0 0\nv 0 1 0\nv 1 0 0\nf 1 2 3\n".to_vec(), // OBJ: infinite coordinate
        b"f 1 2 3\n".to_vec(),                              // OBJ: face, no vertices
    ];

    // Binary STL: 80-byte header + a triangle count of u32::MAX, with no data.
    // Must error fast, not pre-allocate billions of triangles.
    let mut huge_count = vec![0u8; 80];
    huge_count.extend_from_slice(&u32::MAX.to_le_bytes());
    cases.push(huge_count);

    // Binary STL: header + count=1 but the triangle record is truncated.
    let mut truncated = vec![0u8; 80];
    truncated.extend_from_slice(&1u32.to_le_bytes());
    truncated.extend_from_slice(&[0u8; 12]); // a full record is 50 bytes
    cases.push(truncated);

    // Deterministic pseudo-random bytes (no RNG — `Math.random` is unavailable
    // anyway, and this keeps the test reproducible).
    cases.push(
        (0..1024u32)
            .map(|i| (i.wrapping_mul(2_654_435_761) >> 24) as u8)
            .collect(),
    );

    cases
}

#[test]
fn loader_never_panics_on_adversarial_input() {
    for (i, bytes) in adversarial_cases().into_iter().enumerate() {
        for ext in ["stl", "obj"] {
            let path = write_temp(&format!("a3d_robustness_{i}.{ext}"), &bytes);
            // The contract is "returns, never panics" — Ok and Err both pass.
            let _ = load_model(&path);
            let _ = std::fs::remove_file(&path);
        }
    }
}

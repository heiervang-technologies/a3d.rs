//! End-to-end coverage for terminal-independent single-frame rendering.

use std::process::Command;

#[test]
fn renders_exact_requested_frame_dimensions() {
    let output = Command::new(env!("CARGO_BIN_EXE_a3d"))
        .args(["models/dog.stl", "--frame", "8x6", "--time", "3.5"])
        .output()
        .expect("a3d should launch");

    assert!(
        output.status.success(),
        "a3d failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty(), "single-frame mode wrote stderr");

    let stdout = String::from_utf8(output.stdout).expect("frame should be UTF-8");
    let rows: Vec<_> = stdout.lines().collect();
    assert_eq!(rows.len(), 6, "frame has the wrong height: {stdout:?}");
    assert!(
        rows.iter().all(|row| row.chars().count() == 8),
        "frame has the wrong width: {stdout:?}"
    );
    assert!(
        rows.iter().any(|row| row.trim().len() > 1),
        "frame unexpectedly contains no visible model: {stdout:?}"
    );
}

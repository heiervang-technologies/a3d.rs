use std::path::Path;

use glam::Vec3;

use a3d::model::load_model;
use a3d::render::Framebuffer;
use a3d::{framebuffer_to_string, render_frame};

const LIGHT_DIR: Vec3 = Vec3::new(0.70710677, -0.70710677, 0.0);

/// Render a model at given parameters and return the ASCII string.
fn render_snapshot(
    model_path: &str,
    width: usize,
    height: usize,
    azimuth: f32,
    altitude: f32,
    zoom: f32,
) -> String {
    let mesh = load_model(Path::new(model_path));
    let mut fb = Framebuffer::new(width, height);
    render_frame(&mut fb, &mesh, azimuth, altitude, zoom, LIGHT_DIR, None);
    framebuffer_to_string(&fb)
}

#[test]
fn bunny_80x24_snapshot() {
    let output = render_snapshot("models/bunny.obj", 80, 24, 0.0, 0.0, 1.0);
    let expected = include_str!("snapshots/bunny_80x24.txt");
    assert_eq!(
        output, expected,
        "Bunny 80x24 snapshot mismatch — rendering has regressed"
    );
}

#[test]
fn bunny_80x24_rotated_snapshot() {
    let output = render_snapshot("models/bunny.obj", 80, 24, 1.0, 0.3, 1.0);
    let expected = include_str!("snapshots/bunny_80x24_rotated.txt");
    assert_eq!(
        output, expected,
        "Bunny 80x24 rotated snapshot mismatch — rendering has regressed"
    );
}

#[test]
fn cow_80x24_snapshot() {
    let output = render_snapshot("models/cow.obj", 80, 24, 0.0, 0.0, 1.0);
    let expected = include_str!("snapshots/cow_80x24.txt");
    assert_eq!(
        output, expected,
        "Cow 80x24 snapshot mismatch — rendering has regressed"
    );
}

#[test]
fn teapot_80x24_snapshot() {
    let output = render_snapshot("models/teapot.obj", 80, 24, 0.0, 0.0, 1.0);
    let expected = include_str!("snapshots/teapot_80x24.txt");
    assert_eq!(
        output, expected,
        "Teapot 80x24 snapshot mismatch — rendering has regressed"
    );
}

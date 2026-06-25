use std::path::Path;

use glam::Vec3;

use a3d::model::load_model;
use a3d::render::Framebuffer;
use a3d::{RenderParams, framebuffer_to_string, render_frame};

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
    let mesh = load_model(Path::new(model_path)).expect("model should load");
    let mut fb = Framebuffer::new(width, height);
    let params = RenderParams {
        azimuth,
        altitude,
        zoom,
        light_dir: LIGHT_DIR,
        fg_override: None,
    };
    render_frame(&mut fb, &mesh, &params);
    framebuffer_to_string(&fb)
}

#[test]
fn dog_80x24_snapshot() {
    let output = render_snapshot("models/dog.stl", 80, 24, 0.0, 0.0, 1.0);
    let expected = include_str!("snapshots/dog_80x24.txt");
    assert_eq!(
        output, expected,
        "Dog 80x24 snapshot mismatch — rendering has regressed"
    );
}

#[test]
fn dog_80x24_rotated_snapshot() {
    let output = render_snapshot("models/dog.stl", 80, 24, 1.0, 0.3, 1.0);
    let expected = include_str!("snapshots/dog_80x24_rotated.txt");
    assert_eq!(
        output, expected,
        "Dog 80x24 rotated snapshot mismatch — rendering has regressed"
    );
}

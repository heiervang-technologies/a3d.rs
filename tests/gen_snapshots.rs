/// Run with: cargo test --test gen_snapshots -- --ignored
/// Regenerates snapshot files in tests/snapshots/
use std::path::Path;

use glam::Vec3;

use a3d::model::load_model;
use a3d::render::Framebuffer;
use a3d::{framebuffer_to_string, render_frame};

const LIGHT_DIR: Vec3 = Vec3::new(0.70710677, -0.70710677, 0.0);

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
#[ignore]
fn generate_snapshots() {
    std::fs::create_dir_all("tests/snapshots").unwrap();

    let dog = render_snapshot("models/dog.stl", 80, 24, 0.0, 0.0, 1.0);
    std::fs::write("tests/snapshots/dog_80x24.txt", &dog).unwrap();
    println!("Wrote dog_80x24.txt");

    let dog_rot = render_snapshot("models/dog.stl", 80, 24, 1.0, 0.3, 1.0);
    std::fs::write("tests/snapshots/dog_80x24_rotated.txt", &dog_rot).unwrap();
    println!("Wrote dog_80x24_rotated.txt");
}

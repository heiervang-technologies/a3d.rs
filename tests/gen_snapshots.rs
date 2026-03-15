/// Run with: cargo test --test gen_snapshots -- --ignored
/// Regenerates snapshot files in tests/snapshots/
use std::path::Path;

use glam::Vec3;

use a3d::model::load_model;
use a3d::render::Framebuffer;
use a3d::{framebuffer_to_string, render_frame};

const LIGHT_DIR: Vec3 = Vec3::new(0.70710677, -0.70710677, 0.0);

fn render_snapshot(model_path: &str, width: usize, height: usize, azimuth: f32, altitude: f32, zoom: f32) -> String {
    let mesh = load_model(Path::new(model_path));
    let mut fb = Framebuffer::new(width, height);
    render_frame(&mut fb, &mesh, azimuth, altitude, zoom, LIGHT_DIR, None);
    framebuffer_to_string(&fb)
}

#[test]
#[ignore]
fn generate_snapshots() {
    std::fs::create_dir_all("tests/snapshots").unwrap();

    let bunny = render_snapshot("models/bunny.obj", 80, 24, 0.0, 0.0, 1.0);
    std::fs::write("tests/snapshots/bunny_80x24.txt", &bunny).unwrap();
    println!("Wrote bunny_80x24.txt");

    let bunny_rot = render_snapshot("models/bunny.obj", 80, 24, 1.0, 0.3, 1.0);
    std::fs::write("tests/snapshots/bunny_80x24_rotated.txt", &bunny_rot).unwrap();
    println!("Wrote bunny_80x24_rotated.txt");

    let cow = render_snapshot("models/cow.obj", 80, 24, 0.0, 0.0, 1.0);
    std::fs::write("tests/snapshots/cow_80x24.txt", &cow).unwrap();
    println!("Wrote cow_80x24.txt");

    let teapot = render_snapshot("models/teapot.obj", 80, 24, 0.0, 0.0, 1.0);
    std::fs::write("tests/snapshots/teapot_80x24.txt", &teapot).unwrap();
    println!("Wrote teapot_80x24.txt");
}

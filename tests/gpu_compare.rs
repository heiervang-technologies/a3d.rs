/// Compare GPU vs CPU rendering output.
/// Run with: cargo test --test gpu_compare -- --nocapture
use std::path::Path;

use glam::Vec3;

use a3d::gpu::GpuContext;
use a3d::gpu::RasterPipeline;
use a3d::model::load_model;
use a3d::render::Framebuffer;
use a3d::{framebuffer_to_string, render_frame, render_frame_gpu};

const LIGHT_DIR: Vec3 = Vec3::new(0.70710677, -0.70710677, 0.0);

#[test]
fn gpu_vs_cpu_bunny() {
    let mesh = load_model(Path::new("models/bunny.obj"));
    let width = 80usize;
    let height = 24usize;
    let azimuth = 0.0f32;
    let altitude = 0.0f32;
    let zoom = 1.0f32;

    // CPU render
    let mut cpu_fb = Framebuffer::new(width, height);
    render_frame(&mut cpu_fb, &mesh, azimuth, altitude, zoom, LIGHT_DIR, None);
    let cpu_str = framebuffer_to_string(&cpu_fb);

    // GPU render
    let ctx = pollster::block_on(GpuContext::new()).expect("No GPU available");
    let pipeline = RasterPipeline::new(&ctx, &mesh, width as u32, height as u32);
    let mut gpu_fb = Framebuffer::new(width, height);
    render_frame_gpu(&mut gpu_fb, &pipeline, &ctx, azimuth, altitude, zoom, LIGHT_DIR, None);
    let gpu_str = framebuffer_to_string(&gpu_fb);

    println!("=== CPU ===");
    println!("{}", cpu_str);
    println!();
    println!("=== GPU ===");
    println!("{}", gpu_str);

    // Count differences
    let mut diffs = 0;
    for (i, (c, g)) in cpu_str.chars().zip(gpu_str.chars()).enumerate() {
        if c != g {
            diffs += 1;
            if diffs <= 20 {
                let row = i / (width + 1);
                let col = i % (width + 1);
                println!("DIFF at ({}, {}): CPU='{}' GPU='{}'", col, row, c, g);
            }
        }
    }
    println!("Total diffs: {} / {}", diffs, cpu_str.len());
}

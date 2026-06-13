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
const W: usize = 80;
const H: usize = 24;

/// Render `model` on both backends and return (cpu_str, gpu_str, diff_ratio).
/// Returns `None` when no GPU adapter is available, so callers can skip
/// gracefully on GPU-less CI runners instead of failing the suite.
fn compare(model: &str, az: f32, al: f32, zoom: f32) -> Option<(String, String, f64)> {
    let mesh = load_model(Path::new(model));

    let mut cpu_fb = Framebuffer::new(W, H);
    render_frame(&mut cpu_fb, &mesh, az, al, zoom, LIGHT_DIR, None);
    let cpu_str = framebuffer_to_string(&cpu_fb);

    let ctx = pollster::block_on(GpuContext::new())?;
    let pipeline = RasterPipeline::new(&ctx, &mesh, W as u32, H as u32);
    let mut gpu_fb = Framebuffer::new(W, H);
    render_frame_gpu(&mut gpu_fb, &pipeline, &ctx, az, al, zoom, LIGHT_DIR, None);
    let gpu_str = framebuffer_to_string(&gpu_fb);

    let total = cpu_str.chars().count();
    let diffs = cpu_str
        .chars()
        .zip(gpu_str.chars())
        .filter(|(c, g)| c != g)
        .count();
    Some((cpu_str, gpu_str, diffs as f64 / total as f64))
}

#[test]
fn gpu_vs_cpu_bunny() {
    let Some((cpu_str, gpu_str, ratio)) = compare("models/bunny.obj", 0.0, 0.0, 1.0) else {
        eprintln!("Skipping gpu_vs_cpu_bunny: no GPU adapter available");
        return;
    };

    println!("=== CPU ===\n{cpu_str}\n\n=== GPU ===\n{gpu_str}");
    println!("diff ratio: {:.2}%", ratio * 100.0);

    // The GPU pipeline mirrors the CPU rasterizer, so at zoom 1.0 outputs should
    // be nearly identical — only a few edge pixels differ from the two-pass
    // depth recompute (see issue tracking the single-pass fix). Observed ~0.05%.
    assert!(
        cpu_str.chars().any(|c| c != ' ' && c != '\n'),
        "CPU render is blank — test fixture is broken"
    );
    assert!(
        ratio < 0.01,
        "GPU diverged from CPU by {:.2}% — exceeds 1% tolerance",
        ratio * 100.0
    );
}

/// Regression guard for the depth-vs-zoom bug: the GPU clamps quantized depth to
/// [0,1], so when projected z was scaled by zoom it spilled out of range and
/// collapsed the z-ordering (measured up to ~5% divergence at zoom 4). With z
/// kept zoom-independent the divergence stays ~1% across zoom levels. The 3%
/// ceiling catches a regression of that bug while tolerating the residual
/// two-pass jitter (which grows mildly with overlap at high zoom).
#[test]
fn gpu_vs_cpu_zoom_sweep() {
    let mut ran = false;
    for model in ["models/bunny.obj", "models/cow.obj", "models/teapot.obj"] {
        for zoom in [1.5f32, 2.5, 4.0] {
            let Some((_, _, ratio)) = compare(model, 0.6, 0.3, zoom) else {
                eprintln!("Skipping gpu_vs_cpu_zoom_sweep: no GPU adapter available");
                return;
            };
            ran = true;
            println!("{model} @ zoom {zoom}: {:.2}%", ratio * 100.0);
            assert!(
                ratio < 0.03,
                "{model} @ zoom {zoom}: GPU diverged from CPU by {:.2}% — exceeds 3% \
                 (depth-vs-zoom regression?)",
                ratio * 100.0
            );
        }
    }
    assert!(ran, "zoom sweep ran no cases");
}

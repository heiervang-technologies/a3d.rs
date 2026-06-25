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
fn gpu_vs_cpu_dog() {
    let Some((cpu_str, gpu_str, ratio)) = compare("models/dog.stl", 0.0, 0.0, 1.0) else {
        eprintln!("Skipping gpu_vs_cpu_dog: no GPU adapter available");
        return;
    };

    println!("=== CPU ===\n{cpu_str}\n\n=== GPU ===\n{gpu_str}");
    println!("diff ratio: {:.2}%", ratio * 100.0);

    // The GPU resolves depth with a single atomicMin over packed
    // (depth << 32 | triangle_index), and the shade pass colors a pixel only if
    // its triangle was the recorded winner — no float recompute, no tie holes —
    // so the GPU matches the CPU rasterizer pixel-for-pixel (observed 0.00% on
    // dog.stl). The small tolerance only absorbs rare cross-driver float rounding
    // at ASCII luminance-bucket boundaries.
    assert!(
        cpu_str.chars().any(|c| c != ' ' && c != '\n'),
        "CPU render is blank — test fixture is broken"
    );
    assert!(
        ratio < 0.005,
        "GPU diverged from CPU by {:.2}% — exceeds 0.5% tolerance",
        ratio * 100.0
    );
}

/// Regression guard for the depth-vs-zoom bug: the GPU clamps quantized depth to
/// [0,1], so when projected z was scaled by zoom it spilled out of range and
/// collapsed the z-ordering, with divergence growing sharply with zoom. With z
/// kept zoom-independent the front-facing pose stays pixel-identical and flat
/// across zoom — so a reintroduction of that bug, which blows the depth buffer
/// out of range at high zoom, trips this ceiling.
///
/// With depth resolved by the single-pass atomicMin packing, GPU and CPU agree
/// exactly here (observed 0.00% at every zoom); the 1% ceiling leaves only a
/// little headroom for cross-driver float rounding while still catching a
/// depth-vs-zoom regression, which spikes divergence well past it.
#[test]
fn gpu_vs_cpu_zoom_sweep() {
    let mut ran = false;
    for model in ["models/dog.stl"] {
        for zoom in [1.5f32, 2.5, 4.0] {
            let Some((_, _, ratio)) = compare(model, 0.0, 0.0, zoom) else {
                eprintln!("Skipping gpu_vs_cpu_zoom_sweep: no GPU adapter available");
                return;
            };
            ran = true;
            println!("{model} @ zoom {zoom}: {:.2}%", ratio * 100.0);
            assert!(
                ratio < 0.01,
                "{model} @ zoom {zoom}: GPU diverged from CPU by {:.2}% — exceeds 1% \
                 (depth-vs-zoom regression?)",
                ratio * 100.0
            );
        }
    }
    assert!(ran, "zoom sweep ran no cases");
}

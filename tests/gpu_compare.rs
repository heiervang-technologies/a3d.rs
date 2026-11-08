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
    compare_at_size(model, az, al, zoom, W, H)
}

fn compare_at_size(
    model: &str,
    az: f32,
    al: f32,
    zoom: f32,
    width: usize,
    height: usize,
) -> Option<(String, String, f64)> {
    let mesh = load_model(Path::new(model)).expect("model should load");

    let mut cpu_fb = Framebuffer::new(width, height);
    render_frame(&mut cpu_fb, &mesh, az, al, zoom, LIGHT_DIR, None);
    let cpu_str = framebuffer_to_string(&cpu_fb);

    let ctx = pollster::block_on(GpuContext::new())?;
    let pipeline = RasterPipeline::new(&ctx, &mesh, width as u32, height as u32).unwrap();
    let mut gpu_fb = Framebuffer::new(width, height);
    render_frame_gpu(&mut gpu_fb, &pipeline, &ctx, az, al, zoom, LIGHT_DIR, None).unwrap();
    let gpu_str = framebuffer_to_string(&gpu_fb);

    let total = cpu_fb.chars.len();
    let diffs = cpu_fb
        .chars
        .iter()
        .zip(&gpu_fb.chars)
        .filter(|(c, g)| c != g)
        .count();
    Some((cpu_str, gpu_str, diffs as f64 / total as f64))
}

#[test]
fn gpu_vs_cpu_high_resolution() {
    let Some((cpu_str, _, ratio)) = compare_at_size("models/dog.stl", 0.7, 0.2, 1.0, 640, 192)
    else {
        eprintln!("Skipping gpu_vs_cpu_high_resolution: no GPU adapter available");
        return;
    };

    assert!(
        cpu_str.chars().any(|c| c != ' ' && c != '\n'),
        "CPU render is blank — test fixture is broken"
    );
    assert!(
        ratio < 0.005,
        "high-resolution GPU render diverged from CPU by {:.2}%",
        ratio * 100.0
    );
}

#[test]
fn gpu_vs_cpu_dog() {
    let Some((cpu_str, gpu_str, ratio)) = compare("models/dog.stl", 0.0, 0.0, 1.0) else {
        eprintln!("Skipping gpu_vs_cpu_dog: no GPU adapter available");
        return;
    };

    println!("=== CPU ===\n{cpu_str}\n\n=== GPU ===\n{gpu_str}");
    println!("diff ratio: {:.2}%", ratio * 100.0);

    // Packed depth/index ownership avoids float-equality holes. The observed
    // sample output often matches exactly, but this is a tolerance check:
    // cross-driver rounding and quantization are not guaranteed identical.
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
/// The 1% ceiling allows small cross-driver differences while still catching
/// the much larger divergence caused by scaling depth with zoom.
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

/// Synthetic overlapping colored triangles exercise occlusion, flat vertex
/// colors, foreground overrides, and the documented depth-readback boundary.
#[test]
fn colored_occlusion_matches_cpu_contract() {
    use a3d::model::{Mesh, mesh::Vertex};
    let Some(ctx) = pollster::block_on(GpuContext::new()) else {
        eprintln!("Skipping colored occlusion: no suitable adapter");
        return;
    };
    let mut mesh = Mesh {
        vertices: Vec::new(),
        indices: Vec::new(),
    };
    for (depth, color) in [(0.4, [1.0, 0.1, 0.2]), (-0.4, [0.2, 0.6, 0.9])] {
        let base = mesh.vertices.len() as u32;
        for (index, position) in [[-0.6, -0.6, depth], [0.0, 0.6, depth], [0.6, -0.6, depth]]
            .into_iter()
            .enumerate()
        {
            mesh.vertices.push(Vertex {
                position,
                normal: [0.0; 3],
                color: if index == 0 { color } else { [0.0, 1.0, 0.0] },
            });
        }
        mesh.indices.extend([base, base + 1, base + 2]);
    }
    let pipeline = RasterPipeline::new(&ctx, &mesh, 80, 24).unwrap();
    for override_color in [None, Some([0.9, 0.4, 0.1])] {
        for light in [Vec3::Z, Vec3::new(0.0, 1.0, 1.0).normalize()] {
            let mut cpu = Framebuffer::new(80, 24);
            let mut gpu = Framebuffer::new(80, 24);
            render_frame(&mut cpu, &mesh, 0.0, 0.0, 1.0, light, override_color);
            render_frame_gpu(
                &mut gpu,
                &pipeline,
                &ctx,
                0.0,
                0.0,
                1.0,
                light,
                override_color,
            )
            .unwrap();
            assert_eq!(cpu.chars, gpu.chars);
            assert!(cpu.chars.iter().any(|&ch| ch != ' '));
            for i in 0..cpu.chars.len() {
                assert!((cpu.luminances[i] - gpu.luminances[i]).abs() <= 1e-5);
                for channel in 0..3 {
                    assert!((cpu.colors[i][channel] - gpu.colors[i][channel]).abs() <= 1e-5);
                }
                if cpu.chars[i] != ' ' {
                    assert_eq!(cpu.colors[i], override_color.unwrap_or([0.2, 0.6, 0.9]));
                    assert!(cpu.depth[i].is_finite());
                }
            }
            assert!(gpu.depth.iter().all(|&depth| depth == f32::INFINITY));
        }
    }
}

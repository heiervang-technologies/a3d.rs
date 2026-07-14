//! An empty mesh (e.g. a model file with no geometry) must render a blank frame
//! rather than panicking. The GPU path previously created a zero-sized storage
//! buffer and panicked on pipeline construction; see bug hunt #21.
use glam::Vec3;

use a3d::gpu::{GpuContext, RasterPipeline};
use a3d::model::Mesh;
use a3d::render::Framebuffer;
use a3d::{framebuffer_to_string, render_frame, render_frame_gpu};

const LIGHT_DIR: Vec3 = Vec3::new(0.70710677, -0.70710677, 0.0);
const W: usize = 80;
const H: usize = 24;

fn empty_mesh() -> Mesh {
    Mesh {
        vertices: vec![],
        indices: vec![],
    }
}

fn is_blank(s: &str) -> bool {
    s.chars().all(|c| c == ' ' || c == '\n')
}

#[test]
fn cpu_renders_empty_mesh_blank() {
    let mut fb = Framebuffer::new(W, H);
    render_frame(&mut fb, &empty_mesh(), 0.0, 0.0, 1.0, LIGHT_DIR, None);
    assert!(
        is_blank(&framebuffer_to_string(&fb)),
        "empty mesh should render blank on the CPU"
    );
}

#[test]
fn gpu_builds_and_renders_empty_mesh_without_crashing() {
    let Some(ctx) = pollster::block_on(GpuContext::new()) else {
        eprintln!("Skipping gpu empty-mesh test: no GPU adapter available");
        return;
    };
    // Constructing the pipeline used to panic here on a zero-sized buffer.
    let pipeline = RasterPipeline::new(&ctx, &empty_mesh(), W as u32, H as u32);
    let mut fb = Framebuffer::new(W, H);
    render_frame_gpu(&mut fb, &pipeline, &ctx, 0.0, 0.0, 1.0, LIGHT_DIR, None);
    assert!(
        is_blank(&framebuffer_to_string(&fb)),
        "empty mesh should render blank on the GPU"
    );

    // A programmatic mesh can bypass load_model's validation. Invalid
    // triangles are ignored instead of reaching the shader as out-of-bounds
    // vertex reads.
    let invalid_mesh = Mesh {
        vertices: vec![],
        indices: vec![0, 1, u32::MAX],
    };
    let invalid_pipeline = RasterPipeline::new(&ctx, &invalid_mesh, W as u32, H as u32);
    render_frame_gpu(
        &mut fb,
        &invalid_pipeline,
        &ctx,
        0.0,
        0.0,
        1.0,
        LIGHT_DIR,
        None,
    );
    assert!(is_blank(&framebuffer_to_string(&fb)));

    // A stale pipeline used to panic while copying a larger GPU surface into a
    // smaller framebuffer. The public wrapper now rejects the mismatch safely.
    let mut smaller_fb = Framebuffer::new(W / 2, H / 2);
    smaller_fb.set_pixel(0, 0, 0.0, 1.0, [1.0; 3]);
    render_frame_gpu(
        &mut smaller_fb,
        &pipeline,
        &ctx,
        0.0,
        0.0,
        1.0,
        LIGHT_DIR,
        None,
    );
    assert!(is_blank(&framebuffer_to_string(&smaller_fb)));
}

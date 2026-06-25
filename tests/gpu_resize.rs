//! `RasterPipeline::resize` rebuilds every framebuffer-sized buffer and the bind
//! group when the terminal changes size. This exercises that path (otherwise
//! untested) by asserting a resized pipeline renders identically to a fresh one
//! built directly at the target size. Skips when no GPU adapter is present.
use std::path::Path;

use glam::Vec3;

use a3d::gpu::{GpuContext, RasterPipeline};
use a3d::model::load_model;
use a3d::render::Framebuffer;
use a3d::{framebuffer_to_string, render_frame_gpu};

const LIGHT_DIR: Vec3 = Vec3::new(0.70710677, -0.70710677, 0.0);

fn render(pipeline: &RasterPipeline, ctx: &GpuContext, w: usize, h: usize) -> String {
    let mut fb = Framebuffer::new(w, h);
    render_frame_gpu(&mut fb, pipeline, ctx, 0.0, 0.0, 1.0, LIGHT_DIR, None);
    framebuffer_to_string(&fb)
}

#[test]
fn resized_pipeline_matches_fresh_one() {
    let Some(ctx) = pollster::block_on(GpuContext::new()) else {
        eprintln!("Skipping resized_pipeline_matches_fresh_one: no GPU adapter available");
        return;
    };
    let mesh = load_model(Path::new("models/dog.stl")).expect("load model");

    // A pipeline built small, then resized up, must match a pipeline built
    // directly at the target size — proving resize rebuilt the buffers and bind
    // group correctly (a stale buffer would diverge or crash).
    let mut resized = RasterPipeline::new(&ctx, &mesh, 40, 12);
    resized.resize(&ctx, 100, 30);
    let fresh = RasterPipeline::new(&ctx, &mesh, 100, 30);

    assert_eq!(
        render(&resized, &ctx, 100, 30),
        render(&fresh, &ctx, 100, 30),
        "resized pipeline diverged from a freshly-built one"
    );

    // Resizing back down must work too (buffers shrink, no leftover rows).
    resized.resize(&ctx, 60, 20);
    let fresh_small = RasterPipeline::new(&ctx, &mesh, 60, 20);
    assert_eq!(
        render(&resized, &ctx, 60, 20),
        render(&fresh_small, &ctx, 60, 20),
        "pipeline resized back down diverged from a freshly-built one"
    );
}

//! An empty mesh (e.g. a model file with no geometry) must render a blank frame
//! rather than panicking. The GPU path previously created a zero-sized storage
//! buffer and panicked on pipeline construction; see bug hunt #21.
use glam::Vec3;

use a3d::gpu::{GpuContext, RasterPipeline};
use a3d::model::Mesh;
use a3d::render::Framebuffer;
use a3d::{RenderParams, framebuffer_to_string, render_frame, render_frame_gpu};

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

fn front_view() -> RenderParams {
    RenderParams {
        azimuth: 0.0,
        altitude: 0.0,
        zoom: 1.0,
        light_dir: LIGHT_DIR,
        fg_override: None,
    }
}

#[test]
fn cpu_renders_empty_mesh_blank() {
    let mut fb = Framebuffer::new(W, H);
    render_frame(&mut fb, &empty_mesh(), &front_view());
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
    render_frame_gpu(&mut fb, &pipeline, &ctx, &front_view());
    assert!(
        is_blank(&framebuffer_to_string(&fb)),
        "empty mesh should render blank on the GPU"
    );
}

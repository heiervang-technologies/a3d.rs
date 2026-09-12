//! GPU failures must return errors, leaving callers able to render on CPU.
use a3d::gpu::{GpuContext, RasterPipeline};
use a3d::model::{Mesh, mesh::Vertex};
use a3d::render::Framebuffer;
use a3d::{render_frame, render_frame_gpu};
use glam::Vec3;

fn mesh() -> Mesh {
    Mesh {
        vertices: [[-0.5, -0.5, 0.0], [0.0, 0.5, 0.0], [0.5, -0.5, 0.0]]
            .map(|position| Vertex {
                position,
                normal: [0.0; 3],
                color: [0.8; 3],
            })
            .to_vec(),
        indices: vec![0, 1, 2],
    }
}

#[test]
fn oversized_resources_return_errors_and_preserve_existing_pipeline() {
    let Some(ctx) = pollster::block_on(GpuContext::new()) else {
        eprintln!("Skipping GPU resource failures: no suitable adapter");
        return;
    };
    let mesh = mesh();
    assert!(RasterPipeline::new(&ctx, &mesh, u32::MAX, u32::MAX).is_err());
    // Within shader indexing limits, but too large for storage bindings.
    assert!(RasterPipeline::new(&ctx, &mesh, 65536, 1024).is_err());
    let mut pipeline = RasterPipeline::new(&ctx, &mesh, 40, 20).unwrap();
    assert!(pipeline.resize(&ctx, u32::MAX, u32::MAX).is_err());
    assert_eq!(pipeline.dimensions(), (40, 20));
    let mut fb = Framebuffer::new(40, 20);
    render_frame_gpu(&mut fb, &pipeline, &ctx, 0.0, 0.0, 1.0, Vec3::Z, None).unwrap();
    assert!(fb.chars.iter().any(|&ch| ch != ' '));
}

#[test]
fn lost_device_returns_error_and_cpu_can_render_the_same_frame() {
    let Some(ctx) = pollster::block_on(GpuContext::new()) else {
        eprintln!("Skipping GPU device loss: no suitable adapter");
        return;
    };
    let mesh = mesh();
    let mut pipeline = RasterPipeline::new(&ctx, &mesh, 40, 20).unwrap();
    let mut fb = Framebuffer::new(40, 20);
    ctx.device.destroy();
    let result = render_frame_gpu(&mut fb, &pipeline, &ctx, 0.0, 0.0, 1.0, Vec3::Z, None);
    assert!(result.is_err(), "destroyed device must report an error");
    assert!(fb.chars.iter().all(|&ch| ch == ' '));
    assert!(pipeline.resize(&ctx, 80, 24).is_err());
    assert!(RasterPipeline::new(&ctx, &mesh, 40, 20).is_err());
    render_frame(&mut fb, &mesh, 0.0, 0.0, 1.0, Vec3::Z, None);
    assert!(fb.chars.iter().any(|&ch| ch != ' '));
}

mod gpu;
mod model;
mod render;
mod terminal;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use clap::Parser;
use crossterm::event::KeyCode;
use glam::Vec3;

use gpu::GpuContext;
use model::load_model;
use render::{Camera, Framebuffer};
use terminal::TerminalDisplay;

const GOLDEN_RATIO: f32 = 1.618_034;

#[derive(Parser)]
#[command(name = "a3d", about = "GPU-accelerated ASCII 3D renderer")]
struct Args {
    /// Path to 3D model file (OBJ or STL)
    model: PathBuf,

    /// Target frames per second
    #[arg(short, long, default_value_t = 30)]
    fps: u32,

    /// Interactive rotation mode
    #[arg(short, long)]
    interactive: bool,

    /// Initial zoom level
    #[arg(short, long, default_value_t = 3.0)]
    zoom: f32,
}

fn main() {
    env_logger::init();
    let args = Args::parse();

    // Initialize GPU
    let _gpu = pollster::block_on(GpuContext::new());

    // Load model
    let mesh = load_model(&args.model);
    log::info!(
        "Loaded {} vertices, {} indices",
        mesh.vertices.len(),
        mesh.indices.len()
    );

    // Setup terminal
    let mut display = TerminalDisplay::new().expect("Failed to initialize terminal");
    let (w, h) = display.size();
    let mut fb = Framebuffer::new(w, h);
    let mut camera = Camera {
        distance: args.zoom,
        ..Default::default()
    };

    let frame_duration = Duration::from_secs_f64(1.0 / args.fps as f64);
    let start = Instant::now();
    let light_dir = Vec3::new(1.0, -1.0, 0.5).normalize();

    loop {
        let frame_start = Instant::now();
        let t = start.elapsed().as_secs_f32();

        // Handle input
        if let Some(key) = display.poll_event() {
            match key {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Up => camera.altitude += 0.1,
                KeyCode::Down => camera.altitude -= 0.1,
                KeyCode::Left => camera.azimuth -= 0.1,
                KeyCode::Right => camera.azimuth += 0.1,
                KeyCode::Char('+') | KeyCode::Char('=') => camera.distance -= 0.2,
                KeyCode::Char('-') => camera.distance += 0.2,
                _ => {}
            }
        }

        // Auto-rotate if not interactive
        if !args.interactive {
            camera.azimuth = 0.5 * t;
            camera.altitude =
                0.125 * std::f32::consts::PI * (1.0 - (GOLDEN_RATIO * 0.25 * t).sin());
        }

        // Resize if needed
        let (w, h) = display.size();
        if w != fb.width || h != fb.height {
            fb.resize(w, h);
        }

        // Clear and rasterize (CPU fallback for now)
        fb.clear();
        let view_proj = camera.projection(w as f32 / (h as f32 * 1.8)) * camera.view_matrix();

        for tri in mesh.indices.chunks(3) {
            if tri.len() < 3 {
                continue;
            }
            let v0 = &mesh.vertices[tri[0] as usize];
            let v1 = &mesh.vertices[tri[1] as usize];
            let v2 = &mesh.vertices[tri[2] as usize];

            let p0 = Vec3::from(v0.position);
            let p1 = Vec3::from(v1.position);
            let p2 = Vec3::from(v2.position);

            let edge1 = p1 - p0;
            let edge2 = p2 - p0;
            let normal = edge1.cross(edge2).normalize();

            let luminance = normal.dot(light_dir) * 0.5 + 0.5;

            // Project to screen
            let project = |p: Vec3| -> Option<(f32, f32, f32)> {
                let clip = view_proj * p.extend(1.0);
                if clip.w <= 0.0 {
                    return None;
                }
                let ndc = clip.truncate() / clip.w;
                let sx = (ndc.x * 0.5 + 0.5) * fb.width as f32;
                let sy = (1.0 - (ndc.y * 0.5 + 0.5)) * fb.height as f32;
                Some((sx, sy, ndc.z))
            };

            let Some(s0) = project(p0) else { continue };
            let Some(s1) = project(p1) else { continue };
            let Some(s2) = project(p2) else { continue };

            rasterize_triangle(&mut fb, s0, s1, s2, luminance, v0.color);
        }

        let _ = display.render(&fb);

        // Frame rate limiting
        let elapsed = frame_start.elapsed();
        if elapsed < frame_duration {
            std::thread::sleep(frame_duration - elapsed);
        }
    }
}

fn rasterize_triangle(
    fb: &mut Framebuffer,
    v0: (f32, f32, f32),
    v1: (f32, f32, f32),
    v2: (f32, f32, f32),
    luminance: f32,
    color: [f32; 3],
) {
    let min_x = v0.0.min(v1.0).min(v2.0).max(0.0) as usize;
    let max_x = v0.0.max(v1.0).max(v2.0).min(fb.width as f32 - 1.0) as usize;
    let min_y = v0.1.min(v1.1).min(v2.1).max(0.0) as usize;
    let max_y = v0.1.max(v1.1).max(v2.1).min(fb.height as f32 - 1.0) as usize;

    for y in min_y..=max_y {
        for x in min_x..=max_x {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;

            // Barycentric coordinates
            let d00 = v1.0 - v0.0;
            let d01 = v2.0 - v0.0;
            let d10 = v1.1 - v0.1;
            let d11 = v2.1 - v0.1;
            let denom = d00 * d11 - d01 * d10;
            if denom.abs() < 1e-6 {
                continue;
            }

            let dx = px - v0.0;
            let dy = py - v0.1;
            let u = (dx * d11 - d01 * dy) / denom;
            let v = (d00 * dy - dx * d10) / denom;

            if u >= 0.0 && v >= 0.0 && u + v <= 1.0 {
                let z = v0.2 * (1.0 - u - v) + v1.2 * u + v2.2 * v;
                fb.set_pixel(x, y, z, luminance, color);
            }
        }
    }
}

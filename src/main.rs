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
use render::Framebuffer;
use terminal::TerminalDisplay;

const GOLDEN_RATIO: f32 = 1.618_034;
const AZ_SPEED: f32 = 2.0;
const AL_SPEED: f32 = GOLDEN_RATIO * 0.25;

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
    #[arg(short, long, default_value_t = 1.0)]
    zoom: f32,

    /// Enable ANSI true color output
    #[arg(short, long)]
    color: bool,
}

fn rotate_y(v: Vec3, cos_a: f32, sin_a: f32) -> Vec3 {
    Vec3::new(
        v.x * cos_a - v.z * sin_a,
        v.y,
        v.x * sin_a + v.z * cos_a,
    )
}

fn rotate_x(v: Vec3, cos_a: f32, sin_a: f32) -> Vec3 {
    Vec3::new(
        v.x,
        v.y * cos_a - v.z * sin_a,
        v.y * sin_a + v.z * cos_a,
    )
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

    let mut azimuth: f32 = 0.0;
    let mut altitude: f32 = 0.0;
    let mut zoom = args.zoom;
    let mut color = args.color;

    let frame_duration = Duration::from_secs_f64(1.0 / args.fps as f64);
    let start = Instant::now();
    let light_dir = Vec3::new(1.0, -1.0, 0.0).normalize();

    loop {
        let frame_start = Instant::now();
        let t = start.elapsed().as_secs_f32();

        // Handle input
        if let Some(key) = display.poll_event() {
            match key {
                KeyCode::Char('q') | KeyCode::Esc => break,
                KeyCode::Up => altitude += 0.1,
                KeyCode::Down => altitude -= 0.1,
                KeyCode::Left => azimuth += 0.1,
                KeyCode::Right => azimuth -= 0.1,
                KeyCode::Char('+') | KeyCode::Char('=') => zoom = (zoom * 1.1).min(10.0),
                KeyCode::Char('-') => zoom = (zoom * 0.9).max(0.1),
                KeyCode::Char('c') => color = !color,
                _ => {}
            }
        }

        // Auto-rotate if not interactive (matches voxcii exactly)
        if !args.interactive {
            azimuth = AZ_SPEED * t;
            altitude = 0.125 * std::f32::consts::PI * (1.0 - (AL_SPEED * t).sin());
        }

        // Resize if needed
        let (w, h) = display.size();
        if w != fb.width || h != fb.height {
            fb.resize(w, h);
        }

        // Logical dimensions with aspect ratio correction (chars are ~1.8x taller than wide)
        let logical_h: f32 = 1.0;
        let logical_w: f32 = w as f32 / (h as f32 * 1.8);
        let dx = logical_w / w as f32;
        let dy = logical_h / h as f32;

        // Pre-compute rotation
        let cos_az = azimuth.cos();
        let sin_az = azimuth.sin();
        let cos_al = (-altitude).cos();
        let sin_al = (-altitude).sin();

        fb.clear();

        for tri in mesh.indices.chunks(3) {
            if tri.len() < 3 {
                continue;
            }

            let v0 = &mesh.vertices[tri[0] as usize];
            let p0 = Vec3::from(v0.position);
            let p1 = Vec3::from(mesh.vertices[tri[1] as usize].position);
            let p2 = Vec3::from(mesh.vertices[tri[2] as usize].position);

            // Rotate vertices: Y first, then X (matches voxcii)
            let r0 = rotate_x(rotate_y(p0, cos_az, sin_az), cos_al, sin_al);
            let r1 = rotate_x(rotate_y(p1, cos_az, sin_az), cos_al, sin_al);
            let r2 = rotate_x(rotate_y(p2, cos_az, sin_az), cos_al, sin_al);

            // Compute face normal from rotated vertices, negate for lighting
            let normal = (r1 - r0).cross(r2 - r0);
            if normal.length_squared() < 1e-10 {
                continue;
            }
            let normal = normal.normalize();
            let luminance = (-normal).dot(light_dir) * 0.5 + 0.5;

            // Orthographic projection to screen (matches voxcii mapToSurface)
            let project = |v: Vec3| -> Vec3 {
                Vec3::new(
                    0.5 * logical_w + 0.5 * v.x * zoom,
                    0.5 * logical_h - 0.5 * v.y * zoom,
                    0.5 + 0.5 * v.z * zoom,
                )
            };

            let s0 = project(r0);
            let s1 = project(r1);
            let s2 = project(r2);

            rasterize_triangle(&mut fb, s0, s1, s2, dx, dy, luminance, v0.color);
        }

        let _ = display.render(&fb, color);

        // Frame rate limiting
        let elapsed = frame_start.elapsed();
        if elapsed < frame_duration {
            std::thread::sleep(frame_duration - elapsed);
        }
    }
}

/// Scanline triangle rasterizer matching voxcii's algorithm exactly.
fn rasterize_triangle(
    fb: &mut Framebuffer,
    p0: Vec3,
    p1: Vec3,
    p2: Vec3,
    dx: f32,
    dy: f32,
    luminance: f32,
    color: [f32; 3],
) {
    // Back-face culling: 2D cross product test on screen coords
    if (p1.x - p0.x) * (p2.y - p1.y) < (p2.x - p1.x) * (p1.y - p0.y) {
        return;
    }

    // Sort vertices by X
    let mut pts = [p0, p1, p2];
    pts.sort_by(|a, b| a.x.partial_cmp(&b.x).unwrap());

    // Triangle plane normal for Z interpolation
    let tri_normal = (p1 - p0).cross(p2 - p0);
    let nz = if tri_normal.z == 0.0 { 0.0001 } else { tri_normal.z };

    let xi = pts[0].x + dx / 2.0;
    let xf = pts[2].x - dx / 2.0;

    let x_start = ((xi / dx) as i32).max(0);
    let x_end = ((xf / dx) as i32).min(fb.width as i32 - 1);

    // Interpolate Y along an edge at given X
    let get_y = |pa: Vec3, pb: Vec3, x: f32| -> f32 {
        if pa.x == pb.x {
            pa.y
        } else {
            pa.y + (pb.y - pa.y) * (x - pa.x) / (pb.x - pa.x)
        }
    };

    for xx in x_start..=x_end {
        let x = (xx as f32 + 0.5) * dx;

        // Find Y span from triangle edges
        let y1 = if x <= pts[1].x {
            get_y(pts[0], pts[1], x)
        } else {
            get_y(pts[1], pts[2], x)
        };
        let y2 = get_y(pts[0], pts[2], x);

        let yi = y1.min(y2);
        let yf = y1.max(y2);

        let y_start = (((yi + dy / 2.0) / dy) as i32).max(0);
        let y_end = (((yf - dy / 2.0) / dy) as i32).min(fb.height as i32 - 1);

        for yy in y_start..=y_end {
            let y = (yy as f32 + 0.5) * dy;

            // Z from plane equation
            let depth = pts[0].z
                - (tri_normal.x * (x - pts[0].x) + tri_normal.y * (y - pts[0].y)) / nz;

            fb.set_pixel(xx as usize, yy as usize, depth, luminance, color);
        }
    }
}

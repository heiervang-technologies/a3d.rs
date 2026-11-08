//! # a3d
//!
//! A GPU-accelerated ASCII 3D rendering engine: it loads OBJ/STL/GLB meshes and
//! rasterizes them to ASCII art, either on the GPU (a wgpu compute pipeline) or
//! on an equivalent CPU scanline rasterizer.
//!
//! The crate ships both the `a3d` terminal binary and this library. The library
//! is renderer-only and TTY-free, so it can be driven headlessly (the test suite
//! does exactly this). A minimal CPU render:
//!
//! ```no_run
//! use std::path::Path;
//! use glam::Vec3;
//! use a3d::model::load_model;
//! use a3d::render::Framebuffer;
//! use a3d::{render_frame, framebuffer_to_string};
//!
//! let mesh = load_model(Path::new("models/dog.stl")).expect("load model");
//! let mut fb = Framebuffer::new(80, 24);
//! let light = Vec3::new(1.0, -1.0, 0.0).normalize();
//! render_frame(&mut fb, &mesh, 0.0, 0.0, 1.0, light, None);
//! print!("{}", framebuffer_to_string(&fb));
//! ```
//!
//! For GPU rendering see [`GpuContext`] and [`render_frame_gpu`]. Both backends
//! implement the same flat-shaded orthographic renderer, but floating-point
//! rounding and GPU depth quantization can differ. Scene regression tests allow
//! 0.5% differing ASCII cells (1% for the zoom sweep); synthetic fixtures also
//! compare colors and luminances with a tolerance of 1e-5.
//! GPU rendering does not read back depth: [`Framebuffer::depth`] stays at
//! infinity, so CPU depth compositing must start with a CPU-rendered frame.
#![forbid(unsafe_code)]
#![warn(missing_docs)]

/// GPU compute rasterizer: wgpu device setup and the 3-pass pipeline.
pub mod gpu;
/// Mesh types and OBJ/STL/GLB loading.
pub mod model;
/// CPU framebuffer, ASCII luminance mapping, and camera.
pub mod render;
/// Terminal display and keyboard/mouse input (crossterm).
pub mod terminal;

use glam::Vec3;
use gpu::{GpuContext, GpuUniforms, RasterPipeline};
use render::Framebuffer;

/// The golden ratio φ, used to make the auto-rotation oscillation non-repeating.
pub const GOLDEN_RATIO: f32 = 1.618_034;
/// Auto-rotation azimuth speed (radians per second of elapsed time).
pub const AZ_SPEED: f32 = 2.0;
/// Auto-rotation altitude speed, offset by the golden ratio for smooth drift.
pub const AL_SPEED: f32 = GOLDEN_RATIO * 0.25;

fn render_inputs_are_finite(
    azimuth: f32,
    altitude: f32,
    zoom: f32,
    light_dir: Vec3,
    fg_override: Option<[f32; 3]>,
) -> bool {
    azimuth.is_finite()
        && altitude.is_finite()
        && zoom.is_finite()
        && light_dir.is_finite()
        && fg_override.is_none_or(|color| color.into_iter().all(f32::is_finite))
}

/// Rotate `v` about the Y axis, given the precomputed cosine and sine of the angle.
pub fn rotate_y(v: Vec3, cos_a: f32, sin_a: f32) -> Vec3 {
    Vec3::new(v.x * cos_a - v.z * sin_a, v.y, v.x * sin_a + v.z * cos_a)
}

/// Rotate `v` about the X axis, given the precomputed cosine and sine of the angle.
pub fn rotate_x(v: Vec3, cos_a: f32, sin_a: f32) -> Vec3 {
    Vec3::new(v.x, v.y * cos_a - v.z * sin_a, v.y * sin_a + v.z * cos_a)
}

/// Render a single frame into the framebuffer.
/// This is the core rendering function, usable without a terminal for testing.
pub fn render_frame(
    fb: &mut Framebuffer,
    mesh: &model::Mesh,
    azimuth: f32,
    altitude: f32,
    zoom: f32,
    light_dir: Vec3,
    fg_override: Option<[f32; 3]>,
) {
    let w = fb.width;
    let h = fb.height;

    fb.clear();
    if w == 0
        || h == 0
        || !render_inputs_are_finite(azimuth, altitude, zoom, light_dir, fg_override)
    {
        return;
    }

    let logical_h: f32 = 1.0;
    let logical_w: f32 = w as f32 / (h as f32 * 1.8);
    let dx = logical_w / w as f32;
    let dy = logical_h / h as f32;

    let cos_az = azimuth.cos();
    let sin_az = azimuth.sin();
    let cos_al = (-altitude).cos();
    let sin_al = (-altitude).sin();

    for tri in mesh.indices.chunks(3) {
        if tri.len() < 3 {
            continue;
        }

        // `Mesh` is constructible by library users, so do not assume it came
        // through the validated file loader.
        let (Some(v0), Some(v1), Some(v2)) = (
            mesh.vertices.get(tri[0] as usize),
            mesh.vertices.get(tri[1] as usize),
            mesh.vertices.get(tri[2] as usize),
        ) else {
            continue;
        };
        let p0 = Vec3::from(v0.position);
        let p1 = Vec3::from(v1.position);
        let p2 = Vec3::from(v2.position);

        let r0 = rotate_x(rotate_y(p0, cos_az, sin_az), cos_al, sin_al);
        let r1 = rotate_x(rotate_y(p1, cos_az, sin_az), cos_al, sin_al);
        let r2 = rotate_x(rotate_y(p2, cos_az, sin_az), cos_al, sin_al);

        let normal = (r1 - r0).cross(r2 - r0);
        if normal.length_squared() < 1e-10 {
            continue;
        }
        let normal = normal.normalize();
        let luminance = (-normal).dot(light_dir) * 0.5 + 0.5;

        let project = |v: Vec3| -> Vec3 {
            Vec3::new(
                0.5 * logical_w + 0.5 * v.x * zoom,
                0.5 * logical_h - 0.5 * v.y * zoom,
                // Depth is NOT scaled by zoom: the GPU quantizes depth into a
                // clamped [0,1] u32 (raster.wgsl pack_depth), so a zoom-scaled z
                // would spill outside [0,1] and collapse the z-ordering. Since
                // only relative ordering matters, dropping zoom here keeps z in
                // [0,1] at every zoom and leaves CPU output unchanged.
                0.5 + 0.5 * v.z,
            )
        };

        let s0 = project(r0);
        let s1 = project(r1);
        let s2 = project(r2);

        let tri_color = fg_override.unwrap_or(v0.color);
        rasterize_triangle(fb, s0, s1, s2, dx, dy, luminance, tri_color);
    }
}

/// Render a single frame using the GPU compute pipeline.
///
/// Returns an error for invalid inputs, a size mismatch, or GPU failure. On
/// error the framebuffer is cleared; callers may render the same frame on CPU.
/// On success characters, colors, and luminances are populated. Depth remains
/// infinity: GPU depth is internal and is not available for CPU compositing.
// Mirrors `render_frame`'s parameter set; a `RenderParams` struct is tracked as
// follow-up cleanup (see issue #3).
#[allow(clippy::too_many_arguments)]
pub fn render_frame_gpu(
    fb: &mut Framebuffer,
    pipeline: &RasterPipeline,
    ctx: &GpuContext,
    azimuth: f32,
    altitude: f32,
    zoom: f32,
    light_dir: Vec3,
    fg_override: Option<[f32; 3]>,
) -> Result<(), String> {
    let w = fb.width;
    let h = fb.height;

    fb.clear();
    if !render_inputs_are_finite(azimuth, altitude, zoom, light_dir, fg_override) {
        return Err("GPU render inputs must be finite".into());
    }
    let w_u32 = u32::try_from(w).map_err(|_| "framebuffer width exceeds the GPU u32 limit")?;
    let h_u32 = u32::try_from(h).map_err(|_| "framebuffer height exceeds the GPU u32 limit")?;
    if w == 0 || h == 0 {
        return Ok(());
    }
    if pipeline.dimensions() != (w_u32, h_u32) {
        return Err(format!(
            "GPU pipeline is {}x{} but framebuffer is {w}x{h}; call RasterPipeline::resize first",
            pipeline.dimensions().0,
            pipeline.dimensions().1,
        ));
    }

    let logical_h: f32 = 1.0;
    let logical_w: f32 = w as f32 / (h as f32 * 1.8);
    let dx = logical_w / w as f32;
    let dy = logical_h / h as f32;

    let uniforms = GpuUniforms {
        width: w as u32,
        height: h as u32,
        cos_az: azimuth.cos(),
        sin_az: azimuth.sin(),
        cos_al: (-altitude).cos(),
        sin_al: (-altitude).sin(),
        zoom,
        logical_w,
        logical_h,
        dx,
        dy,
        has_fg_override: if fg_override.is_some() { 1 } else { 0 },
        light_dir: light_dir.into(),
        _pad0: 0.0,
        fg_override: fg_override.unwrap_or([0.8, 0.8, 0.8]),
        _pad1: 0.0,
    };

    pipeline.render(ctx, fb, &uniforms)
}

/// Convert framebuffer to a string of ASCII art (rows separated by newlines).
pub fn framebuffer_to_string(fb: &Framebuffer) -> String {
    let mut out = String::with_capacity(fb.width * fb.height + fb.height);
    for y in 0..fb.height {
        for x in 0..fb.width {
            out.push(fb.chars[y * fb.width + x]);
        }
        if y < fb.height - 1 {
            out.push('\n');
        }
    }
    out
}

/// Scanline triangle rasterizer matching voxcii's algorithm exactly.
#[allow(clippy::too_many_arguments)]
pub fn rasterize_triangle(
    fb: &mut Framebuffer,
    p0: Vec3,
    p1: Vec3,
    p2: Vec3,
    dx: f32,
    dy: f32,
    luminance: f32,
    color: [f32; 3],
) {
    if fb.width == 0
        || fb.height == 0
        || !dx.is_finite()
        || !dy.is_finite()
        || dx <= 0.0
        || dy <= 0.0
        || !p0.is_finite()
        || !p1.is_finite()
        || !p2.is_finite()
        || !luminance.is_finite()
        || !color.into_iter().all(f32::is_finite)
    {
        return;
    }
    if (p1.x - p0.x) * (p2.y - p1.y) < (p2.x - p1.x) * (p1.y - p0.y) {
        return;
    }

    let mut pts = [p0, p1, p2];
    // total_cmp instead of partial_cmp().unwrap(): a NaN coordinate from a
    // malformed model would otherwise panic the render loop.
    pts.sort_by(|a, b| a.x.total_cmp(&b.x));

    let tri_normal = (p1 - p0).cross(p2 - p0);
    let nz = if tri_normal.z == 0.0 {
        0.0001
    } else {
        tri_normal.z
    };

    let xi = pts[0].x + dx / 2.0;
    let xf = pts[2].x - dx / 2.0;

    let x_start = ((xi / dx) as i32).max(0);
    let x_end = ((xf / dx) as i32).min(fb.width as i32 - 1);

    let get_y = |pa: Vec3, pb: Vec3, x: f32| -> f32 {
        if pa.x == pb.x {
            pa.y
        } else {
            pa.y + (pb.y - pa.y) * (x - pa.x) / (pb.x - pa.x)
        }
    };

    for xx in x_start..=x_end {
        let x = (xx as f32 + 0.5) * dx;

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

            // Clamp to [0,1] to match the GPU, where pack_depth clamps before
            // quantizing. Without this, a steep triangle whose interpolated
            // depth extrapolates outside the range would order differently on
            // the two backends.
            let depth = (pts[0].z
                - (tri_normal.x * (x - pts[0].x) + tri_normal.y * (y - pts[0].y)) / nz)
                .clamp(0.0, 1.0);

            fb.set_pixel(xx as usize, yy as usize, depth, luminance, color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec3;

    #[test]
    fn rotate_y_quarter_turn() {
        // 90°: cos=0, sin=1. (1,0,0) -> (0,0,1).
        let r = rotate_y(Vec3::new(1.0, 0.0, 0.0), 0.0, 1.0);
        assert!((r - Vec3::new(0.0, 0.0, 1.0)).length() < 1e-6, "{r}");
    }

    #[test]
    fn rotate_x_quarter_turn() {
        // 90°: cos=0, sin=1. (0,1,0) -> (0,0,1).
        let r = rotate_x(Vec3::new(0.0, 1.0, 0.0), 0.0, 1.0);
        assert!((r - Vec3::new(0.0, 0.0, 1.0)).length() < 1e-6, "{r}");
    }

    #[test]
    fn rotation_identity() {
        let v = Vec3::new(1.0, 2.0, 3.0);
        assert_eq!(rotate_y(v, 1.0, 0.0), v);
        assert_eq!(rotate_x(v, 1.0, 0.0), v);
    }

    #[test]
    fn rotation_preserves_length() {
        let v = Vec3::new(0.3, -0.7, 0.5);
        let (c, s) = (0.9f32.cos(), 0.9f32.sin());
        let r = rotate_x(rotate_y(v, c, s), c, s);
        assert!((r.length() - v.length()).abs() < 1e-6);
    }

    #[test]
    fn zero_sized_framebuffer_is_a_noop() {
        let mesh = model::Mesh {
            vertices: vec![],
            indices: vec![],
        };
        let mut fb = Framebuffer::new(0, 0);
        render_frame(&mut fb, &mesh, 0.0, 0.0, 1.0, Vec3::Y, None);
        assert!(framebuffer_to_string(&fb).is_empty());
    }

    #[test]
    fn invalid_programmatic_indices_are_ignored() {
        let mesh = model::Mesh {
            vertices: vec![],
            indices: vec![0, 1, u32::MAX],
        };
        let mut fb = Framebuffer::new(2, 2);
        render_frame(&mut fb, &mesh, 0.0, 0.0, 1.0, Vec3::Y, None);
        assert!(fb.chars.iter().all(|&ch| ch == ' '));
    }

    #[test]
    fn non_finite_render_inputs_are_ignored() {
        let mesh = model::Mesh {
            vertices: vec![],
            indices: vec![],
        };
        let mut fb = Framebuffer::new(2, 2);
        fb.set_pixel(0, 0, 0.0, 1.0, [1.0; 3]);
        render_frame(&mut fb, &mesh, f32::NAN, 0.0, 1.0, Vec3::Y, None);
        assert!(fb.chars.iter().all(|&ch| ch == ' '));
    }
}

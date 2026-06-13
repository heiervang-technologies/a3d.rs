pub mod gpu;
pub mod model;
pub mod render;
pub mod terminal;

use glam::Vec3;
use gpu::{GpuContext, GpuUniforms, RasterPipeline};
use render::Framebuffer;

pub const GOLDEN_RATIO: f32 = 1.618_034;
pub const AZ_SPEED: f32 = 2.0;
pub const AL_SPEED: f32 = GOLDEN_RATIO * 0.25;

pub fn rotate_y(v: Vec3, cos_a: f32, sin_a: f32) -> Vec3 {
    Vec3::new(v.x * cos_a - v.z * sin_a, v.y, v.x * sin_a + v.z * cos_a)
}

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

    let logical_h: f32 = 1.0;
    let logical_w: f32 = w as f32 / (h as f32 * 1.8);
    let dx = logical_w / w as f32;
    let dy = logical_h / h as f32;

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
) {
    let w = fb.width;
    let h = fb.height;

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

    pipeline.render(ctx, fb, &uniforms);
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

            let depth =
                pts[0].z - (tri_normal.x * (x - pts[0].x) + tri_normal.y * (y - pts[0].y)) / nz;

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
}

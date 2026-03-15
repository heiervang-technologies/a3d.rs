// GPU compute shader for ASCII 3D rasterization.
// Three entry points: vertex_transform, rasterize_depth, rasterize_shade.

struct Uniforms {
    width: u32,
    height: u32,
    cos_az: f32,
    sin_az: f32,
    cos_al: f32,
    sin_al: f32,
    zoom: f32,
    logical_w: f32,
    logical_h: f32,
    dx: f32,
    dy: f32,
    has_fg_override: u32,
    light_dir: vec3<f32>,
    _pad0: f32,
    fg_override: vec3<f32>,
    _pad1: f32,
}

struct Vertex {
    position: vec3<f32>,
    normal: vec3<f32>,
    color: vec3<f32>,
}

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var<storage, read> vertices: array<Vertex>;
@group(0) @binding(2) var<storage, read> indices: array<u32>;
@group(0) @binding(3) var<storage, read_write> transformed: array<vec4<f32>>;
@group(0) @binding(4) var<storage, read_write> depth_buf: array<atomic<u32>>;
@group(0) @binding(5) var<storage, read_write> output_char: array<u32>;
@group(0) @binding(6) var<storage, read_write> output_luminance: array<f32>;
@group(0) @binding(7) var<storage, read_write> output_color: array<f32>;

const DEPTH_SCALE: f32 = 4294967040.0;
const ASCII_RAMP: array<u32, 12> = array<u32, 12>(
    46u, 44u, 39u, 58u, 59u, 33u, 43u, 42u, 61u, 35u, 36u, 64u
); // ".,':;!+*=#$@"

fn pack_depth(depth: f32) -> u32 {
    return u32(clamp(depth, 0.0, 1.0) * DEPTH_SCALE);
}

fn luminance_to_ascii(luminance: f32) -> u32 {
    let clamped = clamp(luminance, 0.0, 1.0);
    let idx = u32(round(clamped * 11.0));
    return ASCII_RAMP[min(idx, 11u)];
}

fn rotate_y(v: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        v.x * u.cos_az - v.z * u.sin_az,
        v.y,
        v.x * u.sin_az + v.z * u.cos_az,
    );
}

fn rotate_x(v: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        v.x,
        v.y * u.cos_al - v.z * u.sin_al,
        v.y * u.sin_al + v.z * u.cos_al,
    );
}

fn project(v: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        0.5 * u.logical_w + 0.5 * v.x * u.zoom,
        0.5 * u.logical_h - 0.5 * v.y * u.zoom,
        0.5 + 0.5 * v.z * u.zoom,
    );
}

// Pass 1: Transform and project each vertex.
@compute @workgroup_size(256)
fn vertex_transform(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= arrayLength(&vertices)) {
        return;
    }

    let pos = vertices[idx].position;
    let rotated = rotate_x(rotate_y(pos));
    let projected = project(rotated);
    transformed[idx] = vec4<f32>(projected, 0.0);
}

// Helper: sort 3 vec3s by x coordinate (bubble sort, returns indices into array)
fn sort3_by_x(a: vec3<f32>, b: vec3<f32>, c: vec3<f32>) -> array<vec3<f32>, 3> {
    var pts = array<vec3<f32>, 3>(a, b, c);
    // Simple 3-element sort
    if (pts[0].x > pts[1].x) {
        let tmp = pts[0]; pts[0] = pts[1]; pts[1] = tmp;
    }
    if (pts[1].x > pts[2].x) {
        let tmp = pts[1]; pts[1] = pts[2]; pts[2] = tmp;
    }
    if (pts[0].x > pts[1].x) {
        let tmp = pts[0]; pts[0] = pts[1]; pts[1] = tmp;
    }
    return pts;
}

fn get_y(pa: vec3<f32>, pb: vec3<f32>, x: f32) -> f32 {
    if (pa.x == pb.x) {
        return pa.y;
    }
    return pa.y + (pb.y - pa.y) * (x - pa.x) / (pb.x - pa.x);
}

// Pass 2a: Rasterize depth only (atomicMin).
@compute @workgroup_size(64)
fn rasterize_depth(@builtin(global_invocation_id) gid: vec3<u32>) {
    let tri_idx = gid.x;
    let num_triangles = arrayLength(&indices) / 3u;
    if (tri_idx >= num_triangles) {
        return;
    }

    let i0 = indices[tri_idx * 3u];
    let i1 = indices[tri_idx * 3u + 1u];
    let i2 = indices[tri_idx * 3u + 2u];

    let s0 = transformed[i0].xyz;
    let s1 = transformed[i1].xyz;
    let s2 = transformed[i2].xyz;

    // Back-face culling: 2D cross product on screen coords
    if ((s1.x - s0.x) * (s2.y - s1.y) < (s2.x - s1.x) * (s1.y - s0.y)) {
        return;
    }

    let pts = sort3_by_x(s0, s1, s2);

    // Triangle plane normal for Z interpolation
    let tri_normal = cross(s1 - s0, s2 - s0);
    var nz = tri_normal.z;
    if (nz == 0.0) {
        nz = 0.0001;
    }

    let xi = pts[0].x + u.dx / 2.0;
    let xf = pts[2].x - u.dx / 2.0;

    let x_start = max(i32(xi / u.dx), 0);
    let x_end = min(i32(xf / u.dx), i32(u.width) - 1);

    for (var xx: i32 = x_start; xx <= x_end; xx++) {
        let x = (f32(xx) + 0.5) * u.dx;

        var y1: f32;
        if (x <= pts[1].x) {
            y1 = get_y(pts[0], pts[1], x);
        } else {
            y1 = get_y(pts[1], pts[2], x);
        }
        let y2 = get_y(pts[0], pts[2], x);

        let yi = min(y1, y2);
        let yf = max(y1, y2);

        let y_start = max(i32((yi + u.dy / 2.0) / u.dy), 0);
        let y_end = min(i32((yf - u.dy / 2.0) / u.dy), i32(u.height) - 1);

        for (var yy: i32 = y_start; yy <= y_end; yy++) {
            let y = (f32(yy) + 0.5) * u.dy;

            let depth = pts[0].z - (tri_normal.x * (x - pts[0].x) + tri_normal.y * (y - pts[0].y)) / nz;

            let pixel_idx = u32(yy) * u.width + u32(xx);
            let packed = pack_depth(depth);
            atomicMin(&depth_buf[pixel_idx], packed);
        }
    }
}

// Pass 2b: Shade pixels whose depth matches the z-buffer winner.
@compute @workgroup_size(64)
fn rasterize_shade(@builtin(global_invocation_id) gid: vec3<u32>) {
    let tri_idx = gid.x;
    let num_triangles = arrayLength(&indices) / 3u;
    if (tri_idx >= num_triangles) {
        return;
    }

    let i0 = indices[tri_idx * 3u];
    let i1 = indices[tri_idx * 3u + 1u];
    let i2 = indices[tri_idx * 3u + 2u];

    let s0 = transformed[i0].xyz;
    let s1 = transformed[i1].xyz;
    let s2 = transformed[i2].xyz;

    // Back-face culling (same test as depth pass)
    if ((s1.x - s0.x) * (s2.y - s1.y) < (s2.x - s1.x) * (s1.y - s0.y)) {
        return;
    }

    // Compute lighting from rotated vertices
    let v0 = vertices[i0];
    let r0 = rotate_x(rotate_y(v0.position));
    let r1 = rotate_x(rotate_y(vertices[i1].position));
    let r2 = rotate_x(rotate_y(vertices[i2].position));

    let normal_vec = cross(r1 - r0, r2 - r0);
    let normal_len = length(normal_vec);
    if (normal_len < 1e-5) {
        return;
    }
    let normal = normal_vec / normal_len;
    let luminance = dot(-normal, u.light_dir) * 0.5 + 0.5;

    var tri_color = v0.color;
    if (u.has_fg_override != 0u) {
        tri_color = u.fg_override;
    }

    let pts = sort3_by_x(s0, s1, s2);

    let tri_normal = cross(s1 - s0, s2 - s0);
    var nz = tri_normal.z;
    if (nz == 0.0) {
        nz = 0.0001;
    }

    let xi = pts[0].x + u.dx / 2.0;
    let xf = pts[2].x - u.dx / 2.0;

    let x_start = max(i32(xi / u.dx), 0);
    let x_end = min(i32(xf / u.dx), i32(u.width) - 1);

    for (var xx: i32 = x_start; xx <= x_end; xx++) {
        let x = (f32(xx) + 0.5) * u.dx;

        var y1: f32;
        if (x <= pts[1].x) {
            y1 = get_y(pts[0], pts[1], x);
        } else {
            y1 = get_y(pts[1], pts[2], x);
        }
        let y2 = get_y(pts[0], pts[2], x);

        let yi = min(y1, y2);
        let yf = max(y1, y2);

        let y_start = max(i32((yi + u.dy / 2.0) / u.dy), 0);
        let y_end = min(i32((yf - u.dy / 2.0) / u.dy), i32(u.height) - 1);

        for (var yy: i32 = y_start; yy <= y_end; yy++) {
            let y = (f32(yy) + 0.5) * u.dy;

            let depth = pts[0].z - (tri_normal.x * (x - pts[0].x) + tri_normal.y * (y - pts[0].y)) / nz;

            let pixel_idx = u32(yy) * u.width + u32(xx);
            let packed = pack_depth(depth);
            let current = atomicLoad(&depth_buf[pixel_idx]);

            if (packed == current) {
                output_char[pixel_idx] = luminance_to_ascii(luminance);
                output_luminance[pixel_idx] = luminance;
                output_color[pixel_idx * 3u] = tri_color.x;
                output_color[pixel_idx * 3u + 1u] = tri_color.y;
                output_color[pixel_idx * 3u + 2u] = tri_color.z;
            }
        }
    }
}

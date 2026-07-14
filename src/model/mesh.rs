use bytemuck::{Pod, Zeroable};
use glam::Vec3;

/// A single mesh vertex, laid out for direct upload to the GPU (`Pod`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Vertex {
    /// Position in model space `(x, y, z)`.
    pub position: [f32; 3],
    /// Vertex normal `(x, y, z)`; `[0, 0, 0]` when the source file omits normals.
    pub normal: [f32; 3],
    /// Linear RGB base color in `[0, 1]`, from the material or a default gray.
    pub color: [f32; 3],
}

/// A triangle mesh: a vertex list plus triangle indices (3 per triangle).
#[derive(Debug)]
pub struct Mesh {
    /// The vertices referenced by [`indices`](Self::indices).
    pub vertices: Vec<Vertex>,
    /// Triangle indices into [`vertices`](Self::vertices), three per triangle.
    pub indices: Vec<u32>,
}

impl Mesh {
    /// Center the mesh at the origin and scale it to fit within the unit sphere.
    pub fn normalize(&mut self) {
        if self.vertices.is_empty() {
            return;
        }

        // A programmatically constructed mesh can bypass the loader's input
        // validation. Leave non-finite data untouched rather than spreading a
        // single NaN through every vertex.
        if self
            .vertices
            .iter()
            .any(|v| !v.position.iter().all(|coordinate| coordinate.is_finite()))
        {
            return;
        }

        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);

        for v in &self.vertices {
            let p = Vec3::from(v.position);
            min = min.min(p);
            max = max.max(p);
        }

        let center = (min + max) * 0.5;

        // Keep the established f32 path (and therefore snapshot output) for
        // ordinary meshes. Extreme but finite f32 coordinates can overflow the
        // center or length calculation, so detect that and normalize them in
        // f64 instead.
        let max_dist = self
            .vertices
            .iter()
            .map(|v| (Vec3::from(v.position) - center).length())
            .fold(0.0f32, f32::max);
        if center.is_finite() && max_dist.is_finite() {
            let scale = if max_dist > 0.0 { 1.0 / max_dist } else { 1.0 };
            for v in &mut self.vertices {
                v.position = ((Vec3::from(v.position) - center) * scale).into();
            }
            return;
        }

        let min = min.to_array().map(f64::from);
        let max = max.to_array().map(f64::from);
        let center = std::array::from_fn::<_, 3, _>(|axis| min[axis] * 0.5 + max[axis] * 0.5);
        let max_dist = self
            .vertices
            .iter()
            .map(|v| {
                v.position
                    .map(f64::from)
                    .into_iter()
                    .enumerate()
                    .map(|(axis, coordinate)| (coordinate - center[axis]).powi(2))
                    .sum::<f64>()
                    .sqrt()
            })
            .fold(0.0f64, f64::max);
        let scale = if max_dist > 0.0 { 1.0 / max_dist } else { 1.0 };
        for v in &mut self.vertices {
            v.position = std::array::from_fn(|axis| {
                ((f64::from(v.position[axis]) - center[axis]) * scale) as f32
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vert(x: f32, y: f32, z: f32) -> Vertex {
        Vertex {
            position: [x, y, z],
            normal: [0.0; 3],
            color: [0.0; 3],
        }
    }

    fn bbox_center(mesh: &Mesh) -> Vec3 {
        let mut min = Vec3::splat(f32::MAX);
        let mut max = Vec3::splat(f32::MIN);
        for v in &mesh.vertices {
            let p = Vec3::from(v.position);
            min = min.min(p);
            max = max.max(p);
        }
        (min + max) * 0.5
    }

    #[test]
    fn normalize_centers_and_fits_unit_sphere() {
        let mut mesh = Mesh {
            vertices: vec![
                vert(0.0, 0.0, 0.0),
                vert(2.0, 0.0, 0.0),
                vert(0.0, 2.0, 0.0),
                vert(0.0, 0.0, 2.0),
            ],
            indices: vec![0, 1, 2],
        };
        mesh.normalize();

        let max_dist = mesh
            .vertices
            .iter()
            .map(|v| Vec3::from(v.position).length())
            .fold(0.0f32, f32::max);
        assert!((max_dist - 1.0).abs() < 1e-5, "max_dist = {max_dist}");
        assert!(bbox_center(&mesh).length() < 1e-5, "not centered on origin");
    }

    #[test]
    fn normalize_empty_is_noop() {
        let mut mesh = Mesh {
            vertices: vec![],
            indices: vec![],
        };
        mesh.normalize();
        assert!(mesh.vertices.is_empty());
    }

    #[test]
    fn normalize_single_vertex_moves_to_origin() {
        let mut mesh = Mesh {
            vertices: vec![vert(5.0, -3.0, 2.0)],
            indices: vec![],
        };
        mesh.normalize();
        let p = Vec3::from(mesh.vertices[0].position);
        assert!(
            p.length() < 1e-5,
            "single vertex should land at origin, got {p}"
        );
    }

    #[test]
    fn normalize_handles_extreme_finite_coordinates() {
        let mut mesh = Mesh {
            vertices: vec![
                vert(f32::MAX, f32::MAX, 0.0),
                vert(-f32::MAX, -f32::MAX, 0.0),
            ],
            indices: vec![],
        };
        mesh.normalize();
        assert!(
            mesh.vertices
                .iter()
                .flat_map(|v| v.position)
                .all(f32::is_finite)
        );
        let max_dist = mesh
            .vertices
            .iter()
            .map(|v| Vec3::from(v.position).length())
            .fold(0.0f32, f32::max);
        assert!((max_dist - 1.0).abs() < 1e-5, "max_dist = {max_dist}");
    }
}

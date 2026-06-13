use bytemuck::{Pod, Zeroable};
use glam::Vec3;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub struct Vertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
}

pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl Mesh {
    /// Normalize the mesh to fit within [-1, 1] centered at origin.
    pub fn normalize(&mut self) {
        if self.vertices.is_empty() {
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

        // Center at origin, then find max distance (unit sphere, not unit box)
        let mut max_dist: f32 = 0.0;
        for v in &mut self.vertices {
            let p = Vec3::from(v.position) - center;
            v.position = p.into();
            max_dist = max_dist.max(p.length());
        }

        let scale = if max_dist > 0.0 { 1.0 / max_dist } else { 1.0 };
        for v in &mut self.vertices {
            let p = Vec3::from(v.position) * scale;
            v.position = p.into();
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
}

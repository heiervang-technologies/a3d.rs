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

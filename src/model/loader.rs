use std::path::Path;

use super::mesh::{Mesh, Vertex};

pub fn load_model(path: &Path) -> Mesh {
    match path.extension().and_then(|e| e.to_str()) {
        Some("obj") => load_obj(path),
        Some("stl") => load_stl(path),
        _ => panic!("Unsupported model format: {}", path.display()),
    }
}

fn load_obj(path: &Path) -> Mesh {
    let (models, materials) = tobj::load_obj(
        path,
        &tobj::LoadOptions {
            triangulate: true,
            single_index: true,
            ..Default::default()
        },
    )
    .expect("Failed to load OBJ");

    let materials = materials.unwrap_or_default();
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for model in &models {
        let mesh = &model.mesh;
        let base = vertices.len() as u32;

        let mat_color = mesh
            .material_id
            .and_then(|id| materials.get(id))
            .map(|m| m.diffuse.unwrap_or([0.8, 0.8, 0.8]))
            .unwrap_or([0.8, 0.8, 0.8]);

        for i in (0..mesh.positions.len()).step_by(3) {
            let normal = if mesh.normals.len() > i + 2 {
                [mesh.normals[i], mesh.normals[i + 1], mesh.normals[i + 2]]
            } else {
                [0.0, 0.0, 0.0]
            };

            vertices.push(Vertex {
                position: [mesh.positions[i], mesh.positions[i + 1], mesh.positions[i + 2]],
                normal,
                color: mat_color,
            });
        }

        for &idx in &mesh.indices {
            indices.push(base + idx);
        }
    }

    let mut mesh = Mesh { vertices, indices };
    mesh.normalize();
    mesh
}

fn load_stl(path: &Path) -> Mesh {
    let file = std::fs::File::open(path).expect("Failed to open STL file");
    let mut reader = std::io::BufReader::new(file);
    let stl = stl_io::read_stl(&mut reader).expect("Failed to parse STL");

    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for tri in &stl.faces {
        let normal = tri.normal.0;
        let base = vertices.len() as u32;

        for &vi in &tri.vertices {
            let pos = stl.vertices[vi].0;
            vertices.push(Vertex {
                position: pos,
                normal,
                color: [0.8, 0.8, 0.8],
            });
        }

        indices.extend_from_slice(&[base, base + 1, base + 2]);
    }

    let mut mesh = Mesh { vertices, indices };
    mesh.normalize();
    mesh
}

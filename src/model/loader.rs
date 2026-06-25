use std::path::Path;

use super::mesh::{Mesh, Vertex};

/// Load an OBJ or STL model from `path`, normalized to the unit sphere.
///
/// Returns a human-readable error rather than panicking when the path has an
/// unknown extension or the file is missing or malformed — model files are
/// untrusted input (see `SECURITY.md`), so callers should surface the error.
pub fn load_model(path: &Path) -> Result<Mesh, String> {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(str::to_ascii_lowercase);
    match ext.as_deref() {
        Some("obj") => load_obj(path),
        Some("stl") => load_stl(path),
        _ => Err(format!(
            "unsupported model format: {} (expected .obj or .stl)",
            path.display()
        )),
    }
}

fn load_obj(path: &Path) -> Result<Mesh, String> {
    let (models, materials) = tobj::load_obj(
        path,
        &tobj::LoadOptions {
            triangulate: true,
            single_index: true,
            ..Default::default()
        },
    )
    .map_err(|e| format!("failed to load OBJ {}: {e}", path.display()))?;

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
                position: [
                    mesh.positions[i],
                    mesh.positions[i + 1],
                    mesh.positions[i + 2],
                ],
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
    Ok(mesh)
}

fn load_stl(path: &Path) -> Result<Mesh, String> {
    let file =
        std::fs::File::open(path).map_err(|e| format!("failed to open {}: {e}", path.display()))?;
    let mut reader = std::io::BufReader::new(file);
    let stl = stl_io::read_stl(&mut reader)
        .map_err(|e| format!("failed to parse STL {}: {e}", path.display()))?;

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
    Ok(mesh)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_extension() {
        let err = load_model(Path::new("model.xyz")).unwrap_err();
        assert!(
            err.contains("unsupported model format"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn obj_extension_is_case_insensitive() {
        // An uppercase `.OBJ` is routed to the OBJ loader (case-insensitive) and
        // only then fails to load the missing file — proving recognition, not the
        // "unsupported model format" rejection a case-sensitive match would give.
        let err = load_model(Path::new("does-not-exist.OBJ")).unwrap_err();
        assert!(
            err.contains("failed to load OBJ"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn missing_stl_is_an_error_not_a_panic() {
        let err = load_model(Path::new("does-not-exist.stl")).unwrap_err();
        assert!(err.contains("failed to open"), "unexpected error: {err}");
    }
}

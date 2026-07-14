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

/// Reject a mesh with NaN/infinite vertex coordinates. A single non-finite
/// coordinate makes the bounding box infinite, so normalization turns *every*
/// vertex into NaN (`inf - inf`) and the model silently renders blank. Better to
/// surface it as a load error — model files are untrusted input.
fn reject_non_finite(mesh: &Mesh, path: &Path) -> Result<(), String> {
    if mesh
        .vertices
        .iter()
        .any(|v| !v.position.iter().all(|c| c.is_finite()))
    {
        return Err(format!(
            "{}: model has non-finite vertex coordinates (NaN or infinity)",
            path.display()
        ));
    }
    Ok(())
}

/// Enforce the structural invariants assumed by both rasterizers. Parsers
/// normally guarantee these, but keeping the check at the trust boundary makes
/// a parser regression a clean load error instead of an indexing panic or GPU
/// out-of-bounds access.
fn validate_structure(mesh: &Mesh, path: &Path) -> Result<(), String> {
    if mesh.vertices.is_empty() || mesh.indices.len() < 3 {
        return Err(format!("{}: model contains no triangles", path.display()));
    }
    if mesh.indices.len() % 3 != 0 {
        return Err(format!(
            "{}: model has an incomplete triangle index list",
            path.display()
        ));
    }
    if mesh
        .indices
        .iter()
        .any(|&index| index as usize >= mesh.vertices.len())
    {
        return Err(format!(
            "{}: model has an out-of-range vertex index",
            path.display()
        ));
    }
    Ok(())
}

fn finish_load(mut mesh: Mesh, path: &Path) -> Result<Mesh, String> {
    validate_structure(&mesh, path)?;
    reject_non_finite(&mesh, path)?;
    mesh.normalize();
    // `Mesh::normalize` is defensive, but verify the postcondition at this
    // untrusted-input boundary as well.
    reject_non_finite(&mesh, path)?;
    Ok(mesh)
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

    let materials = match materials {
        Ok(materials) => materials,
        Err(error) => {
            // A missing/broken sidecar material should not make otherwise valid
            // geometry unusable, but it should be diagnosable with RUST_LOG.
            log::warn!(
                "failed to load an OBJ material for {}: {error}",
                path.display()
            );
            Vec::new()
        }
    };
    let mut vertices = Vec::new();
    let mut indices = Vec::new();

    for model in &models {
        let mesh = &model.mesh;
        if mesh.positions.len() % 3 != 0 {
            return Err(format!(
                "{}: OBJ parser returned an incomplete vertex position",
                path.display()
            ));
        }
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

    finish_load(Mesh { vertices, indices }, path)
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

    finish_load(Mesh { vertices, indices }, path)
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

    #[test]
    fn rejects_non_finite_coordinates() {
        // tobj parses "inf" as f32::INFINITY. Without the guard, normalization
        // would spread NaN across the whole mesh and render blank; instead the
        // load must fail cleanly.
        let path = std::env::temp_dir().join("a3d_nonfinite_test.obj");
        std::fs::write(&path, "v inf 0 0\nv 0 1 0\nv 1 0 0\nf 1 2 3\n").unwrap();
        let result = load_model(&path);
        let _ = std::fs::remove_file(&path);
        let err = result.unwrap_err();
        assert!(err.contains("non-finite"), "unexpected error: {err}");
    }

    #[test]
    fn rejects_empty_obj() {
        let path = std::env::temp_dir().join(format!("a3d_empty_{}.obj", std::process::id()));
        std::fs::write(&path, "# no geometry\n").unwrap();
        let result = load_model(&path);
        let _ = std::fs::remove_file(&path);
        let err = result.unwrap_err();
        assert!(err.contains("no triangles"), "unexpected error: {err}");
    }
}

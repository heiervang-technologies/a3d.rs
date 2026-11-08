use std::path::Path;

use super::mesh::{Mesh, Vertex};

/// Load an OBJ, STL, or binary glTF model from `path`, normalized to the unit sphere.
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
        Some("glb") => load_glb(path),
        _ => Err(format!(
            "unsupported model format: {} (expected .obj, .stl, or .glb)",
            path.display()
        )),
    }
}

fn load_glb(path: &Path) -> Result<Mesh, String> {
    let bytes =
        std::fs::read(path).map_err(|e| format!("failed to open {}: {e}", path.display()))?;
    let gltf = gltf::Gltf::from_slice(&bytes)
        .map_err(|e| format!("failed to parse GLB {}: {e}", path.display()))?;
    let blob = gltf
        .blob
        .as_deref()
        .ok_or_else(|| format!("{}: GLB contains no binary buffer", path.display()))?;
    for buffer in gltf.buffers() {
        if !matches!(buffer.source(), gltf::buffer::Source::Bin)
            || buffer.index() != 0
            || buffer.length() > blob.len()
        {
            return Err(format!(
                "{}: GLB buffer is external or truncated",
                path.display()
            ));
        }
    }
    let scene = gltf
        .default_scene()
        .or_else(|| gltf.scenes().next())
        .ok_or_else(|| format!("{}: GLB contains no scene", path.display()))?;
    let mut mesh = Mesh {
        vertices: Vec::new(),
        indices: Vec::new(),
    };
    // Validate the selected scene before expanding any geometry. Iterative
    // traversal handles deep trees without consuming the call stack; visiting
    // each node once also rejects cycles and multiply-parented nodes.
    let mut visited = vec![false; gltf.nodes().len()];
    let mut pending: Vec<_> = scene
        .nodes()
        .map(|node| (node, glam::Mat4::IDENTITY))
        .collect();
    pending.reverse();
    let mut nodes = Vec::new();
    while let Some((node, parent)) = pending.pop() {
        if std::mem::replace(&mut visited[node.index()], true) {
            return Err(format!(
                "{}: GLB scene contains a cycle or repeated node",
                path.display()
            ));
        }
        let transform = parent * glam::Mat4::from_cols_array_2d(&node.transform().matrix());
        let children: Vec<_> = node.children().collect();
        pending.extend(children.into_iter().rev().map(|child| (child, transform)));
        nodes.push((node, transform));
    }
    for (node, transform) in nodes {
        append_glb_mesh(node, transform, blob, &mut mesh, path)?;
    }
    finish_load(mesh, path)
}

// gltf's reader assumes valid counts, strides, types, and byte ranges; its
// JSON validation alone does not establish these invariants.
fn validate_glb_accessor(accessor: &gltf::Accessor<'_>, blob: &[u8]) -> Result<(), String> {
    let count = accessor.count();
    let size = accessor.size();
    // Sparse accessors can expand a tiny file into a huge allocation.
    if count == 0 || count > (256 * 1024 * 1024) / size.max(16) {
        return Err("accessor count is zero or exceeds the 256 MiB decoded limit".into());
    }
    if let Some(view) = accessor.view() {
        checked_glb_range(&view, accessor.offset(), count, size, blob)?;
    } else if accessor.sparse().is_none() {
        return Err("accessor has neither a buffer view nor sparse values".into());
    }
    if let Some(sparse) = accessor.sparse() {
        if sparse.count() == 0 || sparse.count() > count {
            return Err("invalid sparse accessor count".into());
        }
        let indices = sparse.indices();
        let values = sparse.values();
        if indices.view().stride().is_some() || values.view().stride().is_some() {
            return Err("sparse buffer views must not have a stride".into());
        }
        let index_size = indices.index_type().size();
        let bytes = checked_glb_range(
            &indices.view(),
            indices.offset(),
            sparse.count(),
            index_size,
            blob,
        )?;
        checked_glb_range(&values.view(), values.offset(), sparse.count(), size, blob)?;
        let mut previous = None;
        for bytes in bytes.chunks_exact(index_size) {
            let mut word = [0; 4];
            word[..index_size].copy_from_slice(bytes);
            let index = u32::from_le_bytes(word) as usize;
            if index >= count || previous.is_some_and(|previous| index <= previous) {
                return Err("sparse indices must be increasing and within accessor bounds".into());
            }
            previous = Some(index);
        }
    }
    Ok(())
}

fn checked_glb_range<'a>(
    view: &gltf::buffer::View<'_>,
    offset: usize,
    count: usize,
    size: usize,
    blob: &'a [u8],
) -> Result<&'a [u8], String> {
    let invalid = || "accessor byte range or stride is invalid".to_string();
    let stride = view.stride().unwrap_or(size);
    if stride < size || count == 0 {
        return Err(invalid());
    }
    let end = count
        .checked_sub(1)
        .and_then(|n| n.checked_mul(stride))
        .and_then(|n| n.checked_add(size))
        .and_then(|n| n.checked_add(offset))
        .ok_or_else(invalid)?;
    let view_end = view
        .offset()
        .checked_add(view.length())
        .ok_or_else(invalid)?;
    if end > view.length() || view_end > view.buffer().length() {
        return Err(invalid());
    }
    blob.get(view.offset()..view_end)
        .and_then(|bytes| bytes.get(offset..end))
        .ok_or_else(invalid)
}

fn append_glb_mesh(
    node: gltf::Node<'_>,
    transform: glam::Mat4,
    blob: &[u8],
    output: &mut Mesh,
    path: &Path,
) -> Result<(), String> {
    if let Some(source_mesh) = node.mesh() {
        for primitive in source_mesh.primitives() {
            if primitive.mode() != gltf::mesh::Mode::Triangles {
                return Err(format!(
                    "{}: GLB primitive uses unsupported {:?} topology (expected triangles)",
                    path.display(),
                    primitive.mode()
                ));
            }
            use gltf::accessor::{DataType, Dimensions};
            for (label, accessor) in [
                ("position", primitive.get(&gltf::Semantic::Positions)),
                ("normal", primitive.get(&gltf::Semantic::Normals)),
                ("color", primitive.get(&gltf::Semantic::Colors(0))),
                ("index", primitive.indices()),
            ] {
                if let Some(accessor) = accessor {
                    let valid_type = match label {
                        "position" | "normal" => {
                            accessor.data_type() == DataType::F32
                                && accessor.dimensions() == Dimensions::Vec3
                        }
                        "color" => {
                            matches!(
                                accessor.data_type(),
                                DataType::F32 | DataType::U8 | DataType::U16
                            ) && matches!(
                                accessor.dimensions(),
                                Dimensions::Vec3 | Dimensions::Vec4
                            )
                        }
                        _ => {
                            matches!(
                                accessor.data_type(),
                                DataType::U8 | DataType::U16 | DataType::U32
                            ) && accessor.dimensions() == Dimensions::Scalar
                        }
                    };
                    if !valid_type {
                        return Err(format!(
                            "{}: GLB {label} accessor has an invalid type",
                            path.display()
                        ));
                    }
                    validate_glb_accessor(&accessor, blob).map_err(|error| {
                        format!(
                            "{}: GLB {label} accessor is unreadable: {error}",
                            path.display()
                        )
                    })?;
                }
            }
            let reader = primitive.reader(|buffer| (buffer.index() == 0).then_some(blob));
            let positions: Vec<_> = reader
                .read_positions()
                .ok_or_else(|| format!("{}: GLB primitive has no positions", path.display()))?
                .collect();
            let normals = reader.read_normals().map(Iterator::collect::<Vec<_>>);
            if primitive.get(&gltf::Semantic::Normals).is_some() && normals.is_none() {
                return Err(format!(
                    "{}: GLB normal accessor is unreadable",
                    path.display()
                ));
            }
            if normals.as_ref().is_some_and(|v| v.len() != positions.len()) {
                return Err(format!(
                    "{}: GLB position/normal counts differ",
                    path.display()
                ));
            }
            let colors = reader
                .read_colors(0)
                .map(|values| values.into_rgb_f32().collect::<Vec<_>>());
            if primitive.get(&gltf::Semantic::Colors(0)).is_some() && colors.is_none() {
                return Err(format!(
                    "{}: GLB color accessor is unreadable",
                    path.display()
                ));
            }
            if colors.as_ref().is_some_and(|v| v.len() != positions.len()) {
                return Err(format!(
                    "{}: GLB position/color counts differ",
                    path.display()
                ));
            }
            let factor = primitive
                .material()
                .pbr_metallic_roughness()
                .base_color_factor();
            let linear_transform = glam::Mat3::from_mat4(transform);
            let normal_transform = if linear_transform.determinant().abs() > f32::EPSILON {
                linear_transform.inverse().transpose()
            } else {
                glam::Mat3::ZERO
            };
            let base = u32::try_from(output.vertices.len())
                .map_err(|_| format!("{}: GLB has too many vertices", path.display()))?;
            let positions_len = positions.len();
            for (index, position) in positions.into_iter().enumerate() {
                let vertex_color = colors.as_ref().map_or([1.0; 3], |v| v[index]);
                output.vertices.push(Vertex {
                    position: transform
                        .transform_point3(glam::Vec3::from(position))
                        .into(),
                    normal: normals.as_ref().map_or([0.0; 3], |v| {
                        normal_transform
                            .mul_vec3(glam::Vec3::from(v[index]))
                            .normalize_or_zero()
                            .into()
                    }),
                    color: [
                        vertex_color[0] * factor[0],
                        vertex_color[1] * factor[1],
                        vertex_color[2] * factor[2],
                    ],
                });
            }
            let index_start = output.indices.len();
            if primitive.indices().is_some() {
                let indices = reader.read_indices().ok_or_else(|| {
                    format!("{}: GLB index accessor is unreadable", path.display())
                })?;
                for index in indices.into_u32() {
                    if index as usize >= positions_len {
                        return Err(format!(
                            "{}: GLB primitive has an out-of-range vertex index",
                            path.display()
                        ));
                    }
                    output.indices.push(
                        base.checked_add(index).ok_or_else(|| {
                            format!("{}: GLB vertex index overflow", path.display())
                        })?,
                    );
                }
                if (output.indices.len() - index_start) % 3 != 0 {
                    return Err(format!(
                        "{}: GLB primitive has an incomplete triangle index list",
                        path.display()
                    ));
                }
            } else {
                if positions_len % 3 != 0 {
                    return Err(format!(
                        "{}: non-indexed GLB primitive has an incomplete triangle",
                        path.display()
                    ));
                }
                let count = u32::try_from(output.vertices.len())
                    .map_err(|_| format!("{}: GLB has too many vertices", path.display()))?
                    - base;
                output.indices.extend(base..base + count);
            }
            // A reflection reverses winding. Keep the first vertex (flat color)
            // while swapping the other two so culling and lighting stay correct.
            if linear_transform.determinant() < 0.0 {
                for triangle in output.indices[index_start..].chunks_exact_mut(3) {
                    triangle.swap(1, 2);
                }
            }
        }
    }
    Ok(())
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

    fn test_glb() -> Vec<u8> {
        let json = r#"{"asset":{"version":"2.0"},"scene":0,"scenes":[{"nodes":[0]}],"nodes":[{"mesh":0,"scale":[2,1,1]}],"meshes":[{"primitives":[{"attributes":{"POSITION":0},"indices":1,"material":0}]}],"materials":[{"pbrMetallicRoughness":{"baseColorFactor":[0.25,0.5,0.75,1]}}],"buffers":[{"byteLength":44}],"bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":36},{"buffer":0,"byteOffset":36,"byteLength":6}],"accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","max":[1,1,0],"min":[0,0,0]},{"bufferView":1,"componentType":5123,"count":3,"type":"SCALAR"}]}"#;
        let mut json = json.as_bytes().to_vec();
        while json.len() % 4 != 0 {
            json.push(b' ');
        }
        let mut binary = Vec::new();
        for value in [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0] {
            binary.extend_from_slice(&value.to_le_bytes());
        }
        for index in [0u16, 1, 2] {
            binary.extend_from_slice(&index.to_le_bytes());
        }
        binary.extend_from_slice(&[0; 2]);

        let total_length = 12 + 8 + json.len() + 8 + binary.len();
        let mut glb = Vec::with_capacity(total_length);
        glb.extend_from_slice(b"glTF");
        glb.extend_from_slice(&2u32.to_le_bytes());
        glb.extend_from_slice(&(total_length as u32).to_le_bytes());
        glb.extend_from_slice(&(json.len() as u32).to_le_bytes());
        glb.extend_from_slice(&0x4e4f534au32.to_le_bytes());
        glb.extend_from_slice(&json);
        glb.extend_from_slice(&(binary.len() as u32).to_le_bytes());
        glb.extend_from_slice(&0x004e4942u32.to_le_bytes());
        glb.extend_from_slice(&binary);
        glb
    }

    // Rebuild a valid GLB container while changing its JSON or binary payload.
    fn edited_glb(edit: impl FnOnce(String, &mut Vec<u8>) -> String) -> Vec<u8> {
        let original = test_glb();
        let json_len = u32::from_le_bytes(original[12..16].try_into().unwrap()) as usize;
        let json = String::from_utf8(original[20..20 + json_len].to_vec()).unwrap();
        let mut binary = original[28 + json_len..].to_vec();
        let mut json = edit(json, &mut binary).into_bytes();
        while json.len() % 4 != 0 {
            json.push(b' ');
        }
        while binary.len() % 4 != 0 {
            binary.push(0);
        }
        let mut bytes = Vec::new();
        for word in [
            0x46546c67u32,
            2,
            (28 + json.len() + binary.len()) as u32,
            json.len() as u32,
            0x4e4f534a,
        ] {
            bytes.extend_from_slice(&word.to_le_bytes());
        }
        bytes.extend_from_slice(&json);
        bytes.extend_from_slice(&(binary.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&0x004e4942u32.to_le_bytes());
        bytes.extend_from_slice(&binary);
        bytes
    }

    fn load_bytes(bytes: Vec<u8>) -> Result<Mesh, String> {
        static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "a3d_glb_regression_{}_{id}.glb",
            std::process::id()
        ));
        std::fs::write(&path, bytes).unwrap();
        let result = load_model(&path);
        std::fs::remove_file(path).unwrap();
        result
    }

    #[test]
    fn rejects_malformed_accessor_layouts_without_panicking() {
        for (from, to) in [
            (r#""count":3"#, r#""count":0"#),
            (r#""count":3"#, r#""count":9223372036854775808"#),
            (r#""byteLength":36"#, r#""byteLength":36,"byteStride":4"#),
            (r#""byteOffset":0"#, r#""byteOffset":18446744073709551615"#),
            (
                r#""bufferView":0"#,
                r#""bufferView":0,"byteOffset":18446744073709551615"#,
            ),
            (r#""type":"VEC3""#, r#""type":"VEC2""#),
            (r#""componentType":5123"#, r#""componentType":5126"#),
            (r#""POSITION":0"#, r#""POSITION":0,"NORMAL":1"#),
            (r#""POSITION":0"#, r#""POSITION":0,"COLOR_0":1"#),
        ] {
            let result = load_bytes(edited_glb(|json, _| json.replace(from, to)));
            assert!(result.is_err(), "accepted {from} -> {to}");
        }
    }

    fn sparse_glb(edit: impl FnOnce(String, &mut Vec<u8>) -> String) -> Vec<u8> {
        edited_glb(|json, binary| {
            let positions = binary[..36].to_vec();
            binary.extend_from_slice(&[0, 1, 2, 0]);
            binary.extend_from_slice(&positions);
            let json = json
                .replace(r#""byteLength":44"#, r#""byteLength":84"#)
                .replace(
                    r#""byteLength":6}]"#,
                    r#""byteLength":6},{"buffer":0,"byteOffset":44,"byteLength":3},{"buffer":0,"byteOffset":48,"byteLength":36}]"#,
                )
                .replace(
                    r#""bufferView":0,"componentType"#,
                    r#""sparse":{"count":3,"indices":{"bufferView":2,"componentType":5121},"values":{"bufferView":3}},"componentType"#,
                );
            edit(json, binary)
        })
    }

    #[test]
    fn loads_sparse_positions_and_rejects_invalid_sparse_layouts() {
        let expected = load_bytes(test_glb()).unwrap();
        let actual = load_bytes(sparse_glb(|json, _| json)).unwrap();
        assert_eq!(actual.indices, expected.indices);
        for (actual, expected) in actual.vertices.iter().zip(&expected.vertices) {
            assert_eq!(actual.position, expected.position);
        }
        for (from, to) in [
            (r#""sparse":{"count":3"#, r#""sparse":{"count":0"#),
            (r#""sparse":{"count":3"#, r#""sparse":{"count":4"#),
            (r#""byteLength":3"#, r#""byteLength":2"#),
            (r#""byteOffset":48"#, r#""byteOffset":80"#),
            (
                r#""values":{"bufferView":3}"#,
                r#""values":{"bufferView":3,"byteOffset":18446744073709551615}"#,
            ),
            (
                r#""indices":{"bufferView":2"#,
                r#""indices":{"byteOffset":18446744073709551615,"bufferView":2"#,
            ),
            (r#""byteLength":3"#, r#""byteLength":3,"byteStride":4"#),
        ] {
            assert!(load_bytes(sparse_glb(|json, _| json.replace(from, to))).is_err());
        }
        for indices in [[0, 0, 2], [2, 1, 0], [0, 1, 3]] {
            assert!(
                load_bytes(sparse_glb(|json, binary| {
                    binary[44..47].copy_from_slice(&indices);
                    json
                }))
                .is_err()
            );
        }
    }

    #[test]
    fn rejects_cyclic_and_multiply_parented_scenes() {
        for nodes in [
            r#"[{"children":[0]}]"#,
            r#"[{"children":[1]},{"children":[0]}]"#,
            r#"[{"children":[1,2]},{"children":[2]},{"mesh":0}]"#,
        ] {
            let bytes =
                edited_glb(|json, _| json.replace(r#"[{"mesh":0,"scale":[2,1,1]}]"#, nodes));
            let error = load_bytes(bytes).unwrap_err();
            assert!(error.contains("cycle or repeated node"), "{error}");
        }
    }

    #[test]
    fn deep_scene_uses_bounded_iterative_traversal() {
        let bytes = edited_glb(|json, _| {
            let mut nodes: Vec<_> = (1..10_000)
                .map(|index| format!(r#"{{"children":[{index}]}}"#))
                .collect();
            nodes.push(r#"{"mesh":0,"scale":[2,1,1]}"#.into());
            json.replace(
                r#"[{"mesh":0,"scale":[2,1,1]}]"#,
                &format!("[{}]", nodes.join(",")),
            )
        });
        let mesh = load_bytes(bytes).unwrap();
        assert_eq!(mesh.indices, [0, 1, 2]);
    }

    #[test]
    fn reflections_preserve_visibility_for_indexed_and_nonindexed_meshes() {
        for indexed in [false, true] {
            for (nodes, reflected) in [
                (r#"[{"mesh":0,"scale":[-2,1,1]}]"#, true),
                (
                    r#"[{"scale":[-1,1,1],"children":[1]},{"mesh":0,"scale":[2,1,1]}]"#,
                    true,
                ),
                (
                    r#"[{"scale":[-1,1,1],"children":[1]},{"mesh":0,"scale":[-2,1,1]}]"#,
                    false,
                ),
            ] {
                let mesh = load_bytes(edited_glb(|json, _| {
                    let json = json.replace(r#"[{"mesh":0,"scale":[2,1,1]}]"#, nodes);
                    if indexed {
                        json
                    } else {
                        json.replace(r#","indices":1"#, "")
                    }
                }))
                .unwrap();
                assert_eq!(mesh.indices, if reflected { [0, 2, 1] } else { [0, 1, 2] });
                let mut fb = crate::render::Framebuffer::new(40, 20);
                crate::render_frame(
                    &mut fb,
                    &mesh,
                    std::f32::consts::PI,
                    0.0,
                    1.0,
                    glam::Vec3::Z,
                    None,
                );
                assert!(
                    fb.chars.iter().any(|&ch| ch != ' '),
                    "reflected geometry was culled"
                );
                assert_eq!(mesh.vertices[0].color, [0.25, 0.5, 0.75]);
            }
        }
    }

    #[test]
    fn rejects_truncated_binary_and_unreadable_declared_indices() {
        let truncated = edited_glb(|json, binary| {
            binary.truncate(36);
            json
        });
        assert!(
            load_bytes(truncated)
                .unwrap_err()
                .contains("buffer is external or truncated")
        );
        let unreadable =
            edited_glb(|json, _| json.replace(r#""byteOffset":36"#, r#""byteOffset":100"#));
        assert!(
            load_bytes(unreadable)
                .unwrap_err()
                .contains("index accessor is unreadable")
        );
    }

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
    fn loads_glb_geometry_transforms_and_material_color() {
        let path = std::env::temp_dir().join(format!("a3d_valid_{}.GLB", std::process::id()));
        std::fs::write(&path, test_glb()).unwrap();
        let result = load_model(&path);
        let _ = std::fs::remove_file(&path);
        let mesh = result.unwrap();

        assert_eq!(mesh.vertices.len(), 3);
        assert_eq!(mesh.indices, [0, 1, 2]);
        assert_eq!(mesh.vertices[0].color, [0.25, 0.5, 0.75]);
        let x_extent = mesh
            .vertices
            .iter()
            .map(|v| v.position[0])
            .fold(f32::NEG_INFINITY, f32::max)
            - mesh
                .vertices
                .iter()
                .map(|v| v.position[0])
                .fold(f32::INFINITY, f32::min);
        let y_extent = mesh
            .vertices
            .iter()
            .map(|v| v.position[1])
            .fold(f32::NEG_INFINITY, f32::max)
            - mesh
                .vertices
                .iter()
                .map(|v| v.position[1])
                .fold(f32::INFINITY, f32::min);
        assert!((x_extent / y_extent - 2.0).abs() < 1e-5);
    }

    #[test]
    fn malformed_glb_is_a_clean_error() {
        let path = std::env::temp_dir().join(format!("a3d_bad_{}.glb", std::process::id()));
        std::fs::write(&path, b"not a GLB").unwrap();
        let result = load_model(&path);
        let _ = std::fs::remove_file(&path);
        let err = result.unwrap_err();
        assert!(
            err.contains("failed to parse GLB"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn glb_indices_must_stay_within_their_primitive() {
        let mut bytes = test_glb();
        // The fixture's final eight bytes are three u16 indices plus padding.
        // Change the last index from 2 to 3; a three-vertex primitive may only
        // reference 0..=2, regardless of vertices from later primitives.
        let index_offset = bytes.len() - 4;
        bytes[index_offset..index_offset + 2].copy_from_slice(&3u16.to_le_bytes());
        let path = std::env::temp_dir().join(format!("a3d_bad_index_{}.glb", std::process::id()));
        std::fs::write(&path, bytes).unwrap();
        let result = load_model(&path);
        let _ = std::fs::remove_file(&path);
        let err = result.unwrap_err();
        assert!(
            err.contains("GLB primitive has an out-of-range vertex index"),
            "unexpected error: {err}"
        );
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

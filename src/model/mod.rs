mod loader;
/// [`Mesh`]/[`Vertex`](mesh::Vertex) types and unit-sphere normalization.
pub mod mesh;

pub use loader::load_model;
pub use mesh::Mesh;

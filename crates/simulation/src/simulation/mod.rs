use crate::simulation::{
    edges::EdgeEntityMap, resources::ResourceField, vertices::VertexEntityMap,
};

pub mod bounds;
pub mod diffusion;
pub mod edges;
pub mod resources;
pub mod vertices;

pub struct Simulation {
    pub resources: ResourceField,
    pub vertices: VertexEntityMap,
    pub edges: EdgeEntityMap,
}

//! Original import paths for the field, graph indexes, and normalization.

use crate::{
    field::ResourceField,
    graph::{EdgeEntityMap, VertexEntityMap},
};

pub use crate::normalization as bounds;

pub mod diffusion {
    pub use crate::field::{Diffuser, Neighbourhood};
}

pub mod edges {
    pub use crate::graph::{EdgeEntity, EdgeEntityMap};
}

pub mod resources {
    pub use crate::field::ResourceField;
}

pub mod vertices {
    pub use crate::graph::{VertexEntity, VertexEntityMap};
}

pub struct Simulation {
    pub resources: ResourceField,
    pub vertices: VertexEntityMap,
    pub edges: EdgeEntityMap,
}

use slotmap::new_key_type;
use thiserror::Error;

use super::VertexId;

new_key_type! {
    pub struct EdgeId;
}

#[derive(Debug)]
pub struct Edge {
    pub(super) endpoints: [VertexId; 2],
}

impl Edge {
    pub fn endpoints(&self) -> [VertexId; 2] {
        self.endpoints
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum AddEdgeError {
    #[error("unknown vertex {0:?}")]
    UnknownVertex(VertexId),

    #[error("edge {0:?} already connects these vertices")]
    DuplicateEdge(EdgeId),
}

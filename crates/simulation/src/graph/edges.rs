use slotmap::new_key_type;
use thiserror::Error;

use super::VertexId;

new_key_type! {
    /// Identifies a live edge within the graph that issued it.
    pub struct EdgeId;
}

/// An undirected connection between two vertices.
#[derive(Debug)]
pub struct Edge {
    /// Endpoints in insertion order.
    pub(super) endpoints: [VertexId; 2],
}

impl Edge {
    /// Returns the endpoints in insertion order, including both copies for a self-loop.
    pub fn endpoints(&self) -> [VertexId; 2] {
        self.endpoints
    }
}

/// An edge insertion rejected before the graph was modified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum AddEdgeError {
    /// The endpoint ID has no live entry in the graph.
    #[error("unknown vertex {0:?}")]
    UnknownVertex(VertexId),

    /// The existing edge already joins the same unordered endpoint pair.
    #[error("edge {0:?} already connects these vertices")]
    DuplicateEdge(EdgeId),
}

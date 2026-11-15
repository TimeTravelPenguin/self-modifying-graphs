use slotmap::DenseSlotMap;

use crate::Position;

mod edges;
mod vertices;

pub use edges::{AddEdgeError, Edge, EdgeId};
pub use vertices::{Uptake, Vertex, VertexId};

/// Mutable undirected graph with one edge per unordered endpoint pair.
///
/// Vertex and edge IDs belong to the graph that issued them. Placement rules
/// for the resource field are enforced by the simulation.
#[derive(Debug, Default)]
pub struct Graph {
    vertices: DenseSlotMap<VertexId, Vertex>,
    edges: DenseSlotMap<EdgeId, Edge>,
}

impl Graph {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_vertex(&mut self, position: Position, uptake: Uptake) -> VertexId {
        self.vertices.insert(Vertex::new(position, uptake))
    }

    pub fn remove_vertex(&mut self, vertex_id: VertexId) -> Option<Vertex> {
        let mut vertex = self.vertices.remove(vertex_id)?;

        for edge_id in vertex.incident_edges.drain(..) {
            self.remove_edge(edge_id);
        }

        Some(vertex)
    }

    pub fn vertex(&self, vertex_id: VertexId) -> Option<&Vertex> {
        self.vertices.get(vertex_id)
    }

    pub fn vertices(&self) -> impl Iterator<Item = (VertexId, &Vertex)> {
        self.vertices.iter()
    }

    pub(crate) fn vertices_mut(&mut self) -> impl Iterator<Item = &mut Vertex> {
        self.vertices.values_mut()
    }

    pub fn add_edge(&mut self, from: VertexId, to: VertexId) -> Result<EdgeId, AddEdgeError> {
        let from_vertex = self
            .vertices
            .get(from)
            .ok_or(AddEdgeError::UnknownVertex(from))?;
        let to_vertex = self
            .vertices
            .get(to)
            .ok_or(AddEdgeError::UnknownVertex(to))?;

        if let Some(edge_id) = self.find_edge(from, from_vertex, to, to_vertex) {
            return Err(AddEdgeError::DuplicateEdge(edge_id));
        }

        let edge_id = self.edges.insert(Edge {
            endpoints: [from, to],
        });

        self.vertices[from].incident_edges.push(edge_id);

        if from != to {
            self.vertices[to].incident_edges.push(edge_id);
        }

        Ok(edge_id)
    }

    pub fn edge_between(&self, from: VertexId, to: VertexId) -> Option<EdgeId> {
        let from_vertex = self.vertices.get(from)?;
        let to_vertex = self.vertices.get(to)?;

        self.find_edge(from, from_vertex, to, to_vertex)
    }

    fn find_edge(
        &self,
        from: VertexId,
        from_vertex: &Vertex,
        to: VertexId,
        to_vertex: &Vertex,
    ) -> Option<EdgeId> {
        let candidates = if from_vertex.incident_edges.len() <= to_vertex.incident_edges.len() {
            &from_vertex.incident_edges
        } else {
            &to_vertex.incident_edges
        };

        candidates.iter().copied().find(|&edge_id| {
            let endpoints = self.edges[edge_id].endpoints;
            endpoints == [from, to] || endpoints == [to, from]
        })
    }

    pub fn edge(&self, edge_id: EdgeId) -> Option<&Edge> {
        self.edges.get(edge_id)
    }

    pub fn edges(&self) -> impl Iterator<Item = (EdgeId, &Edge)> {
        self.edges.iter()
    }

    pub fn remove_edge(&mut self, edge_id: EdgeId) -> Option<Edge> {
        let edge = self.edges.remove(edge_id)?;
        let [from, to] = edge.endpoints;

        self.unlink_edge(from, edge_id);

        if from != to {
            self.unlink_edge(to, edge_id);
        }

        Some(edge)
    }

    fn unlink_edge(&mut self, vertex_id: VertexId, edge_id: EdgeId) {
        // Cascade removal has already taken the deleted vertex out of the graph.
        if let Some(vertex) = self.vertices.get_mut(vertex_id) {
            let index = vertex
                .incident_edges
                .iter()
                .position(|&incident_id| incident_id == edge_id)
                .expect("edge must be in its endpoint's incident list");
            vertex.incident_edges.swap_remove(index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AddEdgeError, Graph, Uptake};

    #[test]
    fn undirected_edges_and_self_loops_have_one_record() {
        let mut graph = Graph::new();
        let uptake = Uptake::new(0.0, 1.0).unwrap();
        let from = graph.add_vertex((0, 0).into(), uptake);
        let to = graph.add_vertex((1, 0).into(), uptake);

        let edge = graph.add_edge(from, to).unwrap();
        let self_loop = graph.add_edge(from, from).unwrap();

        assert_eq!(graph.edge_between(from, to), Some(edge));
        assert_eq!(graph.edge_between(to, from), Some(edge));
        assert_eq!(
            graph.add_edge(to, from),
            Err(AddEdgeError::DuplicateEdge(edge))
        );
        assert_eq!(
            graph.add_edge(from, from),
            Err(AddEdgeError::DuplicateEdge(self_loop))
        );
        assert_eq!(
            graph.vertex(from).unwrap().incident_edges(),
            &[edge, self_loop]
        );
        assert_eq!(graph.vertex(to).unwrap().incident_edges(), &[edge]);
        assert_eq!(graph.edges().count(), 2);

        assert_eq!(
            graph.remove_edge(self_loop).unwrap().endpoints(),
            [from, from]
        );
        assert_eq!(graph.vertex(from).unwrap().incident_edges(), &[edge]);
        assert_eq!(graph.edge_between(from, from), None);
    }

    #[test]
    fn vertex_removal_unlinks_edges_and_invalidates_ids() {
        let mut graph = Graph::new();
        let uptake = Uptake::new(0.0, 1.0).unwrap();
        let center = graph.add_vertex((0, 0).into(), uptake);
        let left = graph.add_vertex((1, 0).into(), uptake);
        let right = graph.add_vertex((2, 0).into(), uptake);

        let left_edge = graph.add_edge(left, center).unwrap();
        let right_edge = graph.add_edge(center, right).unwrap();
        let self_loop = graph.add_edge(center, center).unwrap();
        let surviving_edge = graph.add_edge(left, right).unwrap();

        graph.remove_vertex(center).unwrap();
        let replacement = graph.add_vertex((0, 0).into(), uptake);
        let replacement_edge = graph.add_edge(replacement, left).unwrap();

        assert_ne!(replacement, center);
        assert!(graph.vertex(center).is_none());
        assert_eq!(
            graph.add_edge(left, center),
            Err(AddEdgeError::UnknownVertex(center))
        );

        for removed in [left_edge, right_edge, self_loop] {
            assert_ne!(replacement_edge, removed);
            assert!(graph.edge(removed).is_none());
        }

        assert_eq!(graph.edge_between(left, right), Some(surviving_edge));
        assert_eq!(
            graph.vertex(left).unwrap().incident_edges(),
            &[surviving_edge, replacement_edge]
        );
        assert_eq!(
            graph.vertex(right).unwrap().incident_edges(),
            &[surviving_edge]
        );
    }
}

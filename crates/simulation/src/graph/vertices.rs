use slotmap::new_key_type;

use super::EdgeId;
use crate::Position;

new_key_type! {
    /// Identifies a live vertex within the graph that issued it.
    pub struct VertexId;
}

/// Constant-rate uptake from a vertex's home cell.
///
/// The rate is measured in resource units per unit of simulation time. A step
/// withdraws at most the amount available in the cell and stores the withdrawn
/// amount multiplied by the efficiency.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Uptake {
    rate: f64,
    efficiency: f64,
}

impl Uptake {
    /// Creates a rate and retained fraction for resource uptake.
    ///
    /// Returns `None` unless the rate is finite and nonnegative and efficiency is in `0..=1`.
    pub fn new(rate: f64, efficiency: f64) -> Option<Self> {
        if !rate.is_finite() || rate < 0.0 || !(0.0..=1.0).contains(&efficiency) {
            return None;
        }

        Some(Self { rate, efficiency })
    }
}

/// A vertex's lattice position, stored resources, and incident edges.
#[derive(Debug)]
pub struct Vertex {
    position: Position,
    stored_resource: f64,
    uptake: Uptake,
    /// IDs of the edges incident to this vertex.
    pub(super) incident_edges: Vec<EdgeId>,
}

impl Vertex {
    /// Creates an isolated vertex with zero stored resources.
    pub(super) fn new(position: Position, uptake: Uptake) -> Self {
        Self {
            position,
            stored_resource: 0.0,
            uptake,
            incident_edges: Vec::new(),
        }
    }

    /// Returns the vertex's home cell.
    pub fn position(&self) -> Position {
        self.position
    }

    /// Returns how much resource the vertex retained from uptake.
    pub fn stored_resource(&self) -> f64 {
        self.stored_resource
    }

    /// Returns the incident edge IDs, with any self-loop listed once.
    ///
    /// The order is unspecified and can change when edges are removed.
    pub fn incident_edges(&self) -> &[EdgeId] {
        &self.incident_edges
    }

    /// Withdraws available resources and credits the efficiency-adjusted amount.
    pub(crate) fn consume(&mut self, available: &mut f64, dt: f64) {
        let withdrawn = (self.uptake.rate * dt).min(*available);
        *available -= withdrawn;
        self.stored_resource += self.uptake.efficiency * withdrawn;
    }
}

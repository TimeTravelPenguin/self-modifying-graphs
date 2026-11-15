use slotmap::new_key_type;

use super::EdgeId;
use crate::Position;

new_key_type! {
    pub struct VertexId;
}

/// Constant-rate uptake from a vertex's home cell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Uptake {
    rate: f64,
    efficiency: f64,
}

impl Uptake {
    /// Returns `None` unless the rate is finite and nonnegative and efficiency is in `0..=1`.
    pub fn new(rate: f64, efficiency: f64) -> Option<Self> {
        if !rate.is_finite() || rate < 0.0 || !(0.0..=1.0).contains(&efficiency) {
            return None;
        }

        Some(Self { rate, efficiency })
    }
}

#[derive(Debug)]
pub struct Vertex {
    position: Position,
    stored_resource: f64,
    uptake: Uptake,
    pub(super) incident_edges: Vec<EdgeId>,
}

impl Vertex {
    pub(super) fn new(position: Position, uptake: Uptake) -> Self {
        Self {
            position,
            stored_resource: 0.0,
            uptake,
            incident_edges: Vec::new(),
        }
    }

    pub fn position(&self) -> Position {
        self.position
    }

    pub fn stored_resource(&self) -> f64 {
        self.stored_resource
    }

    pub fn incident_edges(&self) -> &[EdgeId] {
        &self.incident_edges
    }

    pub(crate) fn consume(&mut self, available: &mut f64, dt: f64) {
        let withdrawn = (self.uptake.rate * dt).min(*available);
        *available -= withdrawn;
        self.stored_resource += self.uptake.efficiency * withdrawn;
    }
}

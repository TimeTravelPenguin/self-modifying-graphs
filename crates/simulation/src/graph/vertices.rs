use slotmap::new_key_type;

use super::EdgeId;
use crate::Position;

new_key_type! {
    /// Identifies a live vertex within the graph that issued it.
    pub struct VertexId;
}

/// Resource uptake from a vertex's home cell.
///
/// Uptake can use a constant rate or a rate that saturates as the cell fills.
/// A step withdraws at most the amount available and retains the withdrawn
/// amount multiplied by the efficiency.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Uptake {
    rate: f64,
    efficiency: f64,
    model: UptakeModel,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum UptakeModel {
    Constant,
    Saturating { half_saturation: f64 },
}

impl Uptake {
    /// Creates constant-rate uptake in resource units per unit of model time.
    ///
    /// Returns `None` unless the rate is finite and nonnegative and efficiency is in `0..=1`.
    pub fn new(rate: f64, efficiency: f64) -> Option<Self> {
        Self::with_model(rate, efficiency, UptakeModel::Constant)
    }

    /// Creates uptake with a rate of `max_rate * available / (half_saturation + available)`.
    ///
    /// `max_rate` is measured in resource units per unit of model time.
    /// `half_saturation` is the cell amount at which the rate reaches half its maximum.
    /// Returns `None` if the maximum rate or efficiency fails [`Self::new`]'s
    /// validation, or if half-saturation is non-finite or non-positive.
    pub fn saturating(max_rate: f64, half_saturation: f64, efficiency: f64) -> Option<Self> {
        if !half_saturation.is_finite() || half_saturation <= 0.0 {
            return None;
        }

        Self::with_model(
            max_rate,
            efficiency,
            UptakeModel::Saturating { half_saturation },
        )
    }

    fn with_model(rate: f64, efficiency: f64, model: UptakeModel) -> Option<Self> {
        if !rate.is_finite() || rate < 0.0 || !(0.0..=1.0).contains(&efficiency) {
            return None;
        }

        Some(Self {
            rate,
            efficiency,
            model,
        })
    }

    fn requested_amount(self, available: f64, dt: f64) -> f64 {
        let rate = match self.model {
            UptakeModel::Constant => self.rate,
            UptakeModel::Saturating { half_saturation } => {
                // Scale by the larger quantity to keep the denominator finite.
                let saturation = if available >= half_saturation {
                    1.0 / (1.0 + half_saturation / available)
                } else {
                    let ratio = available / half_saturation;
                    ratio / (1.0 + ratio)
                };

                self.rate * saturation
            }
        };

        rate * dt
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
        let withdrawn = self.uptake.requested_amount(*available, dt).min(*available);
        *available -= withdrawn;
        self.stored_resource += self.uptake.efficiency * withdrawn;
    }
}

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

/// Uptake and living cost for a vertex.
///
/// An [`Uptake`] value can be passed on its own to create a vertex with zero living cost.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VertexConfig {
    uptake: Uptake,
    living_cost: f64,
}

impl VertexConfig {
    /// Configures uptake and the resource amount due per tick.
    ///
    /// Returns `None` if `living_cost` is negative or non-finite.
    pub fn new(uptake: Uptake, living_cost: f64) -> Option<Self> {
        if !living_cost.is_finite() || living_cost < 0.0 {
            return None;
        }

        Some(Self {
            uptake,
            living_cost,
        })
    }
}

impl From<Uptake> for VertexConfig {
    fn from(uptake: Uptake) -> Self {
        Self {
            uptake,
            living_cost: 0.0,
        }
    }
}

/// A vertex's lattice position, resource balance, and incident edges.
#[derive(Debug)]
pub struct Vertex {
    position: Position,
    stored_resource: f64,
    config: VertexConfig,
    /// IDs of the edges incident to this vertex.
    pub(super) incident_edges: Vec<EdgeId>,
}

impl Vertex {
    /// Creates an isolated vertex with zero stored resources.
    pub(super) fn new(position: Position, config: VertexConfig) -> Self {
        Self {
            position,
            stored_resource: 0.0,
            config,
            incident_edges: Vec::new(),
        }
    }

    /// Returns the vertex's home cell.
    pub fn position(&self) -> Position {
        self.position
    }

    /// Returns the vertex's current stored resource balance.
    pub fn stored_resource(&self) -> f64 {
        self.stored_resource
    }

    /// Returns the resource amount due from this vertex each tick.
    pub fn living_cost(&self) -> f64 {
        self.config.living_cost
    }

    /// Returns the incident edge IDs, with any self-loop listed once.
    ///
    /// The order is unspecified and can change when edges are removed.
    pub fn incident_edges(&self) -> &[EdgeId] {
        &self.incident_edges
    }

    /// Withdraws available resources and credits the efficiency-adjusted amount.
    pub(crate) fn consume(&mut self, available: &mut f64, dt: f64) {
        let uptake = self.config.uptake;
        let withdrawn = uptake.requested_amount(*available, dt).min(*available);
        *available -= withdrawn;
        self.stored_resource += uptake.efficiency * withdrawn;
    }

    /// Pays as much of the per-tick living cost as the current balance covers.
    pub(crate) fn pay_living_cost(&mut self) {
        let payment = self.config.living_cost.min(self.stored_resource);
        self.stored_resource -= payment;
    }
}

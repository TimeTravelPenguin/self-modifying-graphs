use slotmap::new_key_type;
use thiserror::Error;

use super::EdgeId;
use crate::{
    Position,
    effects::{EffectContext, EffectMultipliers, EffectsConfig},
};

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

    fn requested_amount(self, available: f64, dt: f64, capacity: f64) -> f64 {
        let capacity = self.rate * capacity;
        let rate = match self.model {
            UptakeModel::Constant => capacity,
            UptakeModel::Saturating { half_saturation } => {
                // Scale by the larger quantity to keep the denominator finite.
                let saturation = if available >= half_saturation {
                    1.0 / (1.0 + half_saturation / available)
                } else {
                    let ratio = available / half_saturation;
                    ratio / (1.0 + ratio)
                };

                capacity * saturation
            }
        };

        rate * dt
    }
}

/// Rules for health damage, recovery, and health-based death.
///
/// Damage and recovery are measured in health points per tick. Age is measured
/// in elapsed model time and advances even when health is disabled.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HealthConfig {
    /// Disables health damage, recovery, and health-based death.
    Disabled,
    /// Enables a bounded health reserve that starts at its maximum.
    Enabled {
        /// Finite, positive initial health and upper bound.
        max_health: f64,
        /// Finite, nonnegative damage for a completely unpaid living cost.
        damage_per_unpaid_tick: f64,
        /// Finite, nonnegative base recovery after a fully paid living cost.
        recovery_per_paid_tick: f64,
    },
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self::Enabled {
            max_health: 10.0,
            damage_per_unpaid_tick: 1.0,
            recovery_per_paid_tick: 0.25,
        }
    }
}

/// Errors from configuring vertex health, living costs, or amplified effects.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum VertexConfigError {
    /// Living cost must be finite and nonnegative.
    #[error("living cost must be finite and nonnegative, got {0}")]
    InvalidLivingCost(f64),
    /// Maximum health must be finite and positive.
    #[error("maximum health must be finite and positive, got {0}")]
    InvalidMaxHealth(f64),
    /// Damage must be finite and nonnegative.
    #[error("damage per unpaid tick must be finite and nonnegative, got {0}")]
    InvalidDamage(f64),
    /// Recovery must be finite and nonnegative.
    #[error("recovery per paid tick must be finite and nonnegative, got {0}")]
    InvalidRecovery(f64),
    /// The effect set can overflow the base extraction capacity.
    #[error("effects can produce a non-finite extraction capacity")]
    NonFiniteUptakeCapacity,
    /// The effect set can overflow the base health recovery amount.
    #[error("effects can produce a non-finite health recovery amount")]
    NonFiniteRecovery,
    /// A live vertex cannot issue another configuration revision.
    #[error("vertex effect configuration revision is exhausted")]
    EffectsRevisionExhausted,
}

/// Validated uptake, living cost, health rules, and shared effects for a vertex.
#[derive(Debug, Clone, PartialEq)]
pub struct VertexConfig {
    uptake: Uptake,
    living_cost: f64,
    health: HealthConfig,
    effects: EffectsConfig,
}

impl VertexConfig {
    /// Configures uptake, the resource amount due per tick, and an explicit health mode.
    ///
    /// Starts with an empty effect set. Use [`Self::with_effects`] to configure
    /// extraction and recovery modifiers.
    ///
    /// # Errors
    ///
    /// Returns an error if a living cost or health parameter is invalid.
    pub fn new(
        uptake: Uptake,
        living_cost: f64,
        health: HealthConfig,
    ) -> Result<Self, VertexConfigError> {
        if !living_cost.is_finite() || living_cost < 0.0 {
            return Err(VertexConfigError::InvalidLivingCost(living_cost));
        }

        if let HealthConfig::Enabled {
            max_health,
            damage_per_unpaid_tick,
            recovery_per_paid_tick,
        } = health
        {
            if !max_health.is_finite() || max_health <= 0.0 {
                return Err(VertexConfigError::InvalidMaxHealth(max_health));
            }

            if !damage_per_unpaid_tick.is_finite() || damage_per_unpaid_tick < 0.0 {
                return Err(VertexConfigError::InvalidDamage(damage_per_unpaid_tick));
            }

            if !recovery_per_paid_tick.is_finite() || recovery_per_paid_tick < 0.0 {
                return Err(VertexConfigError::InvalidRecovery(recovery_per_paid_tick));
            }
        }

        Ok(Self {
            uptake,
            living_cost,
            health,
            effects: EffectsConfig::default(),
        })
    }

    /// Attaches a shared effect set after checking its amplified base quantities.
    ///
    /// # Errors
    ///
    /// Returns an error if an effect can make extraction capacity or enabled
    /// health recovery non-finite.
    pub fn with_effects(mut self, effects: EffectsConfig) -> Result<Self, VertexConfigError> {
        let maximum = effects.maximum_multipliers();

        if !(self.uptake.rate * maximum.uptake_capacity).is_finite() {
            return Err(VertexConfigError::NonFiniteUptakeCapacity);
        }

        if let HealthConfig::Enabled {
            recovery_per_paid_tick,
            ..
        } = self.health
            && !(recovery_per_paid_tick * maximum.recovery_amount).is_finite()
        {
            return Err(VertexConfigError::NonFiniteRecovery);
        }

        self.effects = effects;
        Ok(self)
    }

    /// Returns the unmodified environmental uptake law.
    pub fn uptake(&self) -> Uptake {
        self.uptake
    }

    /// Returns the resource amount due each tick.
    pub fn living_cost(&self) -> f64 {
        self.living_cost
    }

    /// Returns the health mode and base health parameters.
    pub fn health(&self) -> HealthConfig {
        self.health
    }

    /// Borrows the immutable shared effect set.
    pub fn effects(&self) -> &EffectsConfig {
        &self.effects
    }
}

/// A living-cost payment evaluated before any health transition.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PaymentOutcome {
    pub(crate) due: f64,
    pub(crate) paid: f64,
}

/// A vertex's lattice position, resource balance, vitality, and incident edges.
#[derive(Debug)]
pub struct Vertex {
    position: Position,
    stored_resource: f64,
    age: f64,
    health: Option<f64>,
    effects_revision: u64,
    config: VertexConfig,
    /// IDs of the edges incident to this vertex.
    pub(super) incident_edges: Vec<EdgeId>,
}

impl Vertex {
    /// Creates an isolated vertex with zero stored resources, age zero, and full enabled health.
    pub(super) fn new(position: Position, config: VertexConfig) -> Self {
        let health = match config.health {
            HealthConfig::Disabled => None,
            HealthConfig::Enabled { max_health, .. } => Some(max_health),
        };

        Self {
            position,
            stored_resource: 0.0,
            age: 0.0,
            health,
            effects_revision: 0,
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

    /// Borrows the applied vertex configuration.
    pub fn config(&self) -> &VertexConfig {
        &self.config
    }

    /// Returns age in elapsed model-time units.
    pub fn age(&self) -> f64 {
        self.age
    }

    /// Returns the enabled health reserve, or `None` when health is disabled.
    pub fn health(&self) -> Option<f64> {
        self.health
    }

    /// Returns enabled health normalized to `0..=1`, or `1` when health is disabled.
    pub fn health_fraction(&self) -> f64 {
        match (self.health, self.config.health) {
            (Some(health), HealthConfig::Enabled { max_health, .. }) => {
                (health / max_health).clamp(0.0, 1.0)
            }
            _ => 1.0,
        }
    }

    /// Returns the current effect configuration revision, initially zero.
    pub fn effects_revision(&self) -> u64 {
        self.effects_revision
    }

    /// Calculates multipliers from the current state for the next tick.
    pub fn effective_effects(&self) -> EffectMultipliers {
        self.config.effects.evaluate(&EffectContext {
            age: self.age,
            health_fraction: self.health_fraction(),
        })
    }

    /// Returns potential recovery after a fully paid next tick, or `None` if health is disabled.
    pub fn effective_recovery(&self) -> Option<f64> {
        match self.config.health {
            HealthConfig::Disabled => None,
            HealthConfig::Enabled {
                recovery_per_paid_tick,
                ..
            } => Some(recovery_per_paid_tick * self.effective_effects().recovery_amount),
        }
    }

    /// Returns the incident edge IDs, with any self-loop listed once.
    ///
    /// The order is unspecified and can change when edges are removed.
    pub fn incident_edges(&self) -> &[EdgeId] {
        &self.incident_edges
    }

    /// Checks whether advancing the age would remain finite.
    pub(crate) fn can_advance_age(&self, dt: f64) -> bool {
        (self.age + dt).is_finite()
    }

    /// Withdraws available resources and credits the efficiency-adjusted amount.
    pub(crate) fn consume(&mut self, available: &mut f64, dt: f64, capacity: f64) {
        let uptake = self.config.uptake;
        let withdrawn = uptake
            .requested_amount(*available, dt, capacity)
            .min(*available);
        *available -= withdrawn;
        self.stored_resource += uptake.efficiency * withdrawn;
    }

    /// Pays as much of the per-tick living cost as the current balance covers.
    pub(crate) fn pay_living_cost(&mut self) -> PaymentOutcome {
        let due = self.config.living_cost;
        let paid = due.min(self.stored_resource);
        self.stored_resource -= paid;

        PaymentOutcome { due, paid }
    }

    /// Applies the payment outcome using beginning-of-tick multipliers, then advances age.
    pub(crate) fn finish_tick(
        &mut self,
        payment: PaymentOutcome,
        effects: EffectMultipliers,
        dt: f64,
    ) {
        if let HealthConfig::Enabled {
            max_health,
            damage_per_unpaid_tick,
            recovery_per_paid_tick,
        } = self.config.health
            && let Some(health) = &mut self.health
        {
            if payment.paid == payment.due {
                let recovery = recovery_per_paid_tick * effects.recovery_amount;
                *health += recovery.min(max_health - *health);
            } else {
                let unpaid_fraction = (payment.due - payment.paid) / payment.due;
                *health = (*health - damage_per_unpaid_tick * unpaid_fraction).max(0.0);
            }
        }

        self.age += dt;
    }

    /// Reports whether enabled health has reached zero.
    pub(crate) fn is_expired(&self) -> bool {
        self.health.is_some_and(|health| health <= 0.0)
    }

    /// Atomically replaces effects and advances their revision.
    pub(crate) fn set_effects(&mut self, effects: EffectsConfig) -> Result<u64, VertexConfigError> {
        let next_revision = self
            .effects_revision
            .checked_add(1)
            .ok_or(VertexConfigError::EffectsRevisionExhausted)?;
        let config = self.config.clone().with_effects(effects)?;

        self.config = config;
        self.effects_revision = next_revision;
        Ok(next_revision)
    }
}

#[cfg(test)]
mod tests {
    use crate::effects::{Effect, EffectInput, EffectTarget, ResponseCurve};

    use super::*;

    fn health_vertex() -> Vertex {
        let uptake = Uptake::new(0.0, 1.0).unwrap();
        let health = HealthConfig::Enabled {
            max_health: 2.0,
            damage_per_unpaid_tick: 1.0,
            recovery_per_paid_tick: 0.25,
        };

        Vertex::new(
            (0, 0).into(),
            VertexConfig::new(uptake, 2.0, health).unwrap(),
        )
    }

    #[test]
    fn partial_payment_causes_proportional_damage_and_full_payment_restores_health() {
        let mut vertex = health_vertex();
        let effects = vertex.effective_effects();
        vertex.finish_tick(
            PaymentOutcome {
                due: 2.0,
                paid: 1.0,
            },
            effects,
            0.5,
        );
        assert_eq!(vertex.health(), Some(1.5));
        vertex.finish_tick(
            PaymentOutcome {
                due: 2.0,
                paid: 0.0,
            },
            effects,
            0.5,
        );
        assert_eq!(vertex.health(), Some(0.5));
        vertex.finish_tick(
            PaymentOutcome {
                due: 2.0,
                paid: 2.0,
            },
            effects,
            0.5,
        );
        assert_eq!(vertex.health(), Some(0.75));
        vertex.finish_tick(
            PaymentOutcome {
                due: 0.0,
                paid: 0.0,
            },
            effects,
            0.5,
        );
        assert_eq!(vertex.health(), Some(1.0));

        for _ in 0..8 {
            vertex.finish_tick(
                PaymentOutcome {
                    due: 2.0,
                    paid: 2.0,
                },
                effects,
                0.0,
            );
        }

        assert_eq!(vertex.health(), Some(2.0));
        assert_eq!(vertex.age(), 2.0);
    }

    #[test]
    fn exact_payment_can_leave_an_empty_balance_without_health_damage() {
        let uptake = Uptake::new(1.0, 1.0).unwrap();
        let config = VertexConfig::new(uptake, 1.0, HealthConfig::default()).unwrap();
        let mut vertex = Vertex::new((0, 0).into(), config);
        let effects = vertex.effective_effects();
        let mut available = 1.0;
        vertex.consume(&mut available, 1.0, effects.uptake_capacity);
        let payment = vertex.pay_living_cost();
        vertex.finish_tick(payment, effects, 1.0);

        assert_eq!(vertex.stored_resource(), 0.0);
        assert_eq!(vertex.health(), Some(10.0));
        assert!(!vertex.is_expired());
    }

    #[test]
    fn aging_changes_recovery_using_the_beginning_of_tick_state() {
        let mut vertex = health_vertex();
        let effects = EffectsConfig::new([Effect {
            input: EffectInput::Age,
            target: EffectTarget::RecoveryAmount,
            response: ResponseCurve::ExponentialDecay {
                half_life: 100.0,
                minimum: 0.1,
            },
        }])
        .unwrap();
        vertex.set_effects(effects).unwrap();
        vertex.age = 100.0;
        vertex.health = Some(1.0);
        let effects = vertex.effective_effects();
        let recovery = vertex.effective_recovery().unwrap();
        assert!((recovery - 0.1375).abs() < 1e-12);
        vertex.finish_tick(
            PaymentOutcome {
                due: 2.0,
                paid: 2.0,
            },
            effects,
            1.0,
        );

        assert_eq!(vertex.health(), Some(1.1375));
        assert_eq!(vertex.age(), 101.0);
        assert!(vertex.effective_recovery().unwrap() < recovery);
    }

    #[test]
    fn disabled_health_has_a_full_health_input_and_keeps_aging() {
        let uptake = Uptake::new(0.0, 1.0).unwrap();
        let effects = EffectsConfig::new([Effect {
            input: EffectInput::HealthFraction,
            target: EffectTarget::UptakeCapacity,
            response: ResponseCurve::Linear {
                input_start: 0.0,
                input_end: 1.0,
                output_start: 0.2,
                output_end: 1.0,
            },
        }])
        .unwrap();
        let config = VertexConfig::new(uptake, 1.0, HealthConfig::Disabled)
            .unwrap()
            .with_effects(effects)
            .unwrap();
        let mut vertex = Vertex::new((0, 0).into(), config);
        let effects = vertex.effective_effects();
        vertex.finish_tick(
            PaymentOutcome {
                due: 1.0,
                paid: 0.0,
            },
            effects,
            100.0,
        );

        assert_eq!(vertex.health(), None);
        assert_eq!(vertex.health_fraction(), 1.0);
        assert_eq!(effects.uptake_capacity, 1.0);
        assert_eq!(vertex.effective_recovery(), None);
        assert_eq!(vertex.age(), 100.0);
        assert!(!vertex.is_expired());
    }

    #[test]
    fn failed_replacements_preserve_configuration_and_revision() {
        let uptake = Uptake::new(f64::MAX, 1.0).unwrap();
        let config = VertexConfig::new(uptake, 0.0, HealthConfig::Disabled).unwrap();
        let mut vertex = Vertex::new((0, 0).into(), config.clone());
        let overflow = EffectsConfig::new([Effect {
            input: EffectInput::Age,
            target: EffectTarget::UptakeCapacity,
            response: ResponseCurve::Constant { multiplier: 2.0 },
        }])
        .unwrap();

        assert_eq!(
            vertex.set_effects(overflow),
            Err(VertexConfigError::NonFiniteUptakeCapacity)
        );
        assert_eq!(vertex.config(), &config);
        assert_eq!(vertex.effects_revision(), 0);
        vertex.effects_revision = u64::MAX;
        assert_eq!(
            vertex.set_effects(EffectsConfig::default()),
            Err(VertexConfigError::EffectsRevisionExhausted)
        );
        assert_eq!(vertex.config(), &config);
        assert_eq!(vertex.effects_revision(), u64::MAX);
    }

    #[test]
    fn zero_capacity_with_a_large_timestep_preserves_finite_resource_amounts() {
        let uptake = Uptake::new(f64::MAX, 1.0).unwrap();
        let config = VertexConfig::new(uptake, 0.0, HealthConfig::Disabled).unwrap();
        let mut vertex = Vertex::new((0, 0).into(), config);
        let mut available = 4.0;
        vertex.consume(&mut available, f64::MAX, 0.0);

        assert_eq!(available, 4.0);
        assert_eq!(vertex.stored_resource(), 0.0);
        assert!(vertex.can_advance_age(f64::MAX));
        vertex.age = f64::MAX;
        assert!(!vertex.can_advance_age(f64::MAX));
    }

    #[test]
    fn amplified_saturating_capacity_preserves_small_withdrawals() {
        let uptake = Uptake::saturating(1e-308, 1.0, 1.0).unwrap();
        let effects = EffectsConfig::new([Effect {
            input: EffectInput::Age,
            target: EffectTarget::UptakeCapacity,
            response: ResponseCurve::Constant { multiplier: 1e308 },
        }])
        .unwrap();
        let config = VertexConfig::new(uptake, 0.0, HealthConfig::Disabled)
            .unwrap()
            .with_effects(effects)
            .unwrap();
        let mut vertex = Vertex::new((0, 0).into(), config);
        let capacity = vertex.effective_effects().uptake_capacity;
        let mut available = 1e-308;
        vertex.consume(&mut available, 1.0, capacity);

        assert!(vertex.stored_resource() > 0.0);
        assert_eq!(available + vertex.stored_resource(), 1e-308);
    }
}

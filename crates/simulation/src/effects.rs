//! Configurable responses that modify vertex extraction and health recovery.
//!
//! Effect sets are validated once, shared immutably, and evaluated against the
//! same beginning-of-tick inputs. Effects targeting the same quantity multiply
//! in configuration order. An empty set returns identity multipliers.
//!
//! ```
//! use simulation::effects::{
//!     Effect, EffectContext, EffectInput, EffectTarget, EffectsConfig, ResponseCurve,
//! };
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let effects = EffectsConfig::new([Effect {
//!     input: EffectInput::Age,
//!     target: EffectTarget::RecoveryAmount,
//!     response: ResponseCurve::ExponentialDecay {
//!         half_life: 100.0,
//!         minimum: 0.1,
//!     },
//! }])?;
//! let context = EffectContext { age: 100.0, health_fraction: 0.8 };
//! let multipliers = effects.evaluate(&context);
//! assert_eq!(multipliers.recovery_amount, 0.55);
//! # Ok(())
//! # }
//! ```

use std::sync::Arc;

use thiserror::Error;

/// A property sampled before a vertex begins its tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectInput {
    /// Age measured in elapsed model time.
    Age,
    /// Current health divided by maximum health, or one when health is disabled.
    HealthFraction,
}

/// The model quantity multiplied by an effect's response.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectTarget {
    /// Requested extraction capacity before withdrawal is capped by availability.
    UptakeCapacity,
    /// Health recovered after a fully paid living cost.
    RecoveryAmount,
}

/// A response mapping an input to a nonnegative, dimensionless multiplier.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ResponseCurve {
    /// Returns the same multiplier for every input.
    Constant {
        /// Finite, nonnegative output, including values above one.
        multiplier: f64,
    },
    /// Interpolates linearly and clamps inputs outside the endpoint range.
    Linear {
        /// Finite lower endpoint of the input range.
        input_start: f64,
        /// Finite upper endpoint, strictly greater than the lower endpoint.
        input_end: f64,
        /// Finite, nonnegative output at or below the lower input endpoint.
        output_start: f64,
        /// Finite, nonnegative output at or above the upper input endpoint.
        output_end: f64,
    },
    /// Decays from one toward a minimum as a nonnegative input increases.
    ExponentialDecay {
        /// Finite, positive interval that halves the distance from the minimum.
        half_life: f64,
        /// Finite asymptotic minimum in `0..=1`.
        minimum: f64,
    },
}

impl ResponseCurve {
    fn maximum(self) -> Option<f64> {
        match self {
            Self::Constant { multiplier } => nonnegative_finite(multiplier).then_some(multiplier),
            Self::Linear {
                input_start,
                input_end,
                output_start,
                output_end,
            } => {
                let span = input_end - input_start;
                let valid = input_start.is_finite()
                    && input_end.is_finite()
                    && span.is_finite()
                    && span > 0.0
                    && nonnegative_finite(output_start)
                    && nonnegative_finite(output_end);

                valid.then_some(output_start.max(output_end))
            }
            Self::ExponentialDecay { half_life, minimum } => {
                let valid =
                    half_life.is_finite() && half_life > 0.0 && (0.0..=1.0).contains(&minimum);

                valid.then_some(1.0)
            }
        }
    }

    fn evaluate(self, input: f64) -> f64 {
        match self {
            Self::Constant { multiplier } => multiplier,
            Self::Linear {
                input_start,
                input_end,
                output_start,
                output_end,
            } => {
                let input = input.clamp(input_start, input_end);
                let fraction = (input - input_start) / (input_end - input_start);
                let output = output_start + (output_end - output_start) * fraction;

                output.clamp(output_start.min(output_end), output_start.max(output_end))
            }
            Self::ExponentialDecay { half_life, minimum } => {
                minimum + (1.0 - minimum) * (-input / half_life).exp2()
            }
        }
    }
}

fn nonnegative_finite(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}

/// One configured relationship between a vertex property and a model quantity.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Effect {
    /// Property supplying this effect's input.
    pub input: EffectInput,
    /// Quantity multiplied by this effect.
    pub target: EffectTarget,
    /// Response converting the input into a multiplier.
    pub response: ResponseCurve,
}

/// Invalid response parameters or an overflowing combination of responses.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum EffectsError {
    /// A curve contains an invalid parameter or input range.
    #[error("effect {index} has invalid response parameters")]
    InvalidCurve {
        /// Zero-based index of the invalid effect in configuration order.
        index: usize,
    },
    /// The product of response bounds overflows for a target.
    #[error("effect {index} overflows the maximum multiplier for {target:?}")]
    MultiplierOverflow {
        /// Zero-based index of the effect that makes the product non-finite.
        index: usize,
        /// Target whose multiplier cannot be represented finitely.
        target: EffectTarget,
    },
}

/// Beginning-of-tick inputs shared by every effect on a vertex.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectContext {
    /// Finite, nonnegative vertex age in model-time units.
    pub age: f64,
    /// Health fraction in `0..=1`, using one when health is disabled.
    pub health_fraction: f64,
}

/// Combined dimensionless modifiers for the current vertex context.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EffectMultipliers {
    /// Multiplier for requested extraction capacity.
    pub uptake_capacity: f64,
    /// Multiplier for health recovery after full payment.
    pub recovery_amount: f64,
}

impl Default for EffectMultipliers {
    fn default() -> Self {
        Self {
            uptake_capacity: 1.0,
            recovery_amount: 1.0,
        }
    }
}

impl EffectMultipliers {
    fn target_mut(&mut self, target: EffectTarget) -> &mut f64 {
        match target {
            EffectTarget::UptakeCapacity => &mut self.uptake_capacity,
            EffectTarget::RecoveryAmount => &mut self.recovery_amount,
        }
    }
}

/// A validated effect set whose definitions are shared across configurations.
#[derive(Debug, Clone, PartialEq)]
pub struct EffectsConfig {
    effects: Arc<[Effect]>,
    maximum: EffectMultipliers,
}

impl Default for EffectsConfig {
    fn default() -> Self {
        Self {
            effects: Arc::from([]),
            maximum: EffectMultipliers::default(),
        }
    }
}

impl EffectsConfig {
    /// Validates an ordered set and shares its immutable definitions.
    ///
    /// # Errors
    ///
    /// Returns [`EffectsError::InvalidCurve`] for invalid response parameters.
    /// Returns [`EffectsError::MultiplierOverflow`] if any prefix of the maximum
    /// multiplier product for a target becomes non-finite.
    pub fn new(effects: impl IntoIterator<Item = Effect>) -> Result<Self, EffectsError> {
        let effects: Arc<[Effect]> = effects.into_iter().collect();
        let mut maximum = EffectMultipliers::default();

        for (index, effect) in effects.iter().enumerate() {
            let bound = effect
                .response
                .maximum()
                .ok_or(EffectsError::InvalidCurve { index })?;
            let product = maximum.target_mut(effect.target);
            *product *= bound;

            if !product.is_finite() {
                return Err(EffectsError::MultiplierOverflow {
                    index,
                    target: effect.target,
                });
            }
        }

        Ok(Self { effects, maximum })
    }

    /// Borrows the effect definitions in their evaluation order.
    pub fn effects(&self) -> &[Effect] {
        &self.effects
    }

    /// Returns conservative output bounds for validating amplified base values.
    pub fn maximum_multipliers(&self) -> EffectMultipliers {
        self.maximum
    }

    /// Evaluates the set without allocating or changing model state.
    ///
    /// # Panics
    ///
    /// Panics if age is negative or non-finite, or if health is outside `0..=1`.
    pub fn evaluate(&self, context: &EffectContext) -> EffectMultipliers {
        assert!(nonnegative_finite(context.age), "invalid effect age");
        assert!(
            (0.0..=1.0).contains(&context.health_fraction),
            "invalid effect health fraction"
        );

        let mut multipliers = EffectMultipliers::default();

        for effect in &*self.effects {
            let input = match effect.input {
                EffectInput::Age => context.age,
                EffectInput::HealthFraction => context.health_fraction,
            };

            *multipliers.target_mut(effect.target) *= effect.response.evaluate(input);
        }

        multipliers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn effect(target: EffectTarget, response: ResponseCurve) -> Effect {
        Effect {
            input: EffectInput::Age,
            target,
            response,
        }
    }

    #[test]
    fn curves_clamp_interpolate_and_decay_toward_their_floor() {
        let linear = ResponseCurve::Linear {
            input_start: 10.0,
            input_end: 20.0,
            output_start: 4.0,
            output_end: 2.0,
        };

        assert_eq!(linear.evaluate(0.0), 4.0);
        assert_eq!(linear.evaluate(10.0), 4.0);
        assert_eq!(linear.evaluate(15.0), 3.0);
        assert_eq!(linear.evaluate(20.0), 2.0);
        assert_eq!(linear.evaluate(f64::MAX), 2.0);

        let decay = ResponseCurve::ExponentialDecay {
            half_life: 100.0,
            minimum: 0.1,
        };

        assert_eq!(decay.evaluate(0.0), 1.0);
        assert_eq!(decay.evaluate(100.0), 0.55);
        assert_eq!(decay.evaluate(f64::MAX), 0.1);
    }

    #[test]
    fn shared_effects_are_identity_when_empty_and_compose_by_target() {
        let context = EffectContext {
            age: 100.0,
            health_fraction: 0.5,
        };

        assert_eq!(
            EffectsConfig::default().evaluate(&context),
            EffectMultipliers::default()
        );
        let config = EffectsConfig::new([
            effect(
                EffectTarget::UptakeCapacity,
                ResponseCurve::Constant { multiplier: 2.0 },
            ),
            Effect {
                input: EffectInput::HealthFraction,
                target: EffectTarget::UptakeCapacity,
                response: ResponseCurve::Linear {
                    input_start: 0.0,
                    input_end: 1.0,
                    output_start: 0.2,
                    output_end: 1.0,
                },
            },
            effect(
                EffectTarget::RecoveryAmount,
                ResponseCurve::ExponentialDecay {
                    half_life: 100.0,
                    minimum: 0.0,
                },
            ),
        ])
        .unwrap();

        let cloned = config.clone();
        assert!(Arc::ptr_eq(&config.effects, &cloned.effects));
        let evaluated = config.evaluate(&context);
        assert!((evaluated.uptake_capacity - 1.2).abs() < 1e-12);
        assert_eq!(evaluated.recovery_amount, 0.5);
        assert_eq!(
            config.maximum_multipliers(),
            EffectMultipliers {
                uptake_capacity: 2.0,
                recovery_amount: 1.0,
            }
        );
    }

    #[test]
    fn rejects_invalid_curves_and_overflowing_prefixes() {
        let invalid = [
            ResponseCurve::Constant {
                multiplier: f64::NAN,
            },
            ResponseCurve::Constant { multiplier: -1.0 },
            ResponseCurve::Constant {
                multiplier: f64::INFINITY,
            },
            ResponseCurve::Linear {
                input_start: 1.0,
                input_end: 1.0,
                output_start: 0.0,
                output_end: 1.0,
            },
            ResponseCurve::Linear {
                input_start: -f64::MAX,
                input_end: f64::MAX,
                output_start: 0.0,
                output_end: 1.0,
            },
            ResponseCurve::Linear {
                input_start: 0.0,
                input_end: 1.0,
                output_start: -1.0,
                output_end: 1.0,
            },
            ResponseCurve::ExponentialDecay {
                half_life: 0.0,
                minimum: 0.0,
            },
            ResponseCurve::ExponentialDecay {
                half_life: 1.0,
                minimum: 1.1,
            },
            ResponseCurve::ExponentialDecay {
                half_life: 1.0,
                minimum: f64::NAN,
            },
        ];

        for response in invalid {
            assert_eq!(
                EffectsConfig::new([effect(EffectTarget::UptakeCapacity, response)]),
                Err(EffectsError::InvalidCurve { index: 0 })
            );
        }

        let overflow = EffectsConfig::new([
            effect(
                EffectTarget::UptakeCapacity,
                ResponseCurve::Constant {
                    multiplier: f64::MAX,
                },
            ),
            effect(
                EffectTarget::UptakeCapacity,
                ResponseCurve::Constant { multiplier: 2.0 },
            ),
            effect(
                EffectTarget::UptakeCapacity,
                ResponseCurve::Constant { multiplier: 0.0 },
            ),
        ]);

        assert_eq!(
            overflow,
            Err(EffectsError::MultiplierOverflow {
                index: 1,
                target: EffectTarget::UptakeCapacity
            })
        );
    }
}

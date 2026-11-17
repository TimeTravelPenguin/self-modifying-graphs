//! Display scaling based on observed resource bounds.

use bevy::{ecs::resource::Resource, reflect::Reflect};

use crate::field::ResourceField;

/// Controls how newly observed field extrema update the display bounds.
#[derive(Debug, Reflect, Default, Clone, Copy, PartialEq)]
pub enum BoundsAveraging {
    /// Uses the minimum and maximum of the most recent observation.
    #[default]
    None,

    /// Smooths the minimum and maximum independently with an exponential moving average.
    EMA {
        /// The finite, positive smoothing time constant in the same units as the elapsed time.
        tau: f64,
    },
}

impl BoundsAveraging {
    fn validate(self) {
        if let Self::EMA { tau } = self {
            assert!(
                tau.is_finite() && tau > 0.0,
                "tau must be finite and positive"
            );
        }
    }
}

/// Tracks the field bounds used to map resource amounts to a display range of `0` to `1`.
#[derive(Debug, Resource, Reflect, Clone, PartialEq)]
pub struct BoundsNormalizer {
    averaging: BoundsAveraging,
    bounds: Option<(f64, f64)>,
}

impl Default for BoundsNormalizer {
    fn default() -> Self {
        Self::new(BoundsAveraging::None)
    }
}

impl BoundsNormalizer {
    /// Creates a normalizer with the given averaging method and no observations.
    ///
    /// # Panics
    ///
    /// Panics if exponential averaging uses a non-finite or non-positive time constant.
    pub fn new(averaging: BoundsAveraging) -> Self {
        averaging.validate();

        Self {
            averaging,
            bounds: None,
        }
    }

    /// Returns the averaging method used for each new observation.
    pub fn method(&self) -> BoundsAveraging {
        self.averaging
    }

    /// Returns the effective minimum and maximum, including any averaging.
    ///
    /// Returns `None` until a nonempty field is observed after construction or reset.
    pub fn bounds(&self) -> Option<(f64, f64)> {
        self.bounds
    }

    /// Maps a value to `[0, 1]` between the observed bounds and clamps values outside them.
    ///
    /// Returns `0.5` when bounds are absent, the value or span is non-finite,
    /// or the span is at most `1e-12`.
    pub fn normalize(&self, value: f64) -> f32 {
        let Some((min, max)) = self.bounds else {
            return 0.5;
        };

        let range = max - min;

        if !value.is_finite() || !range.is_finite() || range <= 1e-12 {
            return 0.5;
        }

        ((value - min) / range).clamp(0.0, 1.0) as f32
    }

    /// Sets the averaging method and discards the previously observed bounds.
    ///
    /// # Panics
    ///
    /// Panics if exponential averaging uses a non-finite or non-positive time constant.
    pub fn set_bounds_averaging(mut self, averaging: BoundsAveraging) -> Self {
        averaging.validate();
        self.averaging = averaging;
        self.reset_bounds();

        self
    }

    /// Discards the current bounds so the next observation can initialize them.
    pub fn reset_bounds(&mut self) {
        self.bounds = None;
    }

    /// Observes the field minimum and maximum after `dt` units of model time.
    ///
    /// An empty field clears the bounds. The first nonempty observation sets
    /// both bounds directly. Later observations use the configured averaging method.
    ///
    /// # Panics
    ///
    /// Panics if `dt` is non-finite or negative, or if any field amount is non-finite.
    pub fn update_bounds(&mut self, field: &ResourceField, dt: f64) {
        assert!(
            dt.is_finite() && dt >= 0.0,
            "dt must be finite and nonnegative"
        );

        let Some((&first, rest)) = field.resources().split_first() else {
            self.reset_bounds();
            return;
        };

        assert!(first.is_finite());
        let (min, max) = rest.iter().fold((first, first), |(min, max), &value| {
            assert!(value.is_finite());
            (min.min(value), max.max(value))
        });

        self.bounds = Some(match (self.averaging, self.bounds) {
            (BoundsAveraging::EMA { tau }, Some((previous_min, previous_max))) => {
                let alpha = -(-dt / tau).exp_m1();

                (
                    smooth_bound(previous_min, min, alpha),
                    smooth_bound(previous_max, max, alpha),
                )
            }
            _ => (min, max),
        });
    }
}

fn smooth_bound(previous: f64, observed: f64, alpha: f64) -> f64 {
    let change = observed - previous;

    if change.is_finite() && alpha < 1.0 {
        return previous + alpha * change;
    }

    (1.0 - alpha) * previous + alpha * observed
}

#[cfg(test)]
mod tests {
    use std::f64::consts::LN_2;

    use super::*;

    #[test]
    fn ema_seeds_and_averages_observations() {
        let mut field = ResourceField::new_random(1, 2, 43, 1.0, 0.0);
        let mut normalizer = BoundsNormalizer::new(BoundsAveraging::EMA { tau: 1.0 });

        field.resources_mut().copy_from_slice(&[20.0, 80.0]);
        normalizer.update_bounds(&field, 0.0);
        assert_eq!(normalizer.bounds(), Some((20.0, 80.0)));

        field.resources_mut().copy_from_slice(&[40.0, 60.0]);
        normalizer.update_bounds(&field, 0.0);
        assert_eq!(normalizer.bounds(), Some((20.0, 80.0)));

        normalizer.update_bounds(&field, LN_2);
        assert_eq!(normalizer.bounds(), Some((30.0, 70.0)));
        assert_eq!(normalizer.normalize(40.0), 0.25);
        assert_eq!(normalizer.normalize(20.0), 0.0);
        assert_eq!(normalizer.normalize(80.0), 1.0);
        assert_eq!(normalizer.normalize(f64::NAN), 0.5);

        normalizer.reset_bounds();
        assert_eq!(normalizer.bounds(), None);

        normalizer.update_bounds(&field, LN_2);
        assert_eq!(normalizer.bounds(), Some((40.0, 60.0)));

        let normalizer = normalizer.set_bounds_averaging(BoundsAveraging::None);
        assert_eq!(normalizer.bounds(), None);
    }

    #[test]
    fn empty_and_nearly_uniform_fields_use_the_midpoint() {
        let mut field = ResourceField::new_random(1, 2, 43, 1.0, 0.0);
        let empty = ResourceField::new_random(1, 0, 43, 1.0, 0.0);
        let mut normalizer = BoundsNormalizer::default();

        assert_eq!(normalizer.normalize(2.0), 0.5);

        field.resources_mut().copy_from_slice(&[2.0, 2.0]);
        normalizer.update_bounds(&field, 0.1);
        assert_eq!(normalizer.normalize(2.0), 0.5);

        field.resources_mut().copy_from_slice(&[2.0, 2.0 + 1e-13]);
        normalizer.update_bounds(&field, 0.1);
        assert_eq!(normalizer.normalize(2.0), 0.5);

        normalizer.update_bounds(&empty, 0.1);
        assert_eq!(normalizer.bounds(), None);
        assert_eq!(normalizer.normalize(2.0), 0.5);
    }

    #[test]
    fn ema_handles_finite_observations_with_an_overflowing_difference() {
        let mut field = ResourceField::new_random(1, 1, 43, 1.0, 0.0);
        let mut normalizer = BoundsNormalizer::new(BoundsAveraging::EMA { tau: 1.0 });

        field.resources_mut()[0] = f64::MAX;
        normalizer.update_bounds(&field, 1.0);

        field.resources_mut()[0] = -f64::MAX;
        normalizer.update_bounds(&field, LN_2);
        assert_eq!(normalizer.bounds(), Some((0.0, 0.0)));
    }

    #[test]
    #[should_panic(expected = "tau must be finite and positive")]
    fn rejects_invalid_ema_on_construction() {
        BoundsNormalizer::new(BoundsAveraging::EMA { tau: f64::NAN });
    }

    #[test]
    #[should_panic(expected = "dt must be finite and nonnegative")]
    fn rejects_negative_dt() {
        let field = ResourceField::new_random(1, 1, 43, 1.0, 0.0);
        let mut normalizer = BoundsNormalizer::default();

        normalizer.update_bounds(&field, -1.0);
    }
}

use crate::field::ResourceField;

#[derive(Debug, Default, Clone, Copy, PartialEq)]
pub enum BoundsAveraging {
    /// No averaging.
    #[default]
    None,

    /// Exponential moving average.
    ///
    /// `tau` is the smoothing time constant,
    /// measured in simulation time units.
    EMA { tau: f64 },
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

#[derive(Debug)]
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
    pub fn new(averaging: BoundsAveraging) -> Self {
        averaging.validate();

        Self {
            averaging,
            bounds: None,
        }
    }

    pub fn method(&self) -> BoundsAveraging {
        self.averaging
    }

    /// Returns the current minimum and maximum, if a nonempty field has been observed since reset.
    pub fn bounds(&self) -> Option<(f64, f64)> {
        self.bounds
    }

    /// Maps a value to `[0, 1]`, using the midpoint when it cannot be normalized.
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

    pub fn set_bounds_averaging(mut self, averaging: BoundsAveraging) -> Self {
        averaging.validate();
        self.averaging = averaging;
        self.reset_bounds();

        self
    }

    pub fn reset_bounds(&mut self) {
        self.bounds = None;
    }

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

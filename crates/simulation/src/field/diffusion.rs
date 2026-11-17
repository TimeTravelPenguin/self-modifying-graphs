use bevy::{ecs::resource::Resource, reflect::Reflect};
use rayon::{
    iter::{IndexedParallelIterator, ParallelIterator},
    slice::ParallelSliceMut,
};
use thiserror::Error;

use super::ResourceField;

#[derive(Debug, Error)]
pub enum DiffusionError {
    #[error("invalid diffusion coefficient {0}")]
    InvalidCoefficient(f64),

    #[error("invalid diffusion timestep {0}")]
    InvalidTimestep(f64),

    #[error("invalid diffusion spacing {0}")]
    InvalidSpacing(f64),

    #[error("diffusion number {diffusion_number} must be finite and at most {limit}")]
    UnstableTimestep { diffusion_number: f64, limit: f64 },
}

#[derive(Debug, Reflect, Default, Clone, Copy, PartialEq, Eq)]
pub enum Neighbourhood {
    /// 5-point Laplacian: 4 cardinal neighbours.
    #[default]
    VonNeumann,

    /// 9-point Laplacian: 4 cardinal + 4 diagonal neighbours.
    Moore,
}

impl Neighbourhood {
    pub(crate) fn diffusion_number(
        self,
        diffusion_coefficient: f64,
        dt: f64,
        spacing: f64,
    ) -> Result<f64, DiffusionError> {
        if !diffusion_coefficient.is_finite() || diffusion_coefficient < 0.0 {
            return Err(DiffusionError::InvalidCoefficient(diffusion_coefficient));
        }

        if !dt.is_finite() || dt < 0.0 {
            return Err(DiffusionError::InvalidTimestep(dt));
        }

        if !spacing.is_finite() || spacing <= 0.0 {
            return Err(DiffusionError::InvalidSpacing(spacing));
        }

        let diffusion_number = diffusion_coefficient * dt / (spacing * spacing);
        let limit = match self {
            Self::VonNeumann => 0.25,
            Self::Moore => 0.30,
        };

        if !diffusion_number.is_finite() || diffusion_number > limit {
            return Err(DiffusionError::UnstableTimestep {
                diffusion_number,
                limit,
            });
        }

        Ok(diffusion_number)
    }
}

#[derive(Debug, Resource, Reflect, Default, Clone, PartialEq)]
pub struct Diffuser {
    neighbourhood: Neighbourhood,
    buffer: Vec<f64>,
}

impl Diffuser {
    pub fn new(neighbourhood: Neighbourhood) -> Self {
        Self {
            neighbourhood,
            buffer: Vec::new(),
        }
    }

    pub fn neighbourhood(&self) -> Neighbourhood {
        self.neighbourhood
    }

    pub fn set_neighbourhood(&mut self, neighbourhood: Neighbourhood) {
        self.neighbourhood = neighbourhood;
    }

    pub fn diffuse(
        &mut self,
        field: &mut ResourceField,
        diffusion_coefficient: f64,
        dt: f64,
        spacing: f64,
    ) {
        let lambda = self
            .neighbourhood
            .diffusion_number(diffusion_coefficient, dt, spacing)
            .unwrap_or_else(|error| panic!("{error}"));

        let width = field.width();
        let height = field.height();

        assert_eq!(field.resources().len(), width * height);

        if lambda == 0.0 || width == 0 || height == 0 {
            return;
        }

        self.buffer.resize(field.resources().len(), 0.0);

        match self.neighbourhood {
            Neighbourhood::VonNeumann => {
                apply::<false>(field.resources(), &mut self.buffer, width, height, lambda);
            }
            Neighbourhood::Moore => {
                apply::<true>(
                    field.resources(),
                    &mut self.buffer,
                    width,
                    height,
                    lambda / 6.0,
                );
            }
        }

        field.swap_resources(&mut self.buffer);
    }
}

/// Apply a Laplacian to the grid.
///
/// DIAGONALS = false: 5-point stencil.
/// DIAGONALS = true:  9-point stencil.
///
/// `scale` includes D * dt / h² and, for the nine-point
/// stencil, the additional factor 1/6.
fn apply<const DIAGONALS: bool>(
    src: &[f64],
    dst: &mut [f64],
    width: usize,
    height: usize,
    scale: f64,
) {
    assert_eq!(src.len(), width * height);
    assert_eq!(dst.len(), src.len());

    if width == 0 || height == 0 {
        return;
    }

    let process_row = |y: usize, dst_row: &mut [f64]| {
        // Handle boundaries and small grids.
        if width < 3 || height < 3 || y == 0 || y == height - 1 {
            for (x, value) in dst_row.iter_mut().enumerate() {
                *value = boundary::<DIAGONALS>(src, width, height, x, y, scale);
            }

            return;
        }

        let offset = y * width;

        // Three adjacent source rows.
        let above = &src[offset - width..offset];
        let current_row = &src[offset..offset + width];
        let below = &src[offset + width..offset + 2 * width];

        // Left boundary.
        dst_row[0] = boundary::<DIAGONALS>(src, width, height, 0, y, scale);

        // Interior cells.
        for x in 1..width - 1 {
            let current = current_row[x];

            let cardinal =
                above[x] + below[x] + current_row[x - 1] + current_row[x + 1] - 4.0 * current;

            let laplacian = if DIAGONALS {
                let diagonal =
                    above[x - 1] + above[x + 1] + below[x - 1] + below[x + 1] - 4.0 * current;

                4.0 * cardinal + diagonal
            } else {
                cardinal
            };

            dst_row[x] = current + scale * laplacian;
        }

        // Right boundary.
        dst_row[width - 1] = boundary::<DIAGONALS>(src, width, height, width - 1, y, scale);
    };

    // Avoid parallel overhead for small grids.
    const PARALLEL_THRESHOLD: usize = 65_536;

    if src.len() < PARALLEL_THRESHOLD || height < 4 {
        for (y, row) in dst.chunks_mut(width).enumerate() {
            process_row(y, row);
        }
    } else {
        dst.par_chunks_mut(width)
            .enumerate()
            .for_each(|(y, row)| process_row(y, row));
    }
}

/// Boundary stencil using only valid neighbours.
///
/// Missing neighbours contribute zero flux.
#[inline]
fn boundary<const DIAGONALS: bool>(
    src: &[f64],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    scale: f64,
) -> f64 {
    let i = y * width + x;
    let current = src[i];

    let mut cardinal = 0.0;

    if x > 0 {
        cardinal += src[i - 1] - current;
    }

    if x + 1 < width {
        cardinal += src[i + 1] - current;
    }

    if y > 0 {
        cardinal += src[i - width] - current;
    }

    if y + 1 < height {
        cardinal += src[i + width] - current;
    }

    let laplacian = if DIAGONALS {
        let mut diagonal = 0.0;

        if x > 0 && y > 0 {
            diagonal += src[i - width - 1] - current;
        }

        if x + 1 < width && y > 0 {
            diagonal += src[i - width + 1] - current;
        }

        if x > 0 && y + 1 < height {
            diagonal += src[i + width - 1] - current;
        }

        if x + 1 < width && y + 1 < height {
            diagonal += src[i + width + 1] - current;
        }

        4.0 * cardinal + diagonal
    } else {
        cardinal
    };

    current + scale * laplacian
}

#[cfg(test)]
mod tests {
    use super::{Diffuser, Neighbourhood, ResourceField};

    const LAMBDA: f64 = 3.0 / 16.0;

    #[test]
    fn stencils_spread_an_interior_impulse_on_a_rectangular_field() {
        let cases = [
            (
                Neighbourhood::VonNeumann,
                [
                    [0.0, LAMBDA, 0.0, 0.0],
                    [LAMBDA, 0.25, LAMBDA, 0.0],
                    [0.0, LAMBDA, 0.0, 0.0],
                ],
            ),
            (
                Neighbourhood::Moore,
                [
                    [1.0 / 32.0, 0.125, 1.0 / 32.0, 0.0],
                    [0.125, 0.375, 0.125, 0.0],
                    [1.0 / 32.0, 0.125, 1.0 / 32.0, 0.0],
                ],
            ),
        ];

        for (neighbourhood, expected) in cases {
            let mut field = ResourceField::new_random(3, 4, 0, 1.0, 0.0);
            let impulse_index = field.width() + 1;
            field.resources_mut()[impulse_index] = 1.0;

            Diffuser::new(neighbourhood).diffuse(&mut field, LAMBDA, 1.0, 1.0);

            assert_eq!(
                field.resources(),
                expected.as_flattened(),
                "{neighbourhood:?}"
            );
        }
    }

    #[test]
    fn no_flux_boundaries_conserve_mass_on_thin_and_parallel_grids() {
        for neighbourhood in [Neighbourhood::VonNeumann, Neighbourhood::Moore] {
            for (height, width) in [(3, 1), (256, 256)] {
                let mut field = ResourceField::new_random(height, width, 0, 1.0, 0.0);
                field.resources_mut()[0] = 1.0;

                Diffuser::new(neighbourhood).diffuse(&mut field, LAMBDA, 1.0, 1.0);

                assert_eq!(
                    field.resources().iter().sum::<f64>(),
                    1.0,
                    "{neighbourhood:?} on a {width}×{height} field",
                );
            }
        }
    }
}

//! Resource amounts on a rectangular lattice and the solvers that diffuse them.

use bevy::{ecs::resource::Resource, reflect::Reflect};
use noise::{NoiseFn, Perlin};

use crate::Position;

mod diffusion;

pub use diffusion::{Diffuser, DiffusionError, Neighbourhood};

/// A rectangular grid of resource amounts, one per cell.
///
/// Cells are stored in row-major order. The cell at `(x, y)` has index
/// `y * width + x`.
#[derive(Debug, Resource, Reflect, Clone, PartialEq)]
pub struct ResourceField {
    height: usize,
    width: usize,
    resources: Vec<f64>,
}

impl ResourceField {
    /// Creates a field with the given height and width and fills it with seeded Perlin noise.
    ///
    /// `scale` is the number of grid cells per noise coordinate unit. The
    /// noise samples are mapped to resource amounts between zero and `max_value`.
    ///
    /// # Panics
    ///
    /// Panics if the dimensions or area exceed `usize`, if `scale` is
    /// non-finite or non-positive, or if `max_value` is non-finite or negative.
    pub fn new_random(height: u32, width: u32, seed: u32, scale: f64, max_value: f64) -> Self {
        let height = usize::try_from(height).expect("resource field height must fit in usize");
        let width = usize::try_from(width).expect("resource field width must fit in usize");
        let area = height
            .checked_mul(width)
            .expect("resource field area must fit in usize");

        let mut field = Self {
            height,
            width,
            resources: vec![0.0; area],
        };

        field.randomize(seed, scale, max_value);

        field
    }

    /// Returns the number of rows.
    pub fn height(&self) -> usize {
        self.height
    }

    /// Returns the number of columns.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Replaces each cell amount with a sample from the seeded Perlin field.
    ///
    /// The sampling scale and upper resource bound follow [`Self::new_random`].
    ///
    /// # Panics
    ///
    /// Panics if `scale` is non-finite or non-positive, or if `max_resource`
    /// is non-finite or negative.
    pub fn randomize(&mut self, seed: u32, scale: f64, max_resource: f64) {
        assert!(scale.is_finite() && scale > 0.0);
        assert!(max_resource.is_finite() && max_resource >= 0.0);

        let perlin = Perlin::new(seed);

        for y in 0..self.height {
            for x in 0..self.width {
                let value = perlin.get([x as f64 / scale, y as f64 / scale]);

                self.resources[y * self.width + x] =
                    ((value + 1.0) * 0.5).clamp(0.0, 1.0) * max_resource;
            }
        }
    }

    /// Borrows the complete row-major array of cell amounts.
    pub fn resources(&self) -> &Vec<f64> {
        &self.resources
    }

    /// Borrows the cell amounts for editing while retaining the grid dimensions.
    pub fn resources_mut(&mut self) -> &mut [f64] {
        &mut self.resources
    }

    fn swap_resources(&mut self, buffer: &mut Vec<f64>) {
        assert_eq!(buffer.len(), self.resources.len());
        std::mem::swap(&mut self.resources, buffer);
    }

    /// Returns the amount at a cell, or `None` if either coordinate is outside the grid.
    pub fn get_resource_at(&self, position: impl Into<Position>) -> Option<f64> {
        self.cell_index(position.into())
            .map(|index| self.resources[index])
    }

    /// Mutably borrows an in-bounds cell amount.
    pub(crate) fn get_resource_at_mut(&mut self, position: Position) -> Option<&mut f64> {
        let index = self.cell_index(position)?;
        Some(&mut self.resources[index])
    }

    /// Returns the row-major index after checking both cell coordinates.
    pub(crate) fn cell_index(&self, position: Position) -> Option<usize> {
        if position.x >= self.width || position.y >= self.height {
            return None;
        }

        Some(position.y * self.width + position.x)
    }
}

#[cfg(test)]
mod tests {
    use super::ResourceField;

    #[test]
    fn resource_lookup_checks_both_coordinates() {
        let mut field = ResourceField::new_random(2, 2, 0, 1.0, 1.0);
        field.resources_mut().copy_from_slice(&[0.0, 1.0, 2.0, 3.0]);

        assert_eq!(field.get_resource_at((0, 1)), Some(2.0));
        assert_eq!(field.get_resource_at((2, 0)), None);
        assert_eq!(field.get_resource_at((0, 2)), None);
        assert_eq!(field.get_resource_at((usize::MAX, 1)), None);
    }
}

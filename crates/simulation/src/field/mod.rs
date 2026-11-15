use noise::{NoiseFn, Perlin};

use crate::Position;

mod diffusion;

pub use diffusion::{Diffuser, Neighbourhood};

#[derive(Debug, Clone, PartialEq)]
pub struct ResourceField {
    height: usize,
    width: usize,
    resources: Vec<f64>,
}

impl ResourceField {
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

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn width(&self) -> usize {
        self.width
    }

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

    pub fn resources(&self) -> &Vec<f64> {
        &self.resources
    }

    pub fn resources_mut(&mut self) -> &mut [f64] {
        &mut self.resources
    }

    fn swap_resources(&mut self, buffer: &mut Vec<f64>) {
        assert_eq!(buffer.len(), self.resources.len());
        std::mem::swap(&mut self.resources, buffer);
    }

    pub fn get_resource_at(&self, position: impl Into<Position>) -> Option<f64> {
        self.cell_index(position.into())
            .map(|index| self.resources[index])
    }

    pub(crate) fn get_resource_at_mut(&mut self, position: Position) -> Option<&mut f64> {
        let index = self.cell_index(position)?;
        Some(&mut self.resources[index])
    }

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

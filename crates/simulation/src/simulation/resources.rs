use noise::{NoiseFn, Perlin};

use crate::Position;

#[derive(Debug, Clone, PartialEq)]
pub struct ResourceField {
    height: usize,
    width: usize,
    resources: Vec<f64>,
}

impl ResourceField {
    pub fn new_random(height: u32, width: u32, seed: u32, scale: f64, max_value: f64) -> Self {
        let resources = vec![0.0; (height * width) as usize];

        let mut field = Self {
            height: height as usize,
            width: width as usize,
            resources,
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

    pub fn get_resource_at(&self, position: Position) -> Option<f64> {
        let index = position.y * self.width + position.x;
        self.resources.get(index).copied()
    }
}

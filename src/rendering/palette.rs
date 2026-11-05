use bevy::{color::Mix, prelude::*};

/// Semantic colours shared by the grid and graph visuals.
#[derive(Resource, Clone, Debug)]
pub struct RenderPalette {
    pub background: Color,
    pub tile_neutral: Color,
    pub resource_high: Color,
    pub vertex: Color,
    pub edge: Color,
    pub selection: Color,
}

impl RenderPalette {
    pub fn from_flavor(flavor: &catppuccin::Flavor) -> Self {
        let colors = &flavor.colors;

        Self {
            background: colors.base.into(),
            tile_neutral: colors.surface0.into(),
            resource_high: colors.green.into(),
            vertex: colors.blue.into(),
            edge: colors.overlay2.into(),
            selection: colors.mauve.into(),
        }
    }

    /// Maps a normalised environmental resource amount to a tile fill.
    pub fn resource_fill(&self, amount: f32) -> Color {
        let amount = if amount.is_finite() {
            amount.clamp(0.0, 1.0)
        } else {
            0.0
        };

        self.tile_neutral
            .to_linear()
            .mix(&self.resource_high.to_linear(), amount)
            .into()
    }
}

impl Default for RenderPalette {
    fn default() -> Self {
        Self::from_flavor(&catppuccin::PALETTE.mocha)
    }
}

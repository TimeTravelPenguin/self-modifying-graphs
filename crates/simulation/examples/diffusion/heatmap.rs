//! Updates the shared heatmap from the example's Bevy resources.

use bevy::prelude::*;
use simulation::{
    field::ResourceField,
    normalization::BoundsNormalizer,
    rendering::{Tile, redraw_heatmap},
};

pub(super) use simulation::rendering::{Heatmap, spawn_heatmap};

#[cfg(test)]
pub(super) use simulation::rendering::heatmap_color;

pub(super) fn update_heatmap(
    heatmap: Res<Heatmap>,
    resources: Res<ResourceField>,
    bounds: Res<BoundsNormalizer>,
    mut tiles: Query<(&mut Sprite, Option<&Children>), With<Tile>>,
    mut texts: Query<(&mut Text2d, &mut TextColor)>,
) {
    if !resources.is_changed() && !bounds.is_changed() {
        return;
    }

    redraw_heatmap(&heatmap, &resources, &bounds, &mut tiles, &mut texts);
}

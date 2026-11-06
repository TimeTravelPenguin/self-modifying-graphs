//! Temporary rendering examples, independent of the future simulation.

use bevy::prelude::*;

use crate::{
    grid::{GridCoord, GridLayout},
    rendering::{GridTiles, RenderPalette, TileFill, edge, vertex},
};

pub fn setup(
    mut commands: Commands,
    layout: Res<GridLayout>,
    tiles: Res<GridTiles>,
    palette: Res<RenderPalette>,
) {
    let center = GridCoord::new(layout.columns() / 2, layout.rows() / 2);
    let neighbour = GridCoord::new(
        (center.column + 1).min(layout.columns() - 1),
        (center.row + 1).min(layout.rows() - 1),
    );
    let source = commands.spawn(vertex(center.column, center.row)).id();

    if neighbour != center {
        let target = commands.spawn(vertex(neighbour.column, neighbour.row)).id();
        commands.spawn(edge(source, target));
    }

    // // A first fill demonstrates a simple override of the default neutral tile fill.
    // if let Some(tile) = tiles.get(center) {
    //     commands.entity(tile).insert(TileFill::Resource(0.35));
    // }
    //
    // // A second fill demonstrates an independent override using the palette.
    // let corner = GridCoord::new(0, 0);
    //
    // if corner != center
    //     && let Some(tile) = tiles.get(corner)
    // {
    //     commands
    //         .entity(tile)
    //         .insert(TileFill::Custom(palette.selection));
    // }
}

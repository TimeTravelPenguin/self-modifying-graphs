//! Presentation of the lattice and its graph. Simulation state belongs elsewhere.

mod graph;
mod palette;
mod shapes;

#[cfg(test)]
mod tests;

use std::collections::HashMap;

use bevy::{camera::ScalingMode, prelude::*};

use crate::grid::{GridCoord, GridLayout};

pub use graph::{EdgeVisual, VertexVisual, edge, vertex};
pub use palette::RenderPalette;

const TILE_Z: f32 = 0.0;
const EDGE_Z: f32 = 1.0;
const VERTEX_Z: f32 = 2.0;
const CAMERA_PADDING: f32 = 32.0;

pub struct RenderingPlugin;

impl Plugin for RenderingPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<GridLayout>()
            .init_resource::<RenderPalette>()
            .init_resource::<GridTiles>()
            .init_resource::<RenderAssets>();

        graph::register_observers(app);

        app.add_systems(Startup, setup).add_systems(
            Update,
            (
                sync_grid_layout,
                sync_tile_fills,
                graph::sync_vertices,
                graph::sync_edges,
            )
                .chain(),
        );
    }
}

#[derive(Component)]
pub struct BackgroundGridTile;

/// A presentation request; environmental amounts can be supplied by a simulation.
#[derive(Component, Clone, Copy, Debug, Default)]
pub enum TileFill {
    #[default]
    Neutral,
    Resource(f32),
    Custom(Color),
}

impl TileFill {
    fn color(self, palette: &RenderPalette) -> Color {
        match self {
            Self::Neutral => palette.tile_neutral,
            Self::Resource(amount) => palette.resource_fill(amount),
            Self::Custom(color) => color,
        }
    }
}

/// Render entities are indexed separately from the simulation's occupancy.
#[derive(Resource, Default)]
pub struct GridTiles {
    entities: HashMap<GridCoord, Entity>,
}

impl GridTiles {
    pub fn get(&self, coord: GridCoord) -> Option<Entity> {
        self.entities.get(&coord).copied()
    }
}

#[derive(Component)]
struct GridCamera;

#[derive(Resource)]
pub(super) struct RenderAssets {
    pub tile: Handle<Image>,
    pub vertex: Handle<Image>,
}

impl FromWorld for RenderAssets {
    fn from_world(world: &mut World) -> Self {
        let layout = *world.resource::<GridLayout>();
        let mut images = world.resource_mut::<Assets<Image>>();

        Self {
            tile: images.add(shapes::rounded_square_image(
                layout.corner_radius() / layout.tile_side(),
            )),
            vertex: images.add(shapes::circle_image()),
        }
    }
}

pub(crate) fn setup(
    mut commands: Commands,
    layout: Res<GridLayout>,
    palette: Res<RenderPalette>,
    assets: Res<RenderAssets>,
    mut tiles: ResMut<GridTiles>,
) {
    commands.insert_resource(ClearColor(palette.background));
    commands.spawn((Camera2d, GridCamera, grid_projection(&layout)));

    for row in 0..layout.rows() {
        for column in 0..layout.columns() {
            let coord = GridCoord::new(column, row);
            let entity = spawn_tile(&mut commands, &layout, &palette, &assets, coord);
            tiles.entities.insert(coord, entity);
        }
    }
}

fn grid_projection(layout: &GridLayout) -> Projection {
    let padded_size = layout.bounds_size() + Vec2::splat(CAMERA_PADDING * 2.0);

    Projection::Orthographic(OrthographicProjection {
        scaling_mode: ScalingMode::AutoMin {
            min_width: padded_size.x,
            min_height: padded_size.y,
        },
        ..OrthographicProjection::default_2d()
    })
}

fn spawn_tile(
    commands: &mut Commands,
    layout: &GridLayout,
    palette: &RenderPalette,
    assets: &RenderAssets,
    coord: GridCoord,
) -> Entity {
    commands
        .spawn((
            BackgroundGridTile,
            coord,
            TileFill::Neutral,
            Sprite {
                image: assets.tile.clone(),
                color: palette.tile_neutral,
                custom_size: Some(Vec2::splat(layout.tile_side())),
                ..default()
            },
            Transform::from_translation(layout.world_position(coord).extend(TILE_Z)),
        ))
        .id()
}

fn sync_grid_layout(
    mut commands: Commands,
    layout: Res<GridLayout>,
    palette: Res<RenderPalette>,
    assets: Res<RenderAssets>,
    mut images: ResMut<Assets<Image>>,
    mut tiles: ResMut<GridTiles>,
    mut cameras: Query<&mut Projection, With<GridCamera>>,
    mut tile_visuals: Query<
        (Entity, &GridCoord, &mut Transform, &mut Sprite),
        With<BackgroundGridTile>,
    >,
) {
    if !layout.is_changed() {
        return;
    }

    *images
        .get_mut(&assets.tile)
        .expect("shared tile image exists") =
        shapes::rounded_square_image(layout.corner_radius() / layout.tile_side());

    for mut projection in &mut cameras {
        *projection = grid_projection(&layout);
    }

    for (entity, &coord, mut transform, mut sprite) in &mut tile_visuals {
        if layout.index(coord).is_none() {
            commands.entity(entity).despawn();
            tiles.entities.remove(&coord);
            continue;
        }

        transform.translation = layout.world_position(coord).extend(TILE_Z);
        sprite.custom_size = Some(Vec2::splat(layout.tile_side()));
    }

    for row in 0..layout.rows() {
        for column in 0..layout.columns() {
            let coord = GridCoord::new(column, row);
            tiles
                .entities
                .entry(coord)
                .or_insert_with(|| spawn_tile(&mut commands, &layout, &palette, &assets, coord));
        }
    }
}

fn sync_tile_fills(
    palette: Res<RenderPalette>,
    mut clear_color: ResMut<ClearColor>,
    mut tiles: Query<(Ref<TileFill>, &mut Sprite), With<BackgroundGridTile>>,
) {
    if palette.is_changed() {
        clear_color.0 = palette.background;
    }

    for (fill, mut sprite) in &mut tiles {
        if fill.is_changed() || palette.is_changed() {
            sprite.color = fill.color(&palette);
        }
    }
}

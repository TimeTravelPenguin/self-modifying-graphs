//! Bevy presentation of completed simulation snapshots.

use std::{
    collections::HashMap,
    f32::consts::{FRAC_1_SQRT_2, FRAC_PI_4},
    sync::Arc,
};

use crate::{
    Position,
    graph::VertexId,
    normalization::{BoundsAveraging, BoundsNormalizer},
    rendering::{Heatmap, TILE_SIZE, Tile, UI_COLORS, cell_center, redraw_heatmap, spawn_heatmap},
    simulation::{SimulationSnapshot, SnapshotReader},
};
use bevy::{prelude::*, sprite::SpriteAlphaMode, window::PrimaryWindow};

mod layout;
mod ui;

use layout::{FieldView, ViewerLayout};

const MARKER_SIZE: f32 = TILE_SIZE * 1.9;

/// Draws completed snapshots from the app's [`SnapshotReader`] resource.
pub struct ViewerPlugin {
    bounds_normalizer: BoundsNormalizer,
}

impl ViewerPlugin {
    /// Configures the heatmap and legend with the given bounds normalizer.
    ///
    /// The default normalizer holds the scale fixed until the field is reseeded.
    /// Exponential averaging also samples each newly displayed snapshot.
    pub fn new(bounds_normalizer: BoundsNormalizer) -> Self {
        Self { bounds_normalizer }
    }
}

impl Plugin for ViewerPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClearColor(UI_COLORS.base.into()))
            .insert_resource(SnapshotView {
                displayed: None,
                selected: None,
                bounds: self.bounds_normalizer.clone(),
            })
            .init_resource::<Viewport>()
            .add_systems(Startup, setup)
            .add_systems(
                Update,
                (
                    refresh_snapshot,
                    update_layout,
                    select_vertex,
                    update_display,
                )
                    .chain(),
            );
    }
}

#[derive(Resource)]
struct SnapshotView {
    displayed: Option<Arc<SimulationSnapshot>>,
    selected: Option<VertexId>,
    bounds: BoundsNormalizer,
}

#[derive(Resource, Default)]
struct Viewport {
    window_size: Option<Vec2>,
    field: Option<FieldView>,
}

#[derive(Component)]
struct FieldCamera;

#[derive(Component)]
struct VertexMarker(VertexId);

fn setup(mut commands: Commands, reader: Res<SnapshotReader>, mut view: ResMut<SnapshotView>) {
    let snapshot = reader.latest();
    view.bounds.reset_bounds();
    view.bounds.update_bounds(&snapshot.field, 0.0);

    commands.spawn((Camera2d, FieldCamera));
    spawn_heatmap(&mut commands, &snapshot.field, &view.bounds);

    for vertex in &snapshot.vertices {
        let center = cell_center(&snapshot.field, vertex.position);
        spawn_marker(&mut commands, vertex.id, center);
    }

    ui::spawn(&mut commands, &snapshot, view.bounds.bounds().unwrap());
    view.displayed = Some(snapshot);
}

fn refresh_snapshot(
    mut commands: Commands,
    reader: Res<SnapshotReader>,
    mut view: ResMut<SnapshotView>,
    heatmap: Res<Heatmap>,
    mut tiles: Query<(&mut Sprite, Option<&Children>), With<Tile>>,
    mut labels: Query<(&mut Text2d, &mut TextColor)>,
    mut markers: Query<(Entity, &VertexMarker, &mut Transform)>,
    mut positions: Local<HashMap<VertexId, Position>>,
) {
    let snapshot = reader.latest();
    let previous = view
        .displayed
        .as_ref()
        .expect("viewer setup loaded a snapshot");

    if Arc::ptr_eq(previous, &snapshot) {
        return;
    }

    let reseeded = previous.field_seed != snapshot.field_seed;
    let elapsed = snapshot.simulation_time - previous.simulation_time;

    if reseeded {
        view.bounds.reset_bounds();
    }

    if reseeded || matches!(view.bounds.method(), BoundsAveraging::EMA { .. }) {
        view.bounds.update_bounds(&snapshot.field, elapsed);
    }

    redraw_heatmap(
        &heatmap,
        &snapshot.field,
        &view.bounds,
        &mut tiles,
        &mut labels,
    );

    positions.extend(
        snapshot
            .vertices
            .iter()
            .map(|vertex| (vertex.id, vertex.position)),
    );

    if view.selected.is_some_and(|id| !positions.contains_key(&id)) {
        view.selected = None;
    }

    for (entity, marker, mut transform) in &mut markers {
        if let Some(position) = positions.remove(&marker.0) {
            let translation =
                cell_center(&snapshot.field, position).extend(transform.translation.z);

            if transform.translation != translation {
                transform.translation = translation;
            }
        } else {
            commands.entity(entity).despawn();
        }
    }

    for (id, position) in positions.drain() {
        let center = cell_center(&snapshot.field, position);
        spawn_marker(&mut commands, id, center);
    }

    view.displayed = Some(snapshot);
}

fn spawn_marker(commands: &mut Commands, id: VertexId, center: Vec2) {
    commands
        .spawn((
            VertexMarker(id),
            Sprite {
                alpha_mode: SpriteAlphaMode::Opaque,
                ..Sprite::from_color(UI_COLORS.crust, Vec2::splat(MARKER_SIZE))
            },
            Transform::from_translation(center.extend(1.0))
                .with_rotation(Quat::from_rotation_z(FRAC_PI_4)),
        ))
        .with_child((
            Sprite {
                alpha_mode: SpriteAlphaMode::Opaque,
                ..Sprite::from_color(UI_COLORS.rosewater, Vec2::splat(TILE_SIZE * 1.25))
            },
            Transform::from_xyz(0.0, 0.0, 0.1),
        ));
}

fn update_layout(
    window: Query<&Window, With<PrimaryWindow>>,
    view: Res<SnapshotView>,
    mut viewport: ResMut<Viewport>,
    camera: Single<(&mut Projection, &mut Transform), With<FieldCamera>>,
    mut regions: Query<(&ui::UiRegion, &mut Node)>,
) {
    let Ok(window) = window.single() else {
        return;
    };

    let size = window.size();

    if viewport.window_size == Some(size) {
        return;
    }

    let snapshot = view
        .displayed
        .as_ref()
        .expect("viewer setup loaded a snapshot");
    let layout = ViewerLayout::new(size);
    let grid_size = Vec2::new(
        snapshot.field.width() as f32,
        snapshot.field.height() as f32,
    ) * TILE_SIZE;
    viewport.window_size = Some(size);
    viewport.field = layout.fit_field(grid_size);
    ui::apply_layout(&layout, &mut regions);

    let (mut projection, mut transform) = camera.into_inner();

    if let Some(field) = &viewport.field
        && let Projection::Orthographic(projection) = &mut *projection
    {
        projection.scale = field.scale;
        transform.translation.x = field.camera_offset.x;
        transform.translation.y = field.camera_offset.y;
    }
}

fn select_vertex(
    window: Query<&Window, With<PrimaryWindow>>,
    buttons: Option<Res<ButtonInput<MouseButton>>>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    viewport: Res<Viewport>,
    mut view: ResMut<SnapshotView>,
) {
    let selection = if keys.is_some_and(|keys| keys.just_pressed(KeyCode::Escape)) {
        None
    } else if buttons.is_some_and(|buttons| buttons.just_pressed(MouseButton::Left)) {
        let Some(world_position) = window
            .single()
            .ok()
            .and_then(Window::cursor_position)
            .and_then(|cursor| viewport.field.as_ref()?.screen_to_world(cursor))
        else {
            return;
        };

        let snapshot = view
            .displayed
            .as_ref()
            .expect("viewer setup loaded a snapshot");
        marker_at(snapshot, world_position)
    } else {
        return;
    };

    if view.selected != selection {
        view.selected = selection;
    }
}

fn marker_at(snapshot: &SimulationSnapshot, world_position: Vec2) -> Option<VertexId> {
    let diamond_radius = MARKER_SIZE * FRAC_1_SQRT_2;

    snapshot
        .vertices
        .iter()
        .filter_map(|vertex| {
            let offset = cell_center(&snapshot.field, vertex.position) - world_position;
            (offset.x.abs() + offset.y.abs() <= diamond_radius)
                .then(|| (offset.length_squared(), vertex.id))
        })
        .min_by(|left, right| {
            left.0
                .total_cmp(&right.0)
                .then_with(|| left.1.cmp(&right.1))
        })
        .map(|(_, id)| id)
}

fn update_display(
    view: Res<SnapshotView>,
    mut markers: Query<(&VertexMarker, &mut Sprite, &mut Transform)>,
    mut text: Query<(&ui::UiText, &mut Text)>,
    mut regions: Query<(&ui::UiRegion, &mut Node)>,
) {
    if !view.is_changed() {
        return;
    }

    for (marker, mut sprite, mut transform) in &mut markers {
        let (outline, depth) = if view.selected == Some(marker.0) {
            (UI_COLORS.lavender, 2.0)
        } else {
            (UI_COLORS.crust, 1.0)
        };

        if sprite.color != Color::from(outline) {
            sprite.color = outline.into();
        }

        if transform.translation.z != depth {
            transform.translation.z = depth;
        }
    }

    let snapshot = view
        .displayed
        .as_ref()
        .expect("viewer setup loaded a snapshot");
    ui::refresh(
        snapshot,
        view.selected,
        view.bounds.bounds().unwrap(),
        &mut text,
        &mut regions,
    );
}

#[cfg(test)]
mod tests;

use bevy::prelude::*;

use crate::grid::{GridCoord, GridLayout};

use super::{EDGE_Z, RenderAssets, RenderPalette, VERTEX_Z};

/// A vertex visual anchored to the lattice coordinate on the same entity.
#[derive(Component)]
#[require(Transform, Visibility)]
pub struct VertexVisual;

/// Connects two vertex entities without owning their background tiles.
#[derive(Component, Clone, Copy, Debug)]
#[require(Transform, Visibility)]
pub struct EdgeVisual {
    pub source: Entity,
    pub target: Entity,
}

impl EdgeVisual {
    pub const fn new(source: Entity, target: Entity) -> Self {
        Self { source, target }
    }
}

/// Creates the components for a vertex at a lattice site.
///
/// Pass directly to `commands.spawn(vertex(9, 6))`, or add your own components
/// with `commands.spawn((vertex(9, 6), additional_component))`.
pub fn vertex(column: u32, row: u32) -> impl Bundle {
    (VertexVisual, GridCoord::new(column, row))
}

/// Creates the components for an edge between two vertex entities.
///
/// Pass to `commands.spawn(edge(source, target))`, optionally alongside more
/// components in an outer tuple.
pub fn edge(source: Entity, target: Entity) -> impl Bundle {
    (EdgeVisual::new(source, target),)
}

pub(super) fn register_observers(app: &mut App) {
    app.add_observer(on_insert_vertex)
        .add_observer(on_insert_edge)
        .add_observer(on_remove_vertex)
        .add_observer(on_remove_edge);
}

// Insert also handles coordinates added after the marker, or replaced later.
fn on_insert_vertex(
    insert: On<Insert<(VertexVisual, GridCoord)>>,
    mut commands: Commands,
    layout: Res<GridLayout>,
    palette: Res<RenderPalette>,
    assets: Res<RenderAssets>,
    vertices: Query<&GridCoord, With<VertexVisual>>,
) {
    let Ok(&coord) = vertices.get(insert.entity) else {
        return;
    };

    commands
        .entity(insert.entity)
        .try_insert(vertex_render_components(coord, &layout, &palette, &assets));
}

fn on_insert_edge(
    insert: On<Insert<EdgeVisual>>,
    mut commands: Commands,
    layout: Res<GridLayout>,
    palette: Res<RenderPalette>,
    edges: Query<&EdgeVisual>,
    vertices: Query<&GridCoord, With<VertexVisual>>,
) {
    let edge = edges.get(insert.entity).expect("inserted edge exists");
    let source = vertices.get(edge.source).ok().copied();
    let target = vertices.get(edge.target).ok().copied();

    // Keep unresolved edges hidden until the current command batch is complete.
    commands
        .entity(insert.entity)
        .try_insert(edge_render_components(source, target, &layout, &palette));
}

fn on_remove_vertex(
    remove: On<Remove<(VertexVisual, GridCoord)>>,
    mut commands: Commands,
    vertices: Query<(), With<VertexVisual>>,
    edges: Query<(Entity, &EdgeVisual)>,
) {
    if !vertices.contains(remove.entity) {
        return;
    }

    commands.entity(remove.entity).try_remove::<Sprite>();

    for (entity, edge) in &edges {
        if edge.source == remove.entity || edge.target == remove.entity {
            commands.entity(entity).try_despawn();
        }
    }
}

fn on_remove_edge(remove: On<Remove<EdgeVisual>>, mut commands: Commands) {
    commands.entity(remove.entity).try_remove::<Sprite>();
}

pub(super) fn sync_vertices(
    layout: Res<GridLayout>,
    palette: Res<RenderPalette>,
    assets: Res<RenderAssets>,
    mut vertices: Query<
        (Ref<GridCoord>, &mut Transform, &mut Visibility, &mut Sprite),
        With<VertexVisual>,
    >,
) {
    let refresh_all = layout.is_changed() || palette.is_changed();

    for (coord, mut transform, mut visibility, mut sprite) in &mut vertices {
        if !refresh_all && !coord.is_changed() {
            continue;
        }

        let (desired_sprite, desired_transform, desired_visibility) =
            vertex_render_components(*coord, &layout, &palette, &assets);

        *sprite = desired_sprite;
        *transform = desired_transform;
        *visibility = desired_visibility;
    }
}

pub(super) fn sync_edges(
    mut commands: Commands,
    layout: Res<GridLayout>,
    palette: Res<RenderPalette>,
    vertices: Query<Ref<GridCoord>, With<VertexVisual>>,
    mut edges: Query<
        (
            Entity,
            Ref<EdgeVisual>,
            &mut Transform,
            &mut Visibility,
            &mut Sprite,
        ),
        (With<EdgeVisual>, Without<VertexVisual>),
    >,
) {
    let refresh_all = layout.is_changed() || palette.is_changed();

    for (entity, edge, mut transform, mut visibility, mut sprite) in &mut edges {
        let (Ok(source), Ok(target)) = (vertices.get(edge.source), vertices.get(edge.target))
        else {
            commands.entity(entity).try_despawn();
            continue;
        };

        if !refresh_all && !edge.is_changed() && !source.is_changed() && !target.is_changed() {
            continue;
        }

        let (desired_sprite, desired_transform, desired_visibility) =
            edge_render_components(Some(*source), Some(*target), &layout, &palette);

        *sprite = desired_sprite;
        *transform = desired_transform;
        *visibility = desired_visibility;
    }
}

fn vertex_render_components(
    coord: GridCoord,
    layout: &GridLayout,
    palette: &RenderPalette,
    assets: &RenderAssets,
) -> (Sprite, Transform, Visibility) {
    let sprite = Sprite {
        image: assets.vertex.clone(),
        color: palette.vertex,
        custom_size: Some(Vec2::splat(layout.tile_side() * 0.4)),
        ..default()
    };
    let transform = Transform::from_translation(layout.world_position(coord).extend(VERTEX_Z));
    let visibility = if layout.index(coord).is_some() {
        Visibility::Visible
    } else {
        Visibility::Hidden
    };

    (sprite, transform, visibility)
}

fn edge_render_components(
    source: Option<GridCoord>,
    target: Option<GridCoord>,
    layout: &GridLayout,
    palette: &RenderPalette,
) -> (Sprite, Transform, Visibility) {
    let geometry = source
        .zip(target)
        .filter(|(source, target)| {
            layout.index(*source).is_some() && layout.index(*target).is_some()
        })
        .and_then(|(source, target)| {
            edge_geometry(layout.world_position(source), layout.world_position(target))
        });

    let (transform, length, visibility) = match geometry {
        Some((transform, length)) => (transform, length, Visibility::Visible),
        None => (Transform::default(), 0.0, Visibility::Hidden),
    };

    (
        Sprite::from_color(palette.edge, Vec2::new(length, 2.0)),
        transform,
        visibility,
    )
}

fn edge_geometry(source: Vec2, target: Vec2) -> Option<(Transform, f32)> {
    let delta = target - source;
    let length = delta.length();

    if !length.is_finite() || length == 0.0 {
        return None;
    }

    let transform = Transform::from_translation(((source + target) / 2.0).extend(EDGE_Z))
        .with_rotation(Quat::from_rotation_z(delta.y.atan2(delta.x)));

    Some((transform, length))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edge_endpoints_match_lattice_centers() {
        let layout = GridLayout::new(4, 6, 50.0, 44.0, 8.0);
        let source = layout.world_position(GridCoord::new(0, 1));
        let target = layout.world_position(GridCoord::new(3, 5));
        let (transform, length) = edge_geometry(source, target).unwrap();
        let half_edge = transform.rotation * Vec3::new(length / 2.0, 0.0, 0.0);
        let rendered_source = (transform.translation - half_edge).truncate();
        let rendered_target = (transform.translation + half_edge).truncate();

        assert!(rendered_source.distance(source) < 0.0001);
        assert!(rendered_target.distance(target) < 0.0001);
        assert_eq!(transform.translation.z, EDGE_Z);
    }

    #[test]
    fn coincident_endpoints_do_not_produce_an_edge() {
        assert!(edge_geometry(Vec2::ZERO, Vec2::ZERO).is_none());
    }
}

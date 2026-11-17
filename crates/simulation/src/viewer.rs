//! Bevy presentation of completed simulation snapshots.

use std::f32::consts::FRAC_PI_4;

use bevy::prelude::*;
use simulation::{
    normalization::BoundsNormalizer,
    rendering::{
        Heatmap, TILE_SIZE, Tile, UI_COLORS, cell_center, heatmap_color, redraw_heatmap,
        spawn_heatmap,
    },
    simulation::{SimulationSnapshot, SnapshotReader},
};

#[derive(Resource)]
struct SnapshotView {
    reader: SnapshotReader,
    displayed_tick: u64,
    bounds: BoundsNormalizer,
}

#[derive(Component)]
struct VertexMarker;

#[derive(Component)]
struct RunStatus;

/// Draws the initial snapshot on startup and refreshes the display when the tick changes.
pub(super) fn configure(app: &mut App, reader: SnapshotReader) {
    app.insert_resource(ClearColor(UI_COLORS.base.into()))
        .insert_resource(SnapshotView {
            reader,
            displayed_tick: 0,
            bounds: BoundsNormalizer::default(),
        })
        .add_systems(Startup, setup)
        .add_systems(Update, refresh);
}

fn setup(mut commands: Commands, mut view: ResMut<SnapshotView>) {
    let snapshot = view.reader.latest();
    view.bounds.update_bounds(&snapshot.field, 0.0);
    view.displayed_tick = snapshot.tick;

    commands.spawn(Camera2d);
    spawn_heatmap(&mut commands, &snapshot.field, &view.bounds);

    let vertex = snapshot.vertices.first().expect("starting vertex exists");
    let position = cell_center(&snapshot.field, vertex.position);
    commands
        .spawn((
            VertexMarker,
            Sprite::from_color(UI_COLORS.crust, Vec2::splat(TILE_SIZE * 1.9)),
            Transform::from_xyz(position.x, position.y, 1.0)
                .with_rotation(Quat::from_rotation_z(FRAC_PI_4)),
        ))
        .with_child((
            Sprite::from_color(UI_COLORS.rosewater, Vec2::splat(TILE_SIZE * 1.25)),
            Transform::from_xyz(0.0, 0.0, 0.1),
        ));

    spawn_header(&mut commands, &snapshot);
    spawn_legend(&mut commands, &view.bounds);
}

fn refresh(
    mut view: ResMut<SnapshotView>,
    heatmap: Res<Heatmap>,
    mut tiles: Query<(&mut Sprite, Option<&Children>), With<Tile>>,
    mut labels: Query<(&mut Text2d, &mut TextColor)>,
    mut marker: Single<&mut Transform, With<VertexMarker>>,
    mut status: Single<&mut Text, With<RunStatus>>,
) {
    let snapshot = view.reader.latest();

    if snapshot.tick == view.displayed_tick {
        return;
    }

    redraw_heatmap(
        &heatmap,
        &snapshot.field,
        &view.bounds,
        &mut tiles,
        &mut labels,
    );

    let vertex = snapshot.vertices.first().expect("starting vertex exists");
    let position = cell_center(&snapshot.field, vertex.position);
    marker.translation = position.extend(1.0);
    status.set_if_neq(Text::new(status_text(&snapshot)));
    view.displayed_tick = snapshot.tick;
}

fn spawn_header(commands: &mut Commands, snapshot: &SimulationSnapshot) {
    commands
        .spawn((
            Node {
                position_type: PositionType::Absolute,
                left: px(0.0),
                right: px(0.0),
                top: px(0.0),
                height: px(60.0),
                padding: UiRect::horizontal(px(24.0)),
                align_items: AlignItems::Center,
                align_content: AlignContent::Center,
                justify_content: JustifyContent::SpaceBetween,
                flex_wrap: FlexWrap::Wrap,
                row_gap: px(4.0),
                column_gap: px(16.0),
                ..default()
            },
            BackgroundColor(UI_COLORS.surface0.into()),
        ))
        .with_children(|header| {
            header.spawn((
                Text::new("Graph simulation"),
                TextFont::from_font_size(21.0),
                TextColor(UI_COLORS.text.into()),
            ));
            header.spawn((
                RunStatus,
                Text::new(status_text(snapshot)),
                TextFont::from_font_size(15.0),
                TextColor(UI_COLORS.subtext1.into()),
            ));
        });
}

fn status_text(snapshot: &SimulationSnapshot) -> String {
    let vertex = snapshot.vertices.first().expect("starting vertex exists");

    format!(
        "Seed {}  |  Tick {}  |  Time {:.1}  |  Stored resource {:.2}",
        snapshot.seed, snapshot.tick, snapshot.simulation_time, vertex.stored_resource
    )
}

fn spawn_legend(commands: &mut Commands, bounds: &BoundsNormalizer) {
    let (min, max) = bounds.bounds().expect("simulation field is nonempty");

    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            left: px(0.0),
            right: px(0.0),
            bottom: px(24.0),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            column_gap: px(10.0),
            ..default()
        })
        .with_children(|legend| {
            legend.spawn((
                Text::new(format!("Cell resource    {min:.1}")),
                TextFont::from_font_size(14.0),
                TextColor(UI_COLORS.subtext1.into()),
            ));
            legend.spawn(Node::default()).with_children(|scale| {
                for index in 0..12 {
                    scale.spawn((
                        Node {
                            width: px(12.0),
                            height: px(8.0),
                            ..default()
                        },
                        BackgroundColor(heatmap_color(index as f32 / 11.0)),
                    ));
                }
            });
            legend.spawn((
                Text::new(format!("{max:.1}")),
                TextFont::from_font_size(14.0),
                TextColor(UI_COLORS.subtext1.into()),
            ));
        });
}

#[cfg(test)]
mod tests {
    use simulation::{
        field::Neighbourhood,
        graph::Uptake,
        simulation::{SimulationConfig, SimulationRun, SnapshotPublisher},
    };

    use super::*;

    #[test]
    fn viewer_draws_the_vertex_and_refreshes_from_a_completed_tick() {
        let config = SimulationConfig {
            width: 9,
            height: 7,
            spacing: 1.0,
            initial_resource: 100.0,
            resource_noise_scale: 10.0,
            diffusion_coefficient: 0.8,
            neighbourhood: Neighbourhood::Moore,
        };

        let uptake = Uptake::new(10.0, 0.75).unwrap();
        let mut run = SimulationRun::single_vertex_from_seed(config, 42, uptake, 0.1).unwrap();
        let initial = run.snapshot();
        let vertex = initial.vertices[0];
        let center = cell_center(&initial.field, vertex.position);
        let mut publisher = SnapshotPublisher::new(initial);
        let mut app = App::new();
        configure(&mut app, publisher.reader());
        app.update();

        let world = app.world_mut();
        let marker = world
            .query_filtered::<Entity, With<VertexMarker>>()
            .single(world)
            .unwrap();
        let status = world
            .query_filtered::<Entity, With<RunStatus>>()
            .single(world)
            .unwrap();
        assert_eq!(
            world.get::<Transform>(marker).unwrap().translation,
            center.extend(1.0)
        );
        assert!(world.get::<Text>(status).unwrap().0.contains("Tick 0"));
        let tiles = world.resource::<Heatmap>().tiles.clone();
        assert_eq!(tiles.len(), 63);

        run.advance();
        let next = run.snapshot();
        publisher.publish(next.clone());
        app.update();

        let world = app.world();
        assert_eq!(world.resource::<Heatmap>().tiles, tiles);
        assert_eq!(world.get::<Text>(status).unwrap().0, status_text(&next));
        assert!(
            world
                .get::<Text>(status)
                .unwrap()
                .0
                .contains("Stored resource 0.75")
        );

        let view = world.resource::<SnapshotView>();
        assert_eq!(view.displayed_tick, 1);

        for (tile, &amount) in tiles.into_iter().zip(next.field.resources()) {
            let sprite = world.get::<Sprite>(tile).unwrap();
            assert_eq!(sprite.color, heatmap_color(view.bounds.normalize(amount)));
        }
    }
}
